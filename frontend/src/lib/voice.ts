import { type VoiceState, type WebSocketManager, API } from './api';
import { user } from './auth';
import { get } from 'svelte/store';
import { MediasoupManager, getChannelProducers } from './mediasoup';

/** Stop all tracks on a MediaStream and return null for assignment. */
function stopStream(stream: MediaStream | null): null {
  if (stream) {
    stream.getTracks().forEach(track => track.stop());
  }
  return null;
}

/** Detach an audio element from the DOM and clean up its source. */
function detachAudioElement(audio: HTMLAudioElement): void {
  audio.srcObject = null;
  audio.remove();
}

export type VoiceInputMode = 'voice-activity' | 'push-to-talk';

export class VoiceManager {
  private mediasoup: MediasoupManager | null = null;
  private remoteAudioElements: Map<string, HTMLAudioElement> = new Map();
  private localStream: MediaStream | null = null;
  private screenStream: MediaStream | null = null;
  private cameraStream: MediaStream | null = null;
  private audioContext: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private gainNode: GainNode | null = null;
  private destinationNode: MediaStreamAudioDestinationNode | null = null;
  private speakingDetectionFrame: number | null = null;
  private isMuted = false;
  private isDeafened = false;
  private isScreenSharing = false;
  private isCameraSharing = false;
  private isConnected = false;
  private currentChannelId: string | null = null;
  private voiceStates: VoiceState[] = [];
  private speakingThreshold = 50;
  private ws: WebSocketManager | null = null;
  private inputMode: VoiceInputMode = 'voice-activity';
  private pttActive = false;

  // Audio settings
  private perUserVolumes: Map<string, number> = new Map();
  private outputVolume = 1.0;
  private currentInputDeviceId = '';
  private currentOutputDeviceId = '';
  private noiseSuppression = true;

  setWebSocketManager(wsManager: WebSocketManager) {
    this.ws = wsManager;
  }

  // Event handlers
  private onVoiceStatesChanged: ((states: VoiceState[]) => void) | null = null;
  private onSpeakingChanged: ((userId: string, isSpeaking: boolean) => void) | null = null;
  private onStateChanged: (() => void) | null = null;
  private onScreenTrackReceived: ((track: MediaStreamTrack, userId: string) => void) | null = null;
  private onScreenTrackRemoved: ((userId: string) => void) | null = null;
  private onCameraTrackReceived: ((track: MediaStreamTrack, userId: string) => void) | null = null;

  async joinVoiceChannel(channelId: string): Promise<void> {
    const currentUser = get(user);
    if (!currentUser) {
      throw new Error('User not authenticated');
    }

    try {
      // Join via REST API first (registers voice state on server)
      await API.joinVoiceChannel({ channel_id: channelId });

      this.currentChannelId = channelId;

      // Initialize mediasoup (device + send/recv transports)
      this.mediasoup = new MediasoupManager();
      this.mediasoup.onTrack = (track, userId, kind, label) => {
        this.handleRemoteTrack(track, userId, kind, label);
      };
      await this.mediasoup.init(channelId);

      // Capture local audio
      await this.setupLocalAudio();

      // Produce local audio track via SFU (use gained track from destination node)
      if (this.destinationNode) {
        const audioTrack = this.destinationNode.stream.getAudioTracks()[0];
        if (audioTrack) {
          await this.mediasoup.produce(audioTrack);
        }
      } else if (this.localStream) {
        const audioTrack = this.localStream.getAudioTracks()[0];
        if (audioTrack) {
          await this.mediasoup.produce(audioTrack);
        }
      }

      // Consume existing producers in the channel
      await this.consumeExistingProducers(channelId, currentUser.id);

      this.isConnected = true;

      // In PTT mode, start muted until key is held
      if (this.inputMode === 'push-to-talk') {
        this.setMuted(true);
      }

      // Reconciliation: re-check for producers after a delay in case of race conditions
      // during the join flow (producers registered after our initial fetch)
      setTimeout(() => {
        this.consumeExistingProducers(channelId, currentUser.id);
      }, 3000);

      // Get current voice states
      await this.updateVoiceStates();

      this.onStateChanged?.();
    } catch (error) {
      console.error('Failed to join voice channel:', error);
      this.cleanup();
      throw error;
    }
  }

  async leaveVoiceChannel(): Promise<void> {
    if (!this.currentChannelId) return;

    try {
      // Stop screen sharing and camera before leaving
      if (this.isScreenSharing) {
        await this.stopScreenShare();
      }
      if (this.isCameraSharing) {
        await this.stopCamera();
      }

      // Close mediasoup transports (server-side cleanup)
      if (this.mediasoup) {
        await this.mediasoup.close();
        this.mediasoup = null;
      }

      // Leave via REST API
      await API.leaveVoiceChannel({ channel_id: this.currentChannelId });

      // Local cleanup
      this.cleanup();
    } catch (error) {
      console.error('Failed to leave voice channel:', error);
      this.cleanup();
      throw error;
    }
  }

  private async consumeExistingProducers(channelId: string, myUserId: string): Promise<void> {
    try {
      const producers = await getChannelProducers(channelId);
      for (const producer of producers) {
        // Skip our own producers and screen/camera producers (consumed on demand when watching)
        if (producer.user_id === myUserId) continue;
        if (producer.label === 'screen') continue;
        if (producer.label === 'camera') continue;

        try {
          await this.mediasoup?.consume(producer.producer_id, producer.user_id, producer.label);
        } catch (e) {
          console.error('Failed to consume producer', producer.producer_id, e);
        }
      }
    } catch (error) {
      console.error('Failed to fetch existing producers:', error);
    }
  }

  async consumeProducer(producerId: string, userId: string, label?: string): Promise<void> {
    if (!this.mediasoup || !this.isConnected) return;

    try {
      await this.mediasoup.consume(producerId, userId, label);
    } catch (e) {
      console.error('Failed to consume producer', producerId, e);
    }
  }

  async reconcileProducers(): Promise<void> {
    if (!this.isConnected || !this.currentChannelId) return;
    const currentUser = get(user);
    if (!currentUser) return;
    await this.consumeExistingProducers(this.currentChannelId, currentUser.id);
  }

  private computeUserVolume(userId: string): number {
    if (this.isDeafened) return 0;
    const perUser = this.perUserVolumes.get(userId) ?? 1.0;
    return Math.min(this.outputVolume * perUser, 1.0);
  }

  private handleRemoteTrack(track: MediaStreamTrack, userId: string, kind: string, label?: string): void {
    // Screen share tracks (video or audio) go to the screen track handler
    if (label === 'screen') {
      this.onScreenTrackReceived?.(track, userId);
      return;
    }

    // Camera tracks go to the camera track handler
    if (label === 'camera') {
      this.onCameraTrackReceived?.(track, userId);
      return;
    }

    if (kind !== 'audio') return;

    let remoteAudio = this.remoteAudioElements.get(userId);
    if (!remoteAudio) {
      remoteAudio = document.createElement('audio');
      remoteAudio.autoplay = true;
      document.body.appendChild(remoteAudio);
      this.remoteAudioElements.set(userId, remoteAudio);
    }

    remoteAudio.volume = this.computeUserVolume(userId);

    // Apply output device if supported
    if (this.currentOutputDeviceId && 'setSinkId' in remoteAudio) {
      (remoteAudio as HTMLAudioElement & { setSinkId: (id: string) => Promise<void> })
        .setSinkId(this.currentOutputDeviceId).catch(() => {});
    }

    remoteAudio.srcObject = new MediaStream([track]);
    remoteAudio.play().catch(e => {
      console.warn('Auto-play prevented for user', userId, ':', e);
    });
  }

  removeUserAudio(userId: string): void {
    const remoteAudio = this.remoteAudioElements.get(userId);
    if (remoteAudio) {
      detachAudioElement(remoteAudio);
      this.remoteAudioElements.delete(userId);
    }
  }

  private async setupLocalAudio(): Promise<void> {
    const constraints: MediaTrackConstraints = {
      echoCancellation: true,
      noiseSuppression: this.noiseSuppression,
      autoGainControl: true,
    };
    if (this.currentInputDeviceId) {
      constraints.deviceId = { exact: this.currentInputDeviceId };
    }

    this.localStream = await navigator.mediaDevices.getUserMedia({ audio: constraints });

    // Setup Web Audio pipeline: Source -> GainNode -> AnalyserNode (VAD)
    //                                             \-> DestinationNode (producer track)
    this.audioContext = new AudioContext();
    const source = this.audioContext.createMediaStreamSource(this.localStream);

    this.gainNode = this.audioContext.createGain();
    source.connect(this.gainNode);

    this.analyser = this.audioContext.createAnalyser();
    this.analyser.fftSize = 512;
    this.gainNode.connect(this.analyser);

    this.destinationNode = this.audioContext.createMediaStreamDestination();
    this.gainNode.connect(this.destinationNode);

    this.startSpeakingDetection();
  }

  private async replaceInputDevice(deviceId: string): Promise<void> {
    // Stop old local stream tracks
    if (this.localStream) {
      this.localStream.getTracks().forEach(t => t.stop());
    }

    // Get new stream with updated constraints
    const constraints: MediaTrackConstraints = {
      echoCancellation: true,
      noiseSuppression: this.noiseSuppression,
      autoGainControl: true,
    };
    if (deviceId) {
      constraints.deviceId = { exact: deviceId };
    }

    this.localStream = await navigator.mediaDevices.getUserMedia({ audio: constraints });

    // Reconnect Web Audio pipeline (GainNode, AnalyserNode, DestinationNode already exist)
    if (this.audioContext && this.gainNode) {
      const source = this.audioContext.createMediaStreamSource(this.localStream);
      source.connect(this.gainNode);
    }

    // Replace the track on the mediasoup producer
    if (this.mediasoup && this.destinationNode) {
      const newTrack = this.destinationNode.stream.getAudioTracks()[0];
      if (newTrack) {
        await this.mediasoup.replaceProducerTrack(newTrack);
      }
    }
  }

  private startSpeakingDetection(): void {
    if (!this.analyser) return;

    const bufferLength = this.analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    let lastSpeakingState = false;

    const checkAudioLevel = () => {
      if (!this.analyser) {
        this.speakingDetectionFrame = null;
        return;
      }

      if (!this.isDeafened) {
        this.analyser.getByteFrequencyData(dataArray);
        const average = dataArray.reduce((a, b) => a + b) / bufferLength;
        const isSpeaking = average > this.speakingThreshold && !this.isMuted;

        if (isSpeaking !== lastSpeakingState) {
          lastSpeakingState = isSpeaking;
          this.updateSpeakingStatus(isSpeaking);
        }
      }

      this.speakingDetectionFrame = requestAnimationFrame(checkAudioLevel);
    };

    this.speakingDetectionFrame = requestAnimationFrame(checkAudioLevel);
  }

  private updateSpeakingStatus(isSpeaking: boolean): void {
    const currentUser = get(user);
    if (currentUser) {
      this.onSpeakingChanged?.(currentUser.id, isSpeaking);
    }
    if (!this.currentChannelId || !this.ws) return;
    this.ws.sendVoiceSpeaking(this.currentChannelId, isSpeaking);
  }

  async startScreenShare(): Promise<void> {
    if (!this.mediasoup || !this.isConnected || !this.currentChannelId) {
      throw new Error('Must be in a voice channel to share screen');
    }

    try {
      this.screenStream = await navigator.mediaDevices.getDisplayMedia({
        video: true,
        audio: true,
      });

      const videoTrack = this.screenStream.getVideoTracks()[0];
      if (!videoTrack) {
        throw new Error('No video track from screen capture');
      }

      // Listen for browser's "Stop sharing" button
      videoTrack.addEventListener('ended', () => {
        this.stopScreenShare();
      });

      await this.mediasoup.produceScreen(videoTrack);

      // Produce desktop audio if the user chose to share it
      const audioTrack = this.screenStream.getAudioTracks()[0];
      if (audioTrack) {
        await this.mediasoup.produceScreen(audioTrack);
      }

      if (this.ws) {
        this.ws.sendScreenShareUpdate(this.currentChannelId, true);
      }
      this.isScreenSharing = true;
      this.onStateChanged?.();
    } catch (error) {
      // User cancelled the screen picker or error occurred
      this.screenStream = stopStream(this.screenStream);
      throw error;
    }
  }

  async stopScreenShare(): Promise<void> {
    if (!this.isScreenSharing) return;

    if (this.mediasoup) {
      this.mediasoup.closeScreenProducers();
    }

    this.screenStream = stopStream(this.screenStream);

    if (this.currentChannelId && this.ws) {
      this.ws.sendScreenShareUpdate(this.currentChannelId, false);
    }

    this.isScreenSharing = false;
    this.onStateChanged?.();
  }

  async startCamera(): Promise<void> {
    if (!this.mediasoup || !this.isConnected || !this.currentChannelId) {
      throw new Error('Must be in a voice channel to share camera');
    }

    try {
      this.cameraStream = await navigator.mediaDevices.getUserMedia({
        video: true,
      });

      const videoTrack = this.cameraStream.getVideoTracks()[0];
      if (!videoTrack) {
        throw new Error('No video track from camera');
      }

      videoTrack.addEventListener('ended', () => {
        this.stopCamera();
      });

      await this.mediasoup.produceCamera(videoTrack);

      if (this.ws) {
        this.ws.sendCameraUpdate(this.currentChannelId, true);
      }
      this.isCameraSharing = true;
      this.onStateChanged?.();
    } catch (error) {
      this.cameraStream = stopStream(this.cameraStream);
      throw error;
    }
  }

  async stopCamera(): Promise<void> {
    if (!this.isCameraSharing) return;

    if (this.mediasoup) {
      this.mediasoup.closeCameraProducers();
    }

    this.cameraStream = stopStream(this.cameraStream);

    if (this.currentChannelId && this.ws) {
      this.ws.sendCameraUpdate(this.currentChannelId, false);
    }

    this.isCameraSharing = false;
    this.onStateChanged?.();
  }

  async toggleMute(): Promise<void> {
    // In PTT mode, toggleMute acts as a force-mute override
    if (this.inputMode === 'push-to-talk') {
      this.setMuted(!this.isMuted);
      return;
    }
    this.setMuted(!this.isMuted);
  }

  private setMuted(muted: boolean): void {
    this.isMuted = muted;

    if (this.mediasoup) {
      if (muted) {
        this.mediasoup.pauseAudioProducer();
      } else {
        this.mediasoup.resumeAudioProducer();
      }
    }

    if (this.localStream) {
      this.localStream.getAudioTracks().forEach(track => {
        track.enabled = !muted;
      });
    }

    if (this.currentChannelId && this.ws) {
      this.ws.sendVoiceStateUpdate(this.currentChannelId, { is_muted: muted });
    }

    this.onStateChanged?.();
  }

  setPTTActive(active: boolean): void {
    if (this.inputMode !== 'push-to-talk') return;
    this.pttActive = active;
    this.setMuted(!active);
  }

  setInputMode(mode: VoiceInputMode): void {
    this.inputMode = mode;
    // Only change mute state if currently in a voice channel
    if (this.isConnected) {
      if (mode === 'push-to-talk') {
        this.setMuted(true);
      } else {
        this.setMuted(false);
      }
    }
  }

  async toggleDeafen(): Promise<void> {
    this.isDeafened = !this.isDeafened;

    // Adjust all remote audio element volumes
    this.remoteAudioElements.forEach((audio, userId) => {
      audio.volume = this.computeUserVolume(userId);
    });

    // Also pause/resume consumers on mediasoup side
    if (this.mediasoup) {
      this.mediasoup.setConsumersEnabled(!this.isDeafened);
    }

    if (this.currentChannelId && this.ws) {
      this.ws.sendVoiceStateUpdate(this.currentChannelId, { is_deafened: this.isDeafened });
    }

    this.onStateChanged?.();
  }

  // --- Audio settings methods ---

  setInputGain(gain: number): void {
    if (this.gainNode) {
      this.gainNode.gain.value = gain;
    }
  }

  setOutputVolume(volume: number): void {
    this.outputVolume = volume;
    this.remoteAudioElements.forEach((audio, userId) => {
      audio.volume = this.computeUserVolume(userId);
    });
  }

  setUserVolume(userId: string, volume: number): void {
    this.perUserVolumes.set(userId, volume);
    const audio = this.remoteAudioElements.get(userId);
    if (audio) {
      audio.volume = this.computeUserVolume(userId);
    }
  }

  getUserVolume(userId: string): number {
    return this.perUserVolumes.get(userId) ?? 1.0;
  }

  setSpeakingThreshold(threshold: number): void {
    this.speakingThreshold = threshold;
  }

  async setNoiseSuppression(enabled: boolean): Promise<void> {
    this.noiseSuppression = enabled;
    if (this.isConnected && this.localStream) {
      await this.replaceInputDevice(this.currentInputDeviceId);
    }
  }

  async setInputDevice(deviceId: string): Promise<void> {
    this.currentInputDeviceId = deviceId;
    if (this.isConnected) {
      await this.replaceInputDevice(deviceId);
    }
  }

  setOutputDevice(deviceId: string): void {
    this.currentOutputDeviceId = deviceId;
    this.remoteAudioElements.forEach(audio => {
      if ('setSinkId' in audio) {
        (audio as HTMLAudioElement & { setSinkId: (id: string) => Promise<void> })
          .setSinkId(deviceId).catch(() => {});
      }
    });
  }

  private async updateVoiceStates(): Promise<void> {
    if (!this.currentChannelId) return;

    try {
      this.voiceStates = await API.getVoiceStates(this.currentChannelId);
      this.onVoiceStatesChanged?.(this.voiceStates);
    } catch (error) {
      console.error('Failed to update voice states:', error);
    }
  }

  private cleanup(): void {
    // Stop speaking detection loop
    if (this.speakingDetectionFrame !== null) {
      cancelAnimationFrame(this.speakingDetectionFrame);
      this.speakingDetectionFrame = null;
    }

    // Clean up all remote audio elements
    this.remoteAudioElements.forEach(detachAudioElement);
    this.remoteAudioElements.clear();

    // Stop local, screen, and camera streams
    this.localStream = stopStream(this.localStream);
    this.screenStream = stopStream(this.screenStream);
    this.cameraStream = stopStream(this.cameraStream);

    // Close audio context
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }

    this.gainNode = null;
    this.destinationNode = null;
    this.analyser = null;
    this.isConnected = false;
    this.isMuted = false;
    this.isDeafened = false;
    this.isScreenSharing = false;
    this.isCameraSharing = false;
    this.currentChannelId = null;
    this.voiceStates = [];

    this.onStateChanged?.();
  }

  // Getters
  get isMutedState(): boolean { return this.isMuted; }
  get isDeafenedState(): boolean { return this.isDeafened; }
  get isScreenSharingState(): boolean { return this.isScreenSharing; }
  get isCameraSharingState(): boolean { return this.isCameraSharing; }
  get isConnectedState(): boolean { return this.isConnected; }
  get currentChannel(): string | null { return this.currentChannelId; }
  get currentVoiceStates(): VoiceState[] { return this.voiceStates; }
  get currentInputMode(): VoiceInputMode { return this.inputMode; }
  get isPTTActive(): boolean { return this.pttActive; }
  get currentInputGain(): number { return this.gainNode?.gain.value ?? 1.0; }
  get currentOutputVolume(): number { return this.outputVolume; }
  get currentSpeakingThresholdValue(): number { return this.speakingThreshold; }
  get isNoiseSuppressionEnabled(): boolean { return this.noiseSuppression; }

  // Event handlers
  onVoiceStatesChange(handler: (states: VoiceState[]) => void): void {
    this.onVoiceStatesChanged = handler;
  }

  onSpeakingChange(handler: (userId: string, isSpeaking: boolean) => void): void {
    this.onSpeakingChanged = handler;
  }

  onStateChange(handler: () => void): void {
    this.onStateChanged = handler;
  }

  onScreenTrack(handler: (track: MediaStreamTrack, userId: string) => void): void {
    this.onScreenTrackReceived = handler;
  }

  onScreenTrackRemove(handler: (userId: string) => void): void {
    this.onScreenTrackRemoved = handler;
  }

  onCameraTrack(handler: (track: MediaStreamTrack, userId: string) => void): void {
    this.onCameraTrackReceived = handler;
  }
}

// Singleton instance
export const voiceManager = new VoiceManager();
