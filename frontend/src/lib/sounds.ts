import { loadAudioSettings } from './audioSettings';

type SoundName = 'connect' | 'disconnect';

const bufferCache = new Map<SoundName, AudioBuffer>();

function generateTone(
  ctx: OfflineAudioContext,
  frequency: number,
  startTime: number,
  duration: number,
  gain: number
): void {
  const osc = ctx.createOscillator();
  const gainNode = ctx.createGain();
  osc.type = 'sine';
  osc.frequency.value = frequency;
  gainNode.gain.setValueAtTime(gain, startTime);
  gainNode.gain.exponentialRampToValueAtTime(0.001, startTime + duration);
  osc.connect(gainNode);
  gainNode.connect(ctx.destination);
  osc.start(startTime);
  osc.stop(startTime + duration);
}

async function generateConnectBuffer(): Promise<AudioBuffer> {
  const sampleRate = 44100;
  const duration = 0.25;
  const ctx = new OfflineAudioContext(1, sampleRate * duration, sampleRate);
  generateTone(ctx, 440, 0, 0.15, 0.3);
  generateTone(ctx, 660, 0.08, 0.17, 0.3);
  return ctx.startRendering();
}

async function generateDisconnectBuffer(): Promise<AudioBuffer> {
  const sampleRate = 44100;
  const duration = 0.25;
  const ctx = new OfflineAudioContext(1, sampleRate * duration, sampleRate);
  generateTone(ctx, 660, 0, 0.15, 0.3);
  generateTone(ctx, 440, 0.08, 0.17, 0.3);
  return ctx.startRendering();
}

async function getBuffer(name: SoundName): Promise<AudioBuffer> {
  let buffer = bufferCache.get(name);
  if (buffer) return buffer;
  buffer = name === 'connect' ? await generateConnectBuffer() : await generateDisconnectBuffer();
  bufferCache.set(name, buffer);
  return buffer;
}

export async function playSound(name: SoundName): Promise<void> {
  try {
    const settings = loadAudioSettings();
    const ctx = new AudioContext();
    const buffer = await getBuffer(name);
    const source = ctx.createBufferSource();
    const gainNode = ctx.createGain();
    source.buffer = buffer;
    gainNode.gain.value = settings.outputVolume;
    source.connect(gainNode);
    gainNode.connect(ctx.destination);

    if (settings.outputDeviceId && 'setSinkId' in ctx) {
      try {
        await (ctx as unknown as { setSinkId: (id: string) => Promise<void> }).setSinkId(
          settings.outputDeviceId
        );
      } catch {
        // fall back to default device
      }
    }

    source.start();
    source.onended = () => ctx.close();
  } catch {
    // silently fail -- sounds are non-critical
  }
}
