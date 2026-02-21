import { get } from 'svelte/store';
import { voiceManager } from '../voice';
import { audioSettingsStore } from '../stores/voiceStore';
import {
  loadAudioSettings,
  saveAudioSettings,
  enumerateAudioDevices,
  onDeviceChange,
  loadPerUserVolumes,
  savePerUserVolume,
} from '../audioSettings';

export let removeDeviceListener: (() => void) | null = null;

export async function initAudioSettings() {
  const settings = loadAudioSettings();
  audioSettingsStore.set({
    inputDeviceId: settings.inputDeviceId,
    outputDeviceId: settings.outputDeviceId,
    inputGain: settings.inputGain,
    outputVolume: settings.outputVolume,
    vadSensitivity: settings.vadSensitivity,
    noiseSuppression: settings.noiseSuppression,
    inputDevices: [],
    outputDevices: [],
  });

  voiceManager.setInputGain(settings.inputGain);
  voiceManager.setOutputVolume(settings.outputVolume);
  voiceManager.setSpeakingThreshold(settings.vadSensitivity);

  const perUserVols = loadPerUserVolumes();
  for (const [userId, vol] of Object.entries(perUserVols)) {
    voiceManager.setUserVolume(userId, vol);
  }

  await refreshDeviceList();
  removeDeviceListener = onDeviceChange(() => refreshDeviceList());
}

export async function refreshDeviceList() {
  const devices = await enumerateAudioDevices();
  audioSettingsStore.update((s) => ({
    ...s,
    inputDevices: devices.inputs,
    outputDevices: devices.outputs,
  }));
}

function saveCurrentSettings() {
  const s = get(audioSettingsStore);
  saveAudioSettings({
    inputDeviceId: s.inputDeviceId,
    outputDeviceId: s.outputDeviceId,
    inputGain: s.inputGain,
    outputVolume: s.outputVolume,
    vadSensitivity: s.vadSensitivity,
    noiseSuppression: s.noiseSuppression,
  });
}

export function changeInputDevice(deviceId: string) {
  audioSettingsStore.update((s) => ({ ...s, inputDeviceId: deviceId }));
  voiceManager.setInputDevice(deviceId);
  saveCurrentSettings();
}

export function changeOutputDevice(deviceId: string) {
  audioSettingsStore.update((s) => ({ ...s, outputDeviceId: deviceId }));
  voiceManager.setOutputDevice(deviceId);
  saveCurrentSettings();
}

export function changeInputGain(gain: number) {
  audioSettingsStore.update((s) => ({ ...s, inputGain: gain }));
  voiceManager.setInputGain(gain);
  saveCurrentSettings();
}

export function changeOutputVolume(volume: number) {
  audioSettingsStore.update((s) => ({ ...s, outputVolume: volume }));
  voiceManager.setOutputVolume(volume);
  saveCurrentSettings();
}

export function changeVadSensitivity(sensitivity: number) {
  audioSettingsStore.update((s) => ({ ...s, vadSensitivity: sensitivity }));
  voiceManager.setSpeakingThreshold(sensitivity);
  saveCurrentSettings();
}

export function toggleNoiseSuppression(enabled: boolean) {
  audioSettingsStore.update((s) => ({ ...s, noiseSuppression: enabled }));
  voiceManager.setNoiseSuppression(enabled);
  saveCurrentSettings();
}

export function changeUserVolume(userId: string, volume: number) {
  voiceManager.setUserVolume(userId, volume);
  savePerUserVolume(userId, volume);
}

export function getUserVolume(userId: string): number {
  return voiceManager.getUserVolume(userId);
}
