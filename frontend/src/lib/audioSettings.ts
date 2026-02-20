const STORAGE_KEY = 'audio-settings';
const PER_USER_STORAGE_KEY = 'per-user-volume';

export interface AudioSettings {
  inputDeviceId: string;
  outputDeviceId: string;
  inputGain: number;
  outputVolume: number;
  vadSensitivity: number;
  noiseSuppression: boolean;
}

export interface AudioDevice {
  deviceId: string;
  label: string;
  kind: 'audioinput' | 'audiooutput';
}

function getDefaultAudioSettings(): AudioSettings {
  return {
    inputDeviceId: '',
    outputDeviceId: '',
    inputGain: 1.0,
    outputVolume: 1.0,
    vadSensitivity: 50,
    noiseSuppression: true,
  };
}

export function loadAudioSettings(): AudioSettings {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      return { ...getDefaultAudioSettings(), ...JSON.parse(saved) };
    }
  } catch {
    // ignore parse errors
  }
  return getDefaultAudioSettings();
}

export function saveAudioSettings(settings: AudioSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

export async function enumerateAudioDevices(): Promise<{
  inputs: AudioDevice[];
  outputs: AudioDevice[];
}> {
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const inputs: AudioDevice[] = [];
    const outputs: AudioDevice[] = [];

    for (const device of devices) {
      if (device.kind === 'audioinput') {
        inputs.push({
          deviceId: device.deviceId,
          label: device.label || `Microphone ${inputs.length + 1}`,
          kind: 'audioinput',
        });
      } else if (device.kind === 'audiooutput') {
        outputs.push({
          deviceId: device.deviceId,
          label: device.label || `Speaker ${outputs.length + 1}`,
          kind: 'audiooutput',
        });
      }
    }

    return { inputs, outputs };
  } catch {
    return { inputs: [], outputs: [] };
  }
}

export function onDeviceChange(callback: () => void): () => void {
  navigator.mediaDevices.addEventListener('devicechange', callback);
  return () => navigator.mediaDevices.removeEventListener('devicechange', callback);
}

export function supportsOutputDeviceSelection(): boolean {
  return typeof HTMLAudioElement !== 'undefined' && 'setSinkId' in HTMLAudioElement.prototype;
}

// --- Per-user volume (separate storage) ---

export function loadPerUserVolumes(): Record<string, number> {
  try {
    const saved = localStorage.getItem(PER_USER_STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch {
    // ignore
  }
  return {};
}

export function savePerUserVolume(userId: string, volume: number): void {
  const volumes = loadPerUserVolumes();
  volumes[userId] = volume;
  localStorage.setItem(PER_USER_STORAGE_KEY, JSON.stringify(volumes));
}

export function getPerUserVolume(userId: string): number {
  const volumes = loadPerUserVolumes();
  return volumes[userId] ?? 1.0;
}
