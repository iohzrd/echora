# Tauri Native Push-to-Talk

Research notes for adding push-to-talk (PTT) capabilities to Echora using Tauri 2 global shortcuts.

## Current State

- Voice chat works via mediasoup SFU with WebRTC audio
- Mute/unmute is toggle-based (`VoiceManager.toggleMute()` in `frontend/src/lib/voice.ts`)
- Mute works by pausing the mediasoup audio producer AND disabling the local audio track
- Speaking detection uses Web Audio API `AnalyserNode` with frequency analysis (threshold: 50)
- No keyboard shortcuts exist for any voice controls
- Tauri setup is minimal: only `tauri-plugin-opener` is installed

## Goal

Add a push-to-talk mode where the user holds a configurable key to transmit audio. When the key is released, audio is muted. This requires global hotkey detection (works even when the app is not focused), which is only possible in the Tauri desktop build.

## Plugin: tauri-plugin-global-shortcut

The official Tauri 2 global shortcut plugin supports both `Pressed` and `Released` event states, making it suitable for push-to-talk.

### Installation

```bash
# Rust
cd frontend/src-tauri
cargo add tauri-plugin-global-shortcut

# JavaScript
cd frontend
npm install @tauri-apps/plugin-global-shortcut
```

### Plugin Registration (Rust)

```rust
// frontend/src-tauri/src/lib.rs
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())  // Add this
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Capabilities

Add to `frontend/src-tauri/capabilities/default.json`:

```json
{
  "permissions": [
    "core:default",
    "opener:default",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-is-registered"
  ]
}
```

### JavaScript API

```typescript
import { register, unregister, isRegistered } from '@tauri-apps/plugin-global-shortcut';

// Register PTT key with press/release detection
await register('Space', (event) => {
  if (event.state === 'Pressed') {
    // Unmute -- user is speaking
    voiceManager.setPTTActive(true);
  } else if (event.state === 'Released') {
    // Mute -- user stopped speaking
    voiceManager.setPTTActive(false);
  }
});

// Unregister when switching back to voice activation mode
await unregister('Space');
```

### ShortcutEvent Interface

```typescript
interface ShortcutEvent {
  shortcut: string;   // The triggered shortcut (e.g., "Space")
  id: number;         // Unique event ID
  state: 'Pressed' | 'Released';
}
```

## Implementation Plan

### 1. Voice Mode State

Add a voice input mode concept to `VoiceManager`:

```typescript
type VoiceInputMode = 'voice-activity' | 'push-to-talk';

// New properties on VoiceManager
private inputMode: VoiceInputMode = 'voice-activity';
private pttKey: string = 'Space';  // Default PTT key
private pttActive = false;         // Whether PTT key is currently held
```

### 2. New VoiceManager Methods

```typescript
// Set voice input mode
setInputMode(mode: VoiceInputMode): void {
  this.inputMode = mode;
  if (mode === 'push-to-talk') {
    // Start muted, only transmit when key held
    this.setMuted(true);
  } else {
    // Voice activity: unmute, rely on speaking detection
    this.setMuted(false);
  }
  this.onStateChanged?.();
}

// Called by PTT key handler
setPTTActive(active: boolean): void {
  if (this.inputMode !== 'push-to-talk') return;
  this.pttActive = active;
  this.setMuted(!active);  // Muted when key NOT held
}

// Internal mute control (shared by toggleMute and PTT)
private setMuted(muted: boolean): void {
  this.isMuted = muted;

  if (this.mediasoup) {
    if (muted) {
      this.mediasoup.pauseAudioProducer();
    } else {
      this.mediasoup.resumeAudioProducer();
    }
  }

  if (this.localStream) {
    this.localStream.getAudioTracks().forEach(track => {
      track.enabled = !muted;
    });
  }

  if (this.currentChannelId && this.ws) {
    this.ws.sendVoiceStateUpdate(this.currentChannelId, { is_muted: muted });
  }

  this.onStateChanged?.();
}
```

### 3. PTT Manager (New File)

Create `frontend/src/lib/ptt.ts` to encapsulate PTT hotkey logic and gracefully handle non-Tauri environments (web browser fallback):

```typescript
import { voiceManager } from './voice';

let isTauri = false;
let currentKey: string | null = null;

// Dynamic import so it doesn't break in browser builds
async function getGlobalShortcut() {
  try {
    const mod = await import('@tauri-apps/plugin-global-shortcut');
    isTauri = true;
    return mod;
  } catch {
    return null;
  }
}

export async function registerPTTKey(key: string): Promise<boolean> {
  const gs = await getGlobalShortcut();
  if (!gs) return false;  // Not running in Tauri

  // Unregister previous key if any
  if (currentKey) {
    await gs.unregister(currentKey);
  }

  await gs.register(key, (event) => {
    if (event.state === 'Pressed') {
      voiceManager.setPTTActive(true);
    } else if (event.state === 'Released') {
      voiceManager.setPTTActive(false);
    }
  });

  currentKey = key;
  return true;
}

export async function unregisterPTTKey(): Promise<void> {
  if (!currentKey) return;
  const gs = await getGlobalShortcut();
  if (!gs) return;
  await gs.unregister(currentKey);
  currentKey = null;
}

export function isTauriApp(): boolean {
  return isTauri;
}
```

### 4. Speaking Detection in PTT Mode

In PTT mode, speaking detection should still run for the visual speaking indicator, but the mute state is controlled entirely by the PTT key, not by audio levels. The existing `startSpeakingDetection()` already respects `this.isMuted` (line 227: `!this.isMuted`), so when PTT is inactive (muted), no speaking events will fire -- this is correct behavior.

### 5. UI Changes

#### Settings/Preferences

Add voice settings (could be a modal or section in a settings page):

- Voice input mode toggle: "Voice Activity" / "Push to Talk"
- PTT key binding selector (only shown when PTT mode selected)
- PTT key recorder: "Press a key..." button that captures the next keypress
- Visual indicator showing current PTT key

#### ChannelList.svelte Voice Controls

- Show PTT indicator when in PTT mode (e.g., "PTT: Space" label)
- Mute button behavior changes: in PTT mode, clicking mute should temporarily override PTT (force mute regardless of key state)

#### Visual Feedback While Transmitting

- PTT active indicator on the user's own voice panel
- Could pulse or highlight the mic icon when PTT key is held
- Optional: play a subtle activation/deactivation sound

### 6. Persistence

Store voice settings in `localStorage`:

```typescript
interface VoiceSettings {
  inputMode: 'voice-activity' | 'push-to-talk';
  pttKey: string;
}

// Load on app start
const saved = localStorage.getItem('voice-settings');
if (saved) {
  const settings: VoiceSettings = JSON.parse(saved);
  voiceManager.setInputMode(settings.inputMode);
  if (settings.inputMode === 'push-to-talk') {
    registerPTTKey(settings.pttKey);
  }
}
```

### 7. Key Binding Format

The global shortcut plugin uses a specific format for key identifiers:

- Single keys: `Space`, `CapsLock`, `F1`-`F24`, `Backquote`, etc.
- With modifiers: `Control+Space`, `Alt+T`, `CommandOrControl+Shift+P`
- Modifier keys alone are NOT valid shortcuts

Common PTT key choices:
- `Space` (most common, but conflicts with typing)
- Mouse buttons (not supported by global-shortcut plugin)
- `CapsLock` (popular in gaming)
- `Backquote` (tilde key, common in games)
- Function keys (`F1`-`F12`)

### 8. Web Fallback

When running in the browser (not Tauri), PTT can still work with limited scope using standard keyboard events, but only when the app window is focused:

```typescript
// Fallback for non-Tauri (browser) PTT
document.addEventListener('keydown', (e) => {
  if (e.code === pttKeyCode && !e.repeat) {
    voiceManager.setPTTActive(true);
  }
});
document.addEventListener('keyup', (e) => {
  if (e.code === pttKeyCode) {
    voiceManager.setPTTActive(false);
  }
});
```

This is inferior to the Tauri global shortcut because it only works when the browser tab is focused. The Tauri version works system-wide.

## Platform Notes

| Platform | Global Shortcuts | Notes |
|----------|-----------------|-------|
| Windows  | Works           | Full support via WebView2 |
| macOS    | Works           | May need Accessibility permission for some key combos |
| Linux    | Works           | X11 fully supported; Wayland via XDG GlobalShortcuts portal |

### Wayland

Global shortcuts on Wayland go through the `org.freedesktop.portal.GlobalShortcuts` portal interface. Works on GNOME 44+, KDE Plasma 5.27+, and other modern compositors. The portal prompts for user consent on first registration.

## Dependencies Summary

### Rust (frontend/src-tauri/Cargo.toml)

```toml
[dependencies]
tauri-plugin-global-shortcut = "2"
```

### JavaScript (frontend/package.json)

```json
{
  "dependencies": {
    "@tauri-apps/plugin-global-shortcut": "^2.0.0"
  }
}
```

## Architecture Diagram

```
User holds PTT key
  -> OS captures global hotkey
  -> tauri-plugin-global-shortcut fires ShortcutEvent { state: "Pressed" }
  -> Frontend JS handler calls voiceManager.setPTTActive(true)
  -> VoiceManager.setMuted(false)
     -> mediasoup producer resumed
     -> local audio track enabled
     -> WebSocket voice_state_update { is_muted: false }
  -> Speaking detection picks up audio, fires voice_speaking events
  -> Other users hear audio

User releases PTT key
  -> tauri-plugin-global-shortcut fires ShortcutEvent { state: "Released" }
  -> Frontend JS handler calls voiceManager.setPTTActive(false)
  -> VoiceManager.setMuted(true)
     -> mediasoup producer paused
     -> local audio track disabled
     -> WebSocket voice_state_update { is_muted: true }
  -> Speaking indicator clears
```

## Implementation Order

1. Install `tauri-plugin-global-shortcut` (Rust + JS)
2. Update capabilities in `default.json`
3. Register plugin in `lib.rs`
4. Add `setMuted()` and `setPTTActive()` to `VoiceManager`
5. Add `inputMode` state and `setInputMode()` to `VoiceManager`
6. Create `ptt.ts` module with `registerPTTKey()` / `unregisterPTTKey()`
7. Add voice settings UI (mode toggle + key binding)
8. Add PTT visual indicator in voice controls
9. Add localStorage persistence for voice settings
10. Add browser fallback for non-Tauri usage
11. Test: PTT works when app is not focused (Tauri only)
12. Test: switching between voice-activity and PTT modes
13. Test: PTT key rebinding

## References

- [Tauri Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [Plugin JS API Reference](https://v2.tauri.app/reference/javascript/global-shortcut/)
- [Plugin Rust Crate](https://crates.io/crates/tauri-plugin-global-shortcut)
- [tauri-plugin-global-shortcut docs.rs](https://docs.rs/tauri-plugin-global-shortcut)
- [XDG GlobalShortcuts Portal (Wayland)](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)
