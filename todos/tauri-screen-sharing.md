# Tauri Native Screen Sharing Integration

Research notes for enhancing Echora's screen sharing with Tauri 2 desktop features.

## Current State

- Tauri v2 is set up in `frontend/src-tauri/` with minimal configuration (only `tauri-plugin-opener`)
- Screen sharing uses `navigator.mediaDevices.getDisplayMedia()` in the webview
- mediasoup SFU supports **VP8 only** for video (plus Opus for audio) -- see `backend/src/sfu/codec.rs`
- `getDisplayMedia()` works reliably on Windows (WebView2/Chromium) but has issues on macOS (WKWebView, broken on Sonoma 14.0+) and Linux (WebKitGTK has no WebRTC support)

## Problem

`getDisplayMedia()` behavior varies by platform in Tauri's webview:

| Platform | Webview     | getDisplayMedia() | Notes                                              |
|----------|-------------|-------------------|----------------------------------------------------|
| Windows  | WebView2    | Works             | System picker may be cropped if window is small     |
| macOS    | WKWebView   | Broken            | Returns NotAllowedError on Sonoma 14.0+             |
| Linux    | WebKitGTK   | Does not work     | No WebRTC support in WebKitGTK                      |

## Native Screen Capture -- Linux / Wayland

Wayland is the priority Linux target. Wayland compositors do not allow direct framebuffer access like X11 -- screen capture must go through the **XDG Desktop Portal** D-Bus interface, which prompts the user for consent and provides a **PipeWire** stream of the selected source.

This is the same mechanism used by OBS, Firefox, and Chrome on Wayland.

### ashpd (Recommended for Linux)

Low-level Rust bindings for XDG Desktop Portal. Mature, well-maintained (by a GNOME developer), widely used.

```rust
use ashpd::desktop::screencast::{CursorMode, PersistMode, Screencast, SourceType};

// Create a screencast session
let proxy = Screencast::new().await?;
let session = proxy.create_session().await?;

// Request screen/window selection (shows portal picker dialog)
proxy
    .select_sources(
        &session,
        CursorMode::Embedded,
        SourceType::Monitor | SourceType::Window,
        true,           // multiple sources
        None,           // restore token
        PersistMode::DoNot,
    )
    .await?;

// Start the screencast -- returns PipeWire node IDs and a PipeWire fd
let response = proxy.start(&session, None).await?;
let streams = response.streams();     // Vec<Stream> with node_id, size, etc.
let pipewire_fd = response.fd()?;     // RawFd to the PipeWire remote

// Use pipewire_fd + node_id to connect a PipeWire client and receive frames
```

Key points:
- Works on all Wayland compositors (GNOME, KDE, Sway, Hyprland, etc.)
- The portal handles the user consent dialog natively
- Returns a PipeWire file descriptor + stream node IDs
- You then connect a PipeWire client to actually receive video frames

Crate: [ashpd on crates.io](https://crates.io/crates/ashpd) / [GitHub](https://github.com/bilelmoussaoui/ashpd)

### lamco-pipewire (PipeWire frame consumer)

High-performance PipeWire integration for consuming the streams that ashpd/portal provides.

- Zero-copy DMA-BUF support for hardware-accelerated frame transfer
- Handles multiple monitor streams concurrently
- Automatic format negotiation with fallbacks
- Built-in YUV conversion (NV12, I420, YUY2 to BGRA)
- Separate cursor tracking

Crate: [lamco-pipewire on crates.io](https://crates.io/crates/lamco-pipewire)

### lamco-portal (Higher-level alternative)

Wraps ashpd + lamco-pipewire into a single high-level API for screen capture.

- Screen capture through PipeWire streams via XDG Desktop Portal
- Multi-monitor support
- Input injection for remote desktop scenarios

Architecture: `your app -> lamco-portal -> ashpd -> XDG Desktop Portal (D-Bus) -> PipeWire + Compositor`

Crate: [lamco-portal on crates.io](https://crates.io/crates/lamco-portal)

### scap

Cross-platform capture crate (ScreenCaptureKit on macOS, Windows.Graphics.Capture on Windows, PipeWire on Linux). Claims PipeWire support but **Wayland support has open issues** (GitHub issue #158 "Running on Linux / Wayland / Niri" is unresolved). Use with caution on Wayland -- may work on some compositors but not reliably across all.

Crate: [scap on GitHub](https://github.com/CapSoftware/scap)

### xcap

Cross-platform capture with X11 + Wayland support, but Wayland features are marked with limited support indicators in the README. Requires libpipewire and libwayland dev packages. Video recording is WIP.

Crate: [xcap on GitHub](https://github.com/nashaofu/xcap)

### Recommended approach for Linux

Use **ashpd** for the portal interaction (source selection, getting the PipeWire fd) and either **lamco-pipewire** or raw `pipewire-rs` for consuming frames from the PipeWire stream. This is the most reliable path because it uses the exact same mechanism as OBS and browsers on Wayland.

```
User clicks "Share Screen"
  -> ashpd: create screencast session, select_sources(), start()
  -> Portal shows native picker dialog (compositor-provided)
  -> User selects monitor/window
  -> ashpd returns PipeWire fd + stream node_id
  -> lamco-pipewire / pipewire-rs: connect to PipeWire, receive frames
  -> Encode frames to VP8 in Rust (SFU only supports VP8 -- see codec.rs)
  -> Stream to frontend via tauri::ipc::Channel
  -> Frontend creates MediaStreamTrack, feeds to mediasoup
```

## Native Screen Capture -- macOS

Use **ScreenCaptureKit** (macOS 12.3+). scap wraps this well on macOS.

Requires `Info.plist` entry:
```xml
<key>NSScreenCaptureUsageDescription</key>
<string>Echora needs screen recording access for screen sharing</string>
```

## Native Screen Capture -- Windows

Use **Windows.Graphics.Capture**. scap wraps this well on Windows. Alternatively, `getDisplayMedia()` works fine in WebView2 so native capture may not be needed.

## Streaming Frames via IPC

`tauri::ipc::Channel` is the fastest mechanism for streaming binary data from Rust to the frontend. Avoids JSON serialization overhead.

### Rust side

```rust
use tauri::ipc::Channel;

#[tauri::command]
async fn start_native_capture(
    target_id: u32,
    on_frame: Channel<Vec<u8>>,
) {
    // After getting PipeWire stream via ashpd...
    // Receive frames, encode, and stream to frontend
    loop {
        let frame = receive_pipewire_frame();
        let encoded = encode_to_vp8(&frame);
        on_frame.send(encoded).unwrap();
    }
}
```

### Frontend side

```typescript
import { invoke, Channel } from '@tauri-apps/api/core';

const onFrame = new Channel<Uint8Array>();
onFrame.onmessage = (frameData) => {
    // Decode with WebCodecs VideoDecoder, render to canvas,
    // create MediaStreamTrack via MediaStreamTrackGenerator,
    // feed to mediasoup.produceScreen()
};

await invoke('start_native_capture', { targetId: selectedSource.id, onFrame });
```

### Performance note

Raw 1080p30 BGRA frames are ~240MB/s. Must encode to **VP8** in Rust (via `vpx` crate) before sending through the channel -- our SFU only supports VP8 for video (`backend/src/sfu/codec.rs`). H.264 will not work without adding it to the router's codec list. With DMA-BUF support (lamco-pipewire), frames can stay in GPU memory until encoding, avoiding CPU copies.

## Picture-in-Picture Window

Tauri's multi-window API supports floating always-on-top windows for a PiP screen share viewer.

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

const pip = new WebviewWindow('screen-share-pip', {
    url: '/screen-share-viewer',
    title: 'Screen Share',
    width: 480,
    height: 270,
    alwaysOnTop: true,
    decorations: false,
    resizable: true,
    skipTaskbar: true,
});
```

Key window methods:
- `setAlwaysOnTop(true)` -- float above other windows
- `setDecorations(false)` -- remove title bar for clean PiP look
- `setContentProtected(true)` -- prevent re-capture of the PiP window
- `setIgnoreCursorEvents(true)` -- click-through overlay mode

Inter-window communication via events:
```typescript
// Main window
import { emit } from '@tauri-apps/api/event';
await emit('screen-frame-update', { userId, producerId });

// PiP window
import { listen } from '@tauri-apps/api/event';
await listen('screen-frame-update', (event) => { /* update video */ });
```

## System Tray Integration

Built-in tray API (no plugin needed). Show screen sharing status and provide quick stop action.

```rust
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};

let stop_sharing = MenuItem::with_id(app, "stop-share", "Stop Screen Sharing", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&stop_sharing])?;

TrayIconBuilder::new()
    .icon(app.default_window_icon().unwrap().clone())
    .tooltip("Echora - Screen sharing active")
    .menu(&menu)
    .on_menu_event(|app, event| match event.id.as_ref() {
        "stop-share" => { /* trigger stop */ }
        _ => {}
    })
    .build(app)?;
```

Dynamic icon updates when screen sharing starts/stops:
```rust
tray.set_icon(Some(sharing_icon))?;
tray.set_tooltip(Some("Echora - Screen sharing active"))?;
```

## Notifications

```bash
cargo add tauri-plugin-notification
npm add @tauri-apps/plugin-notification
```

```typescript
import { sendNotification } from '@tauri-apps/plugin-notification';

sendNotification({
    title: 'Screen Share Started',
    body: 'User X is sharing their screen in #general',
});
```

## Recommended Architecture

### Approach A: Hybrid (Recommended starting point)

Keep `getDisplayMedia()` for Windows where it works. Add native Rust capture for macOS/Linux.

1. Detect platform via `@tauri-apps/api/os`
2. **Windows:** Use current `getDisplayMedia()` path (no changes needed)
3. **Linux (Wayland priority):** Invoke Tauri commands that:
   - Use `ashpd` to create a screencast session and show the portal picker
   - Receive frames via PipeWire (`lamco-pipewire` or `pipewire-rs`)
   - Encode to VP8 in Rust
   - Stream encoded frames via `tauri::ipc::Channel`
   - Frontend creates `MediaStreamTrack` from frames (WebCodecs `VideoDecoder` + `MediaStreamTrackGenerator`)
   - Feed track to `mediasoup.produceScreen()`
4. **macOS:** Same as Linux path but using `scap` (which wraps ScreenCaptureKit)

### Approach B: Full Native Capture (Maximum control, Discord-like)

Replace `getDisplayMedia()` entirely on all platforms:

1. Platform-specific capture backends in `frontend/src-tauri/`:
   - Linux: `ashpd` + `lamco-pipewire` (XDG Portal + PipeWire)
   - macOS: `scap` (ScreenCaptureKit)
   - Windows: `scap` (Windows.Graphics.Capture) or keep `getDisplayMedia()`
2. Tauri commands: `list_capture_sources()`, `start_capture(target_id, on_frame)`, `stop_capture()`
3. Custom source picker in Svelte (Discord-style with window thumbnails)
4. PiP viewer via `WebviewWindow` with `alwaysOnTop`
5. Tray icon showing sharing status with "Stop Sharing" menu item

### Dependencies to add to `frontend/src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-notification = "2"
vpx = "0.3"                              # VP8 encoding (SFU only supports VP8)

[target.'cfg(target_os = "linux")'.dependencies]
ashpd = "0.10"                          # XDG Desktop Portal (source selection)
pipewire = "0.8"                        # PipeWire frame consumption
# OR
# lamco-pipewire = "0.1"               # Higher-level PipeWire wrapper

[target.'cfg(any(target_os = "macos", target_os = "windows"))'.dependencies]
scap = "0.0.8"                          # ScreenCaptureKit / Windows.Graphics.Capture
```

### Capabilities to add to `frontend/src-tauri/capabilities/default.json`

```json
{
    "permissions": [
        "core:default",
        "opener:default",
        "notification:default"
    ]
}
```

### Linux system dependencies (for building)

```bash
# Arch / Manjaro
sudo pacman -S pipewire libpipewire dbus

# Debian / Ubuntu
sudo apt install libpipewire-0.3-dev libdbus-1-dev
```

## References

- [Tauri getDisplayMedia issue #2600](https://github.com/tauri-apps/tauri/issues/2600)
- [WKWebView WebRTC issue #1101](https://github.com/tauri-apps/wry/issues/1101)
- [ashpd -- XDG Portal bindings](https://github.com/bilelmoussaoui/ashpd)
- [ashpd screencast API docs](https://docs.rs/ashpd/latest/ashpd/desktop/screencast/index.html)
- [XDG Desktop Portal ScreenCast spec](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
- [lamco-pipewire](https://crates.io/crates/lamco-pipewire)
- [lamco-portal](https://crates.io/crates/lamco-portal)
- [scap crate](https://github.com/CapSoftware/scap) (Wayland support incomplete -- issue #158)
- [xcap crate](https://github.com/nashaofu/xcap) (Wayland support limited)
- [Tauri WebviewWindow API](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/)
- [Tauri IPC docs](https://v2.tauri.app/develop/calling-rust/)
- [Tauri System Tray docs](https://v2.tauri.app/learn/system-tray/)
