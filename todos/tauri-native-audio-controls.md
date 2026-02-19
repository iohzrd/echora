# Tauri Native Audio Controls

Research and implementation plan for Discord-style native audio controls in the Echora Tauri desktop app. This covers input/output device selection, per-user volume control, noise suppression, and audio processing -- features that go beyond what the browser WebRTC defaults provide.

## Current State

- Audio capture uses `getUserMedia` with basic constraints: `echoCancellation`, `noiseSuppression`, `autoGainControl` (all browser defaults)
- No input or output device selection -- uses system default
- No per-user volume control -- all remote audio elements are set to `1.0` (or `0` when deafened)
- No noise gate or advanced noise suppression
- No audio processing pipeline beyond the browser's built-in WebRTC processing
- Remote audio playback uses dynamically created `<audio>` elements attached to `document.body`
- Voice controls in ChannelList.svelte: mute, deafen, screen share (text labels, no icons)

## Goals

Replicate the Discord desktop audio experience:

1. **Input device selection** -- choose which microphone to use
2. **Output device selection** -- choose which speakers/headphones to use
3. **Input volume (gain) control** -- adjust microphone sensitivity
4. **Output volume (master) control** -- adjust overall playback volume
5. **Per-user volume control** -- right-click a user in voice to adjust their volume independently
6. **Noise suppression toggle** -- on/off for built-in noise suppression, with option for enhanced (RNNoise/Krisp-style)
7. **Input sensitivity / voice activity threshold** -- visual bar showing input level with adjustable threshold
8. **Audio test / mic test** -- loopback playback so users can hear themselves
9. **Automatic gain control toggle** -- enable/disable AGC independently

## Implementation Plan

### 1. Audio Device Enumeration and Selection

Use the Web Audio API `navigator.mediaDevices.enumerateDevices()` to list available input and output devices. This works in both Tauri WebView and browser contexts.

#### Device Listing

```typescript
// frontend/src/lib/audio.ts (new file)

export interface AudioDevice {
  deviceId: string;
  label: string;
  kind: 'audioinput' | 'audiooutput';
}

export async function getAudioDevices(): Promise<{
  inputs: AudioDevice[];
  outputs: AudioDevice[];
}> {
  const devices = await navigator.mediaDevices.enumerateDevices();
  return {
    inputs: devices
      .filter(d => d.kind === 'audioinput')
      .map(d => ({ deviceId: d.deviceId, label: d.label || `Microphone ${d.deviceId.slice(0, 8)}`, kind: d.kind })),
    outputs: devices
      .filter(d => d.kind === 'audiooutput')
      .map(d => ({ deviceId: d.deviceId, label: d.label || `Speaker ${d.deviceId.slice(0, 8)}`, kind: d.kind })),
  };
}
```

#### Applying Device Selection

**Input device**: Re-acquire `getUserMedia` with the selected `deviceId`:

```typescript
async setInputDevice(deviceId: string): Promise<void> {
  this.localStream = stopStream(this.localStream);

  this.localStream = await navigator.mediaDevices.getUserMedia({
    audio: {
      deviceId: { exact: deviceId },
      echoCancellation: this.settings.echoCancellation,
      noiseSuppression: this.settings.noiseSuppression,
      autoGainControl: this.settings.autoGainControl,
    },
  });

  // Reconnect audio analysis
  this.reconnectAudioAnalysis();

  // Replace the track in the mediasoup producer
  if (this.mediasoup) {
    const newTrack = this.localStream.getAudioTracks()[0];
    await this.mediasoup.replaceAudioTrack(newTrack);
  }
}
```

**Output device**: Use `HTMLAudioElement.setSinkId()` to route playback to a specific device:

```typescript
async setOutputDevice(deviceId: string): Promise<void> {
  for (const audio of this.remoteAudioElements.values()) {
    if (typeof audio.setSinkId === 'function') {
      await audio.setSinkId(deviceId);
    }
  }
  this.settings.outputDeviceId = deviceId;
}
```

> Note: `setSinkId` is available in Chromium-based WebViews (Tauri uses WebView2 on Windows and WebKitGTK on Linux). Safari/WebKit has limited support. For Linux, verify WebKitGTK support.

### 2. MediasoupManager: Track Replacement

Add a method to `MediasoupManager` to hot-swap the audio track on the existing producer without re-negotiation:

```typescript
// In MediasoupManager
async replaceAudioTrack(newTrack: MediaStreamTrack): Promise<void> {
  for (const producer of this.producers.values()) {
    if (producer.kind === 'audio') {
      await producer.replaceTrack({ track: newTrack });
      break;
    }
  }
}
```

mediasoup-client's `Producer.replaceTrack()` does a seamless track swap using RTCRtpSender.replaceTrack under the hood -- no renegotiation needed.

### 3. Input Volume (Gain Control)

Insert a `GainNode` into the audio pipeline between the microphone source and the mediasoup producer. This controls how loud the user's mic is before it's sent to others.

```typescript
private gainNode: GainNode | null = null;
private sourceNode: MediaStreamAudioSourceNode | null = null;
private processedStream: MediaStream | null = null;

private setupAudioPipeline(): void {
  if (!this.localStream || !this.audioContext) return;

  const source = this.audioContext.createMediaStreamSource(this.localStream);
  this.sourceNode = source;

  // Gain node for input volume
  this.gainNode = this.audioContext.createGain();
  this.gainNode.gain.value = this.settings.inputGain; // 0.0 to 2.0, default 1.0

  // Analyser for speaking detection
  this.analyser = this.audioContext.createAnalyser();
  this.analyser.fftSize = 512;

  // Pipeline: source -> gain -> analyser -> destination (for mediasoup)
  source.connect(this.gainNode);
  this.gainNode.connect(this.analyser);

  // Create a processed stream from the gain node output
  const dest = this.audioContext.createMediaStreamDestination();
  this.gainNode.connect(dest);
  this.processedStream = dest.stream;

  // Use the processed (gained) track for mediasoup, not the raw mic track
  // This ensures the gain control actually affects what others hear
}

setInputGain(gain: number): void {
  if (this.gainNode) {
    this.gainNode.gain.value = Math.max(0, Math.min(2.0, gain));
  }
  this.settings.inputGain = gain;
}
```

**Important**: The mediasoup producer must use the track from the `MediaStreamDestination` (processed stream), not the raw `getUserMedia` track. Otherwise the gain node only affects local analysis, not what's transmitted.

### 4. Output Volume (Master Volume)

Control the volume of all remote audio elements:

```typescript
setOutputVolume(volume: number): void {
  this.settings.outputVolume = Math.max(0, Math.min(2.0, volume));
  if (!this.isDeafened) {
    this.remoteAudioElements.forEach(audio => {
      audio.volume = this.settings.outputVolume * this.getUserVolume(audio);
    });
  }
}
```

### 5. Per-User Volume Control

Store per-user volume multipliers. The effective volume for a user is `masterVolume * userVolume`.

```typescript
private userVolumes: Map<string, number> = new Map();

setUserVolume(userId: string, volume: number): void {
  this.userVolumes.set(userId, Math.max(0, Math.min(2.0, volume)));
  const audio = this.remoteAudioElements.get(userId);
  if (audio && !this.isDeafened) {
    audio.volume = this.settings.outputVolume * (this.userVolumes.get(userId) ?? 1.0);
  }
}

private getUserVolume(userId: string): number {
  return this.userVolumes.get(userId) ?? 1.0;
}
```

This requires updating `handleRemoteTrack` to apply per-user volume when creating new audio elements, and updating `remoteAudioElements` to store the userId alongside the element (already keyed by userId).

#### UI: Per-User Volume

Right-click on a user in the voice user list to open a context menu / popover with a volume slider (0-200%). This mirrors Discord's per-user volume.

### 6. Noise Suppression

#### Browser Built-in

Toggle the `noiseSuppression` constraint. Requires re-acquiring the media stream:

```typescript
async toggleNoiseSuppression(enabled: boolean): Promise<void> {
  this.settings.noiseSuppression = enabled;
  await this.reacquireAudioStream();
}
```

#### Enhanced: RNNoise via WASM

For higher-quality noise suppression (similar to Discord's Krisp integration), integrate RNNoise compiled to WebAssembly. This runs a neural network noise gate on the audio in real-time.

Libraries:
- `rnnoise-wasm` -- RNNoise compiled to WASM, usable as an AudioWorklet
- `@nicktomlin/rnnoise-wasm` -- alternative packaging

Implementation approach:
1. Load RNNoise WASM module
2. Create an `AudioWorkletNode` that processes audio frames through RNNoise
3. Insert it into the audio pipeline: source -> gain -> rnnoise -> analyser -> destination

```
source -> gainNode -> rnnoiseWorklet -> analyser -> mediaStreamDestination
                                                          |
                                                    processedStream -> mediasoup producer
```

This is a significant chunk of work and could be a separate follow-up. The browser's built-in `noiseSuppression` is a reasonable first step.

### 7. Input Sensitivity / Voice Activity Threshold

The current speaking detection uses a hardcoded threshold of `50`. Make this configurable and add a visual level meter.

```typescript
setSpeakingThreshold(threshold: number): void {
  this.speakingThreshold = Math.max(0, Math.min(255, threshold));
}
```

#### Visual Level Meter

Add a real-time visualization of the input audio level alongside the threshold slider. This helps users calibrate their mic sensitivity.

```typescript
// Expose current audio level for UI rendering
getCurrentAudioLevel(): number {
  if (!this.analyser) return 0;
  const dataArray = new Uint8Array(this.analyser.frequencyBinCount);
  this.analyser.getByteFrequencyData(dataArray);
  return dataArray.reduce((a, b) => a + b) / dataArray.length;
}
```

The UI component renders a horizontal bar that fills based on the current level, with a draggable threshold line overlaid on it.

### 8. Mic Test (Loopback)

Let users hear their own microphone through their selected output device. Create a loopback by connecting the processed audio pipeline to the audio context destination:

```typescript
private loopbackNode: MediaStreamAudioDestinationNode | null = null;
private loopbackAudio: HTMLAudioElement | null = null;

startMicTest(): void {
  if (!this.audioContext || !this.gainNode) return;

  // Connect the gain output to the default audio output
  this.loopbackNode = this.audioContext.createMediaStreamDestination();
  this.gainNode.connect(this.loopbackNode);

  this.loopbackAudio = document.createElement('audio');
  this.loopbackAudio.srcObject = this.loopbackNode.stream;
  this.loopbackAudio.play();

  // Apply output device if set
  if (this.settings.outputDeviceId && typeof this.loopbackAudio.setSinkId === 'function') {
    this.loopbackAudio.setSinkId(this.settings.outputDeviceId);
  }
}

stopMicTest(): void {
  if (this.loopbackNode && this.gainNode) {
    this.gainNode.disconnect(this.loopbackNode);
  }
  if (this.loopbackAudio) {
    this.loopbackAudio.srcObject = null;
    this.loopbackAudio.remove();
    this.loopbackAudio = null;
  }
  this.loopbackNode = null;
}
```

### 9. Audio Settings Persistence

Store all audio settings in `localStorage`:

```typescript
interface AudioSettings {
  inputDeviceId: string | null;
  outputDeviceId: string | null;
  inputGain: number;          // 0.0 - 2.0, default 1.0
  outputVolume: number;       // 0.0 - 2.0, default 1.0
  noiseSuppression: boolean;  // default true
  echoCancellation: boolean;  // default true
  autoGainControl: boolean;   // default true
  speakingThreshold: number;  // 0 - 255, default 50
}

const STORAGE_KEY = 'audio-settings';

function loadAudioSettings(): AudioSettings {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(saved) };
  }
  return { ...DEFAULT_SETTINGS };
}

function saveAudioSettings(settings: AudioSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}
```

Per-user volumes stored separately:

```typescript
const USER_VOLUMES_KEY = 'user-volumes';

function loadUserVolumes(): Record<string, number> {
  const saved = localStorage.getItem(USER_VOLUMES_KEY);
  return saved ? JSON.parse(saved) : {};
}
```

## UI Design: Audio Settings Panel

A settings modal or panel (accessible from the voice controls area) with sections:

### Voice Settings Layout

```
+------------------------------------------+
| VOICE SETTINGS                           |
+------------------------------------------+
|                                          |
| INPUT DEVICE                             |
| [v Default - Built-in Microphone      ]  |
|                                          |
| INPUT VOLUME                             |
| |====[]============| 100%                |
|                                          |
| INPUT SENSITIVITY                        |
| [||||||||     |threshold     ]  Auto     |
| (real-time level meter with threshold)   |
|                                          |
| MIC TEST                                 |
| [ Start Test ]  [ Stop Test ]            |
|                                          |
+------------------------------------------+
|                                          |
| OUTPUT DEVICE                            |
| [v Default - Speakers (HD Audio)      ]  |
|                                          |
| OUTPUT VOLUME                            |
| |====[]============| 100%                |
|                                          |
+------------------------------------------+
|                                          |
| AUDIO PROCESSING                         |
| Echo Cancellation    [ON ]               |
| Noise Suppression    [ON ]               |
| Auto Gain Control    [ON ]               |
|                                          |
+------------------------------------------+
```

### Per-User Volume (Context Menu)

Right-click a user in the voice channel user list:

```
+-------------------------+
| username                |
|-------------------------|
| User Volume             |
| |====[]========| 100%   |
|-------------------------|
| Mute User (local)       |
+-------------------------+
```

## Tauri-Specific Considerations

### Audio Permissions

On macOS, microphone access requires a permission prompt. Tauri handles this through the system's permission dialog. Ensure `tauri.conf.json` includes the microphone usage description:

```json
{
  "bundle": {
    "macOS": {
      "entitlements": {
        "com.apple.security.device.audio-input": true
      }
    }
  }
}
```

### setSinkId Support

- **Windows (WebView2/Chromium)**: Full `setSinkId` support
- **macOS (WebKit)**: Limited -- `setSinkId` may not be available. Fallback: use system audio routing
- **Linux (WebKitGTK)**: Support varies by version. WebKitGTK 2.40+ has better Web Audio support

For platforms where `setSinkId` is not available, output device selection should be hidden or show a message directing users to change output in system settings.

### Native Audio Backend (Future)

For maximum control, a future iteration could use Tauri commands to interact with native audio APIs:

- **Windows**: WASAPI via `cpal` crate
- **macOS**: CoreAudio via `cpal` crate
- **Linux**: PulseAudio/PipeWire via `cpal` crate

This would enable features like:
- True per-application volume control
- Hardware-level device enumeration with full metadata
- Native noise suppression using OS-level APIs
- Better latency control

This is a large effort and the Web Audio API approach covers the common cases.

## Audio Pipeline Diagram

```
Microphone (selected device)
    |
    v
getUserMedia({ audio: { deviceId, constraints } })
    |
    v
MediaStreamSource (Web Audio API)
    |
    v
GainNode (input volume: 0.0 - 2.0)
    |
    +---> [Optional: RNNoise AudioWorklet]
    |
    +---> AnalyserNode (speaking detection + level meter)
    |
    v
MediaStreamDestination
    |
    v
processedStream.getAudioTracks()[0]
    |
    v
mediasoup Producer (sent to SFU)
    |
    v
SFU routes to consumers
    |
    v
mediasoup Consumer (received track)
    |
    v
HTMLAudioElement
    |-- .volume = masterVolume * userVolume
    |-- .setSinkId(outputDeviceId)
    |
    v
Speakers (selected output device)
```

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `frontend/src/lib/audio.ts` | Create | Audio device enumeration, settings management |
| `frontend/src/lib/voice.ts` | Modify | Add gain node pipeline, device selection, per-user volume, mic test |
| `frontend/src/lib/mediasoup.ts` | Modify | Add `replaceAudioTrack()` method |
| `frontend/src/lib/components/AudioSettings.svelte` | Create | Settings panel with device selectors, sliders, toggles |
| `frontend/src/lib/components/UserVolumePopover.svelte` | Create | Per-user volume control (right-click context menu) |
| `frontend/src/lib/components/ChannelList.svelte` | Modify | Add settings button to voice controls, wire up context menu |
| `frontend/src/lib/components/AudioLevelMeter.svelte` | Create | Real-time input level visualization component |

## Implementation Order

1. Create `audio.ts` with device enumeration and settings persistence
2. Refactor `VoiceManager.setupLocalAudio()` to build the gain node pipeline
3. Add `replaceAudioTrack()` to `MediasoupManager`
4. Implement input/output device selection in `VoiceManager`
5. Implement input gain control
6. Implement output volume and per-user volume
7. Add configurable voice activity threshold
8. Build `AudioSettings.svelte` panel with device dropdowns and sliders
9. Build `AudioLevelMeter.svelte` for input sensitivity visualization
10. Add mic test loopback
11. Build `UserVolumePopover.svelte` for per-user volume
12. Wire settings button into `ChannelList.svelte` voice controls
13. Add `localStorage` persistence for all settings
14. Test device hot-swapping (plug/unplug devices while in voice)
15. Test across platforms (Windows WebView2, Linux WebKitGTK, macOS WebKit)

## Device Change Detection

Listen for device changes (user plugs in headphones, disconnects USB mic):

```typescript
navigator.mediaDevices.addEventListener('devicechange', async () => {
  const devices = await getAudioDevices();
  // Update device lists in UI
  // If selected device was removed, fall back to default
  if (this.settings.inputDeviceId) {
    const stillExists = devices.inputs.some(d => d.deviceId === this.settings.inputDeviceId);
    if (!stillExists) {
      await this.setInputDevice('default');
    }
  }
});
```

## References

- [MDN: MediaDevices.enumerateDevices()](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/enumerateDevices)
- [MDN: HTMLMediaElement.setSinkId()](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/setSinkId)
- [MDN: GainNode](https://developer.mozilla.org/en-US/docs/Web/API/GainNode)
- [MDN: AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/AudioWorklet)
- [mediasoup-client: Producer.replaceTrack()](https://mediasoup.org/documentation/v3/mediasoup-client/api/#producer-replaceTrack)
- [RNNoise](https://jmvalin.ca/demo/rnnoise/) -- neural network noise suppression
- [cpal](https://crates.io/crates/cpal) -- cross-platform audio I/O for Rust (future native backend)
