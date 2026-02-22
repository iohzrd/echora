import { writable } from 'svelte/store';
import type { VoiceState } from '../api';
import type { VoiceInputMode } from '../voice';

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
  speakingUsers: string[];
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
  speakingUsers: [],
  watchingScreenUserId: null,
  watchingScreenUsername: '',
  watchingCameraUserId: null,
  watchingCameraUsername: '',
});
