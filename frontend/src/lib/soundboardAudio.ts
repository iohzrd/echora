import { audioSettingsStore } from "./stores/audioSettingsStore.svelte";
import { API } from "./api";

const audioBufferCache = new Map<string, AudioBuffer>();
let sharedCtx: AudioContext | null = null;

function getAudioContext(): AudioContext {
  if (!sharedCtx || sharedCtx.state === "closed") {
    sharedCtx = new AudioContext();
  }
  return sharedCtx;
}

export async function playSoundboardAudio(
  soundId: string,
  soundVolume: number,
): Promise<void> {
  try {
    const settings = audioSettingsStore;
    const ctx = getAudioContext();
    if (ctx.state === "suspended") {
      await ctx.resume();
    }

    // Route to selected output device
    if (settings.outputDeviceId && "setSinkId" in ctx) {
      try {
        await (
          ctx as unknown as { setSinkId: (id: string) => Promise<void> }
        ).setSinkId(settings.outputDeviceId);
      } catch {
        // fall back to default device
      }
    }

    // Fetch and decode audio (with cache)
    let buffer = audioBufferCache.get(soundId);
    if (!buffer) {
      const url = API.getSoundAudioUrl(soundId);
      const response = await fetch(url);
      const arrayBuffer = await response.arrayBuffer();
      buffer = await ctx.decodeAudioData(arrayBuffer);
      audioBufferCache.set(soundId, buffer);
    }

    const source = ctx.createBufferSource();
    source.buffer = buffer;

    const gainNode = ctx.createGain();
    gainNode.gain.value = soundVolume * settings.outputVolume;

    source.connect(gainNode);
    gainNode.connect(ctx.destination);
    source.start();
  } catch {
    // silently fail -- soundboard audio is non-critical
  }
}

/** Pre-cache a sound's audio buffer for low-latency playback. */
export async function precacheSoundAudio(soundId: string): Promise<void> {
  if (audioBufferCache.has(soundId)) return;
  try {
    const ctx = getAudioContext();
    const url = API.getSoundAudioUrl(soundId);
    const response = await fetch(url);
    const arrayBuffer = await response.arrayBuffer();
    const buffer = await ctx.decodeAudioData(arrayBuffer);
    audioBufferCache.set(soundId, buffer);
  } catch {
    // ignore precache failures
  }
}

/** Clear a specific sound from cache (e.g., after deletion). */
export function evictSoundCache(soundId: string): void {
  audioBufferCache.delete(soundId);
}
