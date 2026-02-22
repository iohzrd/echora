import { writable } from 'svelte/store';
import type { AudioDevice } from '../audioSettings';

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
