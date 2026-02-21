import { writable } from 'svelte/store';
import type { VoiceState } from '../api';
import type { VoiceInputMode } from '../voice';
import type { AudioDevice } from '../audioSettings';

export interface VoiceStoreState {
  currentVoiceChannel: string | null;
  isMuted: boolean;
  isDeafened: boolean;
  isScreenSharing: boolean;
  isCameraSharing: boolean;
  voiceInputMode: VoiceInputMode;
  pttKey: string;
  pttActive: boolean;
  voiceStates: VoiceState[];
  speakingUsers: Set<string>;
  watchingScreenUserId: string | null;
  watchingScreenUsername: string;
  watchingCameraUserId: string | null;
  watchingCameraUsername: string;
}

export const voiceStore = writable<VoiceStoreState>({
  currentVoiceChannel: null,
  isMuted: false,
  isDeafened: false,
  isScreenSharing: false,
  isCameraSharing: false,
  voiceInputMode: 'voice-activity',
  pttKey: 'Space',
  pttActive: false,
  voiceStates: [],
  speakingUsers: new Set(),
  watchingScreenUserId: null,
  watchingScreenUsername: '',
  watchingCameraUserId: null,
  watchingCameraUsername: '',
});

export interface AudioSettingsStoreState {
  inputDeviceId: string;
  outputDeviceId: string;
  inputGain: number;
  outputVolume: number;
  vadSensitivity: number;
  noiseSuppression: boolean;
  inputDevices: AudioDevice[];
  outputDevices: AudioDevice[];
}

export const audioSettingsStore = writable<AudioSettingsStoreState>({
  inputDeviceId: '',
  outputDeviceId: '',
  inputGain: 1.0,
  outputVolume: 1.0,
  vadSensitivity: 50,
  noiseSuppression: true,
  inputDevices: [],
  outputDevices: [],
});
