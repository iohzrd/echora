import { type VoiceState, API } from './api';
import { user } from './auth';
import { get } from 'svelte/store';
import { MediasoupManager, getChannelProducers } from './mediasoup';

export class VoiceManager {
  private mediasoup: MediasoupManager | null = null;
  private remoteAudioElements: Map<string, HTMLAudioElement> = new Map();
  private localStream: MediaStream | null = null;
  private screenStream: MediaStream | null = null;
  private audioContext: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private speakingDetectionFrame: number | null = null;
  private isMuted = false;
  private isDeafened = false;
  private isScreenSharing = false;
  private isConnected = false;
  private currentChannelId: string | null = null;
  private voiceStates: VoiceState[] = [];
  private speakingThreshold = 50;

  // Event handlers
  private onVoiceStatesChanged: ((states: VoiceState[]) => void) | null = null;
  private onSpeakingChanged: ((userId: string, isSpeaking: boolean) => void) | null = null;
  private onStateChanged: (() => void) | null = null;
  private onScreenTrackReceived: ((track: MediaStreamTrack, userId: string) => void) | null = null;
  private onScreenTrackRemoved: ((userId: string) => void) | null = null;

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

      // Produce local audio track via SFU
      if (this.localStream) {
        const audioTrack = this.localStream.getAudioTracks()[0];
        if (audioTrack) {
          await this.mediasoup.produce(audioTrack);
        }
      }

      // Consume existing producers in the channel
      await this.consumeExistingProducers(channelId, currentUser.id);

      this.isConnected = true;

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
      // Stop screen sharing before leaving
      if (this.isScreenSharing) {
        await this.stopScreenShare();
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
      console.log('Channel producers:', producers.length, 'total,', producers.filter(p => p.user_id !== myUserId && p.label !== 'screen').length, 'to consume');
      for (const producer of producers) {
        // Skip our own producers and screen share producers (consumed on demand when watching)
        if (producer.user_id === myUserId) continue;
        if (producer.label === 'screen') continue;

        try {
          console.log('Consuming producer', producer.producer_id.substring(0, 8), 'from user', producer.user_id.substring(0, 8), 'kind:', producer.kind);
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

  private handleRemoteTrack(track: MediaStreamTrack, userId: string, kind: string, label?: string): void {
    // Screen share tracks (video or audio) go to the screen track handler
    if (label === 'screen') {
      console.log('Received screen share', kind, 'track from user', userId);
      this.onScreenTrackReceived?.(track, userId);
      return;
    }

    if (kind !== 'audio') return;

    console.log('handleRemoteTrack: audio from user', userId.substring(0, 8), 'track.readyState:', track.readyState, 'track.enabled:', track.enabled);

    let remoteAudio = this.remoteAudioElements.get(userId);
    if (!remoteAudio) {
      remoteAudio = document.createElement('audio');
      remoteAudio.autoplay = true;
      remoteAudio.volume = 1.0;
      document.body.appendChild(remoteAudio);
      this.remoteAudioElements.set(userId, remoteAudio);
      console.log('Created new audio element for user', userId.substring(0, 8), 'total elements:', this.remoteAudioElements.size);
    }

    remoteAudio.srcObject = new MediaStream([track]);

    const playPromise = remoteAudio.play();
    if (playPromise !== undefined) {
      playPromise.catch(e => {
        console.warn('Auto-play prevented for user', userId, ':', e);
      });
    }

    // Apply deafen state to new track
    if (this.isDeafened) {
      remoteAudio.volume = 0;
    }
  }

  removeUserAudio(userId: string): void {
    const remoteAudio = this.remoteAudioElements.get(userId);
    if (remoteAudio) {
      remoteAudio.srcObject = null;
      if (remoteAudio.parentNode) {
        remoteAudio.parentNode.removeChild(remoteAudio);
      }
      this.remoteAudioElements.delete(userId);
    }
  }

  private async setupLocalAudio(): Promise<void> {
    this.localStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
    });

    // Setup audio analysis for speaking detection
    this.audioContext = new AudioContext();
    const source = this.audioContext.createMediaStreamSource(this.localStream);
    this.analyser = this.audioContext.createAnalyser();
    this.analyser.fftSize = 512;
    source.connect(this.analyser);

    this.startSpeakingDetection();
  }

  private startSpeakingDetection(): void {
    if (!this.analyser) return;

    const bufferLength = this.analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    let lastSpeakingState = false;

    const checkAudioLevel = () => {
      if (!this.analyser || !this.isConnected) {
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

  private async updateSpeakingStatus(isSpeaking: boolean): Promise<void> {
    if (!this.currentChannelId) return;

    try {
      await API.updateSpeakingStatus(this.currentChannelId, { is_speaking: isSpeaking });
    } catch (error) {
      console.error('Failed to update speaking status:', error);
    }
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

      await API.updateScreenShareState(this.currentChannelId, true);
      this.isScreenSharing = true;
      this.onStateChanged?.();
    } catch (error) {
      // User cancelled the screen picker or error occurred
      if (this.screenStream) {
        this.screenStream.getTracks().forEach(t => t.stop());
        this.screenStream = null;
      }
      throw error;
    }
  }

  async stopScreenShare(): Promise<void> {
    if (!this.isScreenSharing) return;

    if (this.mediasoup) {
      this.mediasoup.closeScreenProducers();
    }

    if (this.screenStream) {
      this.screenStream.getTracks().forEach(t => t.stop());
      this.screenStream = null;
    }

    if (this.currentChannelId) {
      try {
        await API.updateScreenShareState(this.currentChannelId, false);
      } catch (error) {
        console.error('Failed to update screen share state:', error);
      }
    }

    this.isScreenSharing = false;
    this.onStateChanged?.();
  }

  async toggleMute(): Promise<void> {
    this.isMuted = !this.isMuted;

    // Pause/resume the mediasoup producer (local mute)
    if (this.mediasoup) {
      if (this.isMuted) {
        this.mediasoup.pauseAudioProducer();
      } else {
        this.mediasoup.resumeAudioProducer();
      }
    }

    // Also mute the local stream track
    if (this.localStream) {
      this.localStream.getAudioTracks().forEach(track => {
        track.enabled = !this.isMuted;
      });
    }

    if (this.currentChannelId) {
      try {
        await API.updateVoiceState(this.currentChannelId, { is_muted: this.isMuted });
      } catch (error) {
        console.error('Failed to update mute state:', error);
      }
    }

    this.onStateChanged?.();
  }

  async toggleDeafen(): Promise<void> {
    this.isDeafened = !this.isDeafened;

    // Mute/unmute all remote audio elements
    this.remoteAudioElements.forEach(audio => {
      audio.volume = this.isDeafened ? 0 : 1.0;
    });

    // Also pause/resume consumers on mediasoup side
    if (this.mediasoup) {
      this.mediasoup.setConsumersEnabled(!this.isDeafened);
    }

    if (this.currentChannelId) {
      try {
        await API.updateVoiceState(this.currentChannelId, { is_deafened: this.isDeafened });
      } catch (error) {
        console.error('Failed to update deafen state:', error);
      }
    }

    this.onStateChanged?.();
  }

  private async updateVoiceStates(): Promise<void> {
    if (!this.currentChannelId) return;

    try {
      this.voiceStates = await API.getVoiceStates(this.currentChannelId);
      if (this.onVoiceStatesChanged) {
        this.onVoiceStatesChanged(this.voiceStates);
      }
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
    this.remoteAudioElements.forEach(audio => {
      audio.srcObject = null;
      if (audio.parentNode) {
        audio.parentNode.removeChild(audio);
      }
    });
    this.remoteAudioElements.clear();

    // Stop local stream
    if (this.localStream) {
      this.localStream.getTracks().forEach(track => track.stop());
      this.localStream = null;
    }

    // Stop screen stream
    if (this.screenStream) {
      this.screenStream.getTracks().forEach(track => track.stop());
      this.screenStream = null;
    }

    // Close audio context
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }

    this.isConnected = false;
    this.isMuted = false;
    this.isDeafened = false;
    this.isScreenSharing = false;
    this.currentChannelId = null;
    this.voiceStates = [];

    this.onStateChanged?.();
  }

  // Getters
  get isMutedState(): boolean { return this.isMuted; }
  get isDeafenedState(): boolean { return this.isDeafened; }
  get isScreenSharingState(): boolean { return this.isScreenSharing; }
  get isConnectedState(): boolean { return this.isConnected; }
  get currentChannel(): string | null { return this.currentChannelId; }
  get currentVoiceStates(): VoiceState[] { return this.voiceStates; }

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
}

// Singleton instance
export const voiceManager = new VoiceManager();
