import { voiceManager } from "../voice";
import { audioSettingsStore } from "../stores/audioSettingsStore.svelte";
import {
  loadAudioSettings,
  saveAudioSettings,
  enumerateAudioDevices,
  onDeviceChange,
  loadPerUserVolumes,
  savePerUserVolume,
} from "../audioSettings";

export async function initAudioSettings(): Promise<() => void> {
  const settings = loadAudioSettings();
  audioSettingsStore.inputDeviceId = settings.inputDeviceId;
  audioSettingsStore.outputDeviceId = settings.outputDeviceId;
  audioSettingsStore.inputGain = settings.inputGain;
  audioSettingsStore.outputVolume = settings.outputVolume;
  audioSettingsStore.vadSensitivity = settings.vadSensitivity;
  audioSettingsStore.noiseSuppression = settings.noiseSuppression;
  audioSettingsStore.inputDevices = [];
  audioSettingsStore.outputDevices = [];

  voiceManager.setInputGain(settings.inputGain);
  voiceManager.setOutputVolume(settings.outputVolume);
  voiceManager.setSpeakingThreshold(settings.vadSensitivity);

  const perUserVols = loadPerUserVolumes();
  for (const [userId, vol] of Object.entries(perUserVols)) {
    voiceManager.setUserVolume(userId, vol);
  }

  try {
    await refreshDeviceList();
  } catch (e) {
    console.warn("Could not enumerate audio devices:", e);
  }
  return onDeviceChange(() => refreshDeviceList());
}

export async function refreshDeviceList() {
  try {
    const devices = await enumerateAudioDevices();
    audioSettingsStore.inputDevices = devices.inputs;
    audioSettingsStore.outputDevices = devices.outputs;
  } catch (e) {
    console.warn("Failed to enumerate audio devices:", e);
  }
}

function saveCurrentSettings() {
  saveAudioSettings({
    inputDeviceId: audioSettingsStore.inputDeviceId,
    outputDeviceId: audioSettingsStore.outputDeviceId,
    inputGain: audioSettingsStore.inputGain,
    outputVolume: audioSettingsStore.outputVolume,
    vadSensitivity: audioSettingsStore.vadSensitivity,
    noiseSuppression: audioSettingsStore.noiseSuppression,
  });
}

export function changeInputDevice(deviceId: string) {
  audioSettingsStore.inputDeviceId = deviceId;
  voiceManager.setInputDevice(deviceId);
  saveCurrentSettings();
}

export function changeOutputDevice(deviceId: string) {
  audioSettingsStore.outputDeviceId = deviceId;
  voiceManager.setOutputDevice(deviceId);
  saveCurrentSettings();
}

export function changeInputGain(gain: number) {
  audioSettingsStore.inputGain = gain;
  voiceManager.setInputGain(gain);
  saveCurrentSettings();
}

export function changeOutputVolume(volume: number) {
  audioSettingsStore.outputVolume = volume;
  voiceManager.setOutputVolume(volume);
  saveCurrentSettings();
}

export function changeVadSensitivity(sensitivity: number) {
  audioSettingsStore.vadSensitivity = sensitivity;
  voiceManager.setSpeakingThreshold(sensitivity);
  saveCurrentSettings();
}

export function toggleNoiseSuppression(enabled: boolean) {
  audioSettingsStore.noiseSuppression = enabled;
  voiceManager.setNoiseSuppression(enabled);
  saveCurrentSettings();
}

export function changeUserVolume(userId: string, volume: number) {
  voiceManager.setUserVolume(userId, volume);
  savePerUserVolume(userId, volume);
}
