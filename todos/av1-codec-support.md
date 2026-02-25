# AV1 Codec Support

Research notes for adding AV1 video codec to EchoCell's mediasoup SFU.

## Current State

- mediasoup 0.20.0 and mediasoup-types 0.2.1 (our versions) have `MimeTypeVideo::AV1`
- Currently only VP8 is configured in `backend/src/sfu/codec.rs`
- mediasoup-client negotiates codecs automatically -- no frontend changes needed
- AV1 should be added **alongside** VP8, not replacing it

## Backend Change

Add to `backend/src/sfu/codec.rs` in `create_default_codecs()`:

```rust
RtpCodecCapability::Video {
    mime_type: MimeTypeVideo::AV1,
    preferred_payload_type: None,
    clock_rate: NonZeroU32::new(90000).unwrap(),
    parameters: RtpCodecParametersParameters::default(),
    rtcp_feedback: vec![
        RtcpFeedback::Nack,
        RtcpFeedback::NackPli,
        RtcpFeedback::CcmFir,
        RtcpFeedback::GoogRemb,
        RtcpFeedback::TransportCc,
    ],
},
```

Same clock rate and RTCP feedback as VP8. No special parameters required.

Codec order in the vec determines preference -- put AV1 before VP8 to prefer it, or after VP8 to use it only as a secondary option.

## Browser Support for AV1 WebRTC Encode

| Browser | AV1 WebRTC Encode | Status | Notes |
|---|---|---|---|
| Chrome 113+ | Yes | Stable | Software + hardware encode |
| Firefox 135+ | Yes | Stable, default on | libaom software encode |
| Safari 18.4+ | Experimental | Hardware-gated | Only M3+/A17 Pro+ devices |
| WebKitGTK | Unknown | Experimental | Depends on GStreamer AV1 plugins |

Chrome and Firefox cover the vast majority of users. Safari and WebKitGTK will fall back to VP8.

## AV1 vs VP8 Performance

### Compression
- AV1 achieves ~50-75% bitrate reduction vs VP8 at equivalent quality
- Screen sharing specifically sees ~45% better compression (Chrome's measurements)

### Encoding CPU cost
- AV1 is 10-30x more CPU-intensive than VP8 to encode
- Chrome's libaom "Speed 10" preset is optimized for real-time WebRTC and runs 25% faster than VP9
- VP8 remains lighter on CPU -- important for low-end devices

### Decoding
- AV1 decoding is more demanding than VP8 but well within modern hardware capability
- Hardware AV1 decode available on most GPUs from 2020+

## SVC (Scalable Video Coding)

AV1 supports SVC scalability modes for adaptive quality. Common modes:

- **L1T1** -- single layer, no scalability (start here)
- **L1T3** -- 1 spatial, 3 temporal layers (adaptive framerate)
- **L2T1, L2T3** -- 2 spatial layers (adaptive resolution)
- **L3T3** -- full spatial + temporal scalability

To use SVC when producing:
```typescript
const producer = await sendTransport.produce({
    track,
    encodings: [{ scalabilityMode: 'L1T3' }],
    codecOptions: { videoGoogleStartBitrate: 1000 },
});
```

AV1 uses the DependencyDescriptor RTP header extension (instead of VP8's frame-marking). mediasoup 0.20.0 handles DD forwarding automatically.

## Rust Crates for Native AV1 Encoding (Tauri screen capture)

### SVT-AV1 (Recommended for real-time)

Fastest software AV1 encoder. Presets 10-12 can do real-time 1080p30.

- `svt-av1-enc` -- safe Rust wrapper ([crates.io](https://crates.io/crates/svt-av1-enc))
- `svt-av1-rs` -- higher-level bindings ([GitHub](https://github.com/rust-av/svt-av1-rs))
- Requires system SVT-AV1 library (`libsvtav1-dev` / `svt-av1`)

### rav1e (Pure Rust, simpler integration)

- Pure Rust, no C dependencies, memory-safe
- Too slow for real-time 1080p30 -- suitable for 720p or lower
- Crate: [rav1e](https://crates.io/crates/rav1e) (v0.8.1)

### Hardware encode (NVENC / VAAPI)

- NVIDIA RTX 4000+ and Intel Arc / AMD RDNA3+ have AV1 hardware encoders
- No pure Rust crates wrap these directly
- Accessible through `gstreamer-rs` or `ffmpeg-sys` bindings
- Trivially fast but adds heavy native dependencies

### Recommendation

Use **SVT-AV1** for native capture encoding. For the Tauri dependencies:

```toml
[dependencies]
svt-av1-enc = "0.5"    # Real-time AV1 encoding (presets 10-12)
vpx = "0.3"            # VP8 fallback encoding
```

## Implementation Plan

1. **Phase 1 -- Add AV1 to router (trivial)**
   - Add AV1 codec entry to `codec.rs` after VP8
   - Browsers that support AV1 will auto-negotiate it
   - VP8 remains the fallback -- zero risk of breaking existing clients

2. **Phase 2 -- Prefer AV1 (optional)**
   - Move AV1 before VP8 in the codec list to prefer it
   - Or use `setCodecPreferences()` on the frontend to control per-client

3. **Phase 3 -- SVC support (optional)**
   - Start with L1T3 for adaptive framerate on screen shares
   - Requires frontend change to pass `scalabilityMode` in encodings

4. **Phase 4 -- Native Tauri encoding (when implementing native capture)**
   - Add SVT-AV1 to Tauri Rust dependencies
   - Encode captured frames to AV1 instead of VP8 where supported
   - Keep VP8 encoding as fallback for clients that don't support AV1

## References

- [mediasoup AV1 support (v3.16.0+)](https://github.com/versatica/mediasoup/blob/v3/CHANGELOG.md)
- [mediasoup-types MimeTypeVideo docs](https://docs.rs/mediasoup-types/0.2.1/mediasoup_types/rtp_parameters/enum.MimeTypeVideo.html)
- [Chrome AV1 WebRTC blog](https://developer.chrome.com/blog/av1)
- [Firefox WebRTC 2025 -- AV1 default on](https://blog.mozilla.org/webrtc/firefox-webrtc-2025/)
- [Firefox bug 1921154 -- AV1 WebRTC](https://bugzilla.mozilla.org/show_bug.cgi?id=1921154)
- [Safari 18.4 AV1 support](https://webkit.org/blog/16574/webkit-features-in-safari-18-4/)
- [rav1e](https://github.com/xiph/rav1e)
- [svt-av1-rs](https://github.com/rust-av/svt-av1-rs)
- [setCodecPreferences cross-browser](https://blog.mozilla.org/webrtc/cross-browser-support-for-choosing-webrtc-codecs/)
- [MDN -- WebRTC codecs](https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/WebRTC_codecs)
