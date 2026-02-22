# Media Bot Implementation Research

A media bot that can be given commands to play YouTube/etc links in voice channels, similar to Discord music bots.

---

## Current Architecture (Relevant)

### Backend
- **Framework**: Axum 0.8 (Rust, Tokio async runtime)
- **Database**: PostgreSQL via sqlx 0.8
- **Real-time**: WebSocket at `/ws` with JWT auth, broadcast channels (global + per-channel via DashMap)
- **Voice SFU**: mediasoup 0.20 (Rust bindings) -- handles WebRTC audio/video forwarding between users
- **Storage**: S3 or local filesystem via `object_store`

### Frontend
- **Framework**: Svelte 5 + SvelteKit 2.53, Tauri 2.10 for desktop
- **Voice client**: mediasoup-client 3.18.7, Web Audio API (gain nodes, analysers, VAD)
- **Audio playback**: `sounds.ts` handles notification sounds via Web Audio API AudioContext

### Voice Flow (Current)
1. User calls `POST /api/voice/join` -- creates VoiceState in-memory
2. Frontend MediasoupManager creates send/recv WebRTC transports
3. Audio captured via `getUserMedia()`, produced to SFU
4. Other users consume producers, audio mixed locally on each client

### Key Integration Points
- **AppState** holds: `voice_states: DashMap<Uuid, DashMap<Uuid, VoiceState>>` (channel_id -> user_id -> state)
- **SfuService** manages: workers, routers (one per voice channel), transports, producers, consumers
- **WebSocket events**: `voice_user_joined`, `voice_user_left`, `message`, `typing`, etc.
- **No existing bot infrastructure or plugin system** -- must be built from scratch

---

## Recommended Architecture

```
                    +------------------+
                    |   SvelteKit UI   |
                    |                  |
                    | Player Widget    |  <audio> element
                    | (controls, queue)|  or Web Audio API
                    +--------+---------+
                             |
                    WebSocket (state) + HTTP (audio stream)
                             |
                    +--------+---------+
                    |   Rust Backend   |
                    |   (Axum)         |
                    |                  |
                    | - Bot Service    |  Handles commands, queue, state
                    | - Queue Manager  |  VecDeque<Track> per channel
                    | - Player State   |  Current track, position, paused
                    | - WS Broadcast   |  State sync to all clients
                    | - /api/stream    |  HTTP streaming endpoint
                    +--------+---------+
                             |
                    Spawns child processes
                             |
              +--------------+--------------+
              |                             |
        +-----+------+              +------+-----+
        |   yt-dlp   |              |   ffmpeg   |
        |            |   URL --->   |            |
        | Resolve    |              | Transcode  |
        | audio URL  |              | to Opus/   |
        |            |              | AAC, pipe  |
        +------------+              | to stdout  |
                                    +------------+
```

### Why This Architecture

- **No Lavalink needed** -- Lavalink exists to bridge audio to Discord's voice protocol. We control both server and client.
- **HTTP streaming over WebRTC** -- Music does not need sub-100ms latency. 1-3 seconds of buffer latency is perfectly acceptable. HTTP is simpler, more reliable, and easier to debug.
- **Server-side streaming over client-side URL forwarding** -- YouTube's direct audio stream URLs have CORS headers that prevent browser playback from other origins. URLs also expire quickly. Server proxying is the only reliable approach.
- **WebSocket for state sync** -- Reuses existing infrastructure. Broadcast playback state (now playing, position, pause/resume) to all clients in the channel.

---

## Component Breakdown

### 1. URL Resolution (yt-dlp)

**yt-dlp** is the dominant tool for media extraction, supporting 1,800+ websites (YouTube, Vimeo, TikTok, SoundCloud, Bandcamp, Twitter/X, etc.).

**Integration approach**: Shell out to yt-dlp from Rust via `tokio::process::Command`.

```
yt-dlp -j <url>            # Get metadata as JSON without downloading
yt-dlp -g -f bestaudio <url>  # Get direct audio stream URL
```

**Rust crates**:
- `yt-dlp` crate -- async Rust wrapper around the CLI, parses JSON output
- `ytd-rs` -- simpler wrapper

**No pure-Rust YouTube extractor exists** -- the extraction logic (signatures, throttling, cipher decoding) is extremely complex and changes frequently. yt-dlp (Python) is the only viable maintained option.

**Alternative extraction services** (self-hostable):
- Invidious (Crystal) -- REST API returning stream URLs, scrapes YouTube directly
- Piped (Java) -- similar to Invidious with API access

**Legal considerations**:
- yt-dlp itself is legal (RIAA takedown of youtube-dl was reversed after EFF intervention)
- Downloading/streaming copyrighted content without authorization violates YouTube ToS
- For self-hosted use with a small group, enforcement risk is low
- Safe: personal content, public domain, Creative Commons, user-authorized content

### 2. Audio Transcoding (FFmpeg)

FFmpeg handles transcoding from whatever format yt-dlp provides to a web-friendly format.

**Approach**: Spawn ffmpeg as a child process. It reads from the source URL and pipes transcoded audio to stdout. No intermediate files needed.

```
ffmpeg -i <source_url> -c:a libopus -b:a 128k -f webm pipe:1
# or
ffmpeg -i <source_url> -c:a aac -b:a 128k -f adts pipe:1
```

**Format choice**:
- **Opus in WebM** -- best quality per bitrate, supported in all modern browsers
- **AAC in MP4/ADTS** -- broader compatibility, slightly worse at same bitrate

**Rust FFmpeg crates** (if direct integration is needed later):
| Crate | Approach |
|-------|----------|
| ez-ffmpeg | Safe ergonomic bindings, async support |
| ffmpeg-next | Comprehensive FFmpeg bindings (maintained fork) |
| rsmpeg | Thin safe layer (by ByteDance) |

**Pure Rust audio crates** (for inspection/decoding without ffmpeg):
| Crate | Purpose |
|-------|---------|
| Symphonia | Demuxing + decoding: MP3, AAC, FLAC, Vorbis, WAV, ALAC |
| Rodio | Audio playback (uses Symphonia) |
| opus | Opus codec FFI bindings |
| rubato | Sample rate conversion |

**Performance**: A modern CPU transcodes audio at 100x+ real-time. A 4-minute song takes ~2 seconds. A 4-core server handles 50-100+ simultaneous transcode streams.

### 3. HTTP Audio Streaming

Axum supports streaming response bodies natively. The server endpoint pipes ffmpeg's stdout directly to the HTTP response.

**Endpoint**: `GET /api/channels/{channel_id}/stream`

**Implementation**:
1. Spawn ffmpeg with the resolved audio URL
2. Wrap ffmpeg's stdout in an Axum `StreamBody`
3. Set appropriate headers (`Content-Type: audio/webm`, `Transfer-Encoding: chunked`)
4. Client connects with `<audio src="/api/channels/{id}/stream">` or Web Audio API

**Alternatives considered**:
- **HLS** (segmented HTTP streaming) -- more complex, 2-5s latency, better for large audiences. Overkill for a small chat app.
- **WebRTC** -- sub-500ms latency, complex. Already have the infrastructure via mediasoup, but unnecessary for music.
- **WebSocket audio** -- possible but non-standard, requires manual buffering/jitter handling.

### 4. Queue Management

Per-channel queue stored in backend memory (like voice states).

**Data structures**:
```
BotState per channel:
  - queue: VecDeque<Track>
  - current_track: Option<Track>
  - playback_position: Duration
  - is_paused: bool
  - loop_mode: Off | Track | Queue
  - volume: u8 (0-100)
  - ffmpeg_process: Option<Child>
```

**Track metadata** (from yt-dlp JSON):
```
Track:
  - url: String
  - title: String
  - artist/uploader: String
  - duration: Duration
  - thumbnail_url: Option<String>
  - requested_by: UserId
```

**Standard commands**:

| Command | Description |
|---------|-------------|
| `/play <url/query>` | Add to queue, start if nothing playing |
| `/pause` | Pause playback |
| `/resume` | Resume playback |
| `/skip` | Skip current track |
| `/stop` | Stop playback, clear queue |
| `/queue` | Display current queue |
| `/remove <n>` | Remove track at position |
| `/move <from> <to>` | Reorder tracks |
| `/shuffle` | Randomize queue order |
| `/loop [track/queue/off]` | Set loop mode |
| `/volume <0-100>` | Set volume |
| `/seek <timestamp>` | Jump to position |
| `/nowplaying` | Show current track info |

### 5. State Synchronization

The server broadcasts playback state changes via existing WebSocket infrastructure.

**New WebSocket event types**:
```
bot_track_started   { channel_id, track, position, queue_length }
bot_track_ended     { channel_id, next_track }
bot_paused          { channel_id, position }
bot_resumed         { channel_id, position }
bot_queue_updated   { channel_id, queue }
bot_stopped         { channel_id }
```

**Synchronization approach**:
- Server maintains authoritative playback state (current track, position, paused)
- Broadcasts state on every change
- Clients adjust local `<audio>` element to match server state
- Periodic position sync (every few seconds) to keep clients aligned

### 6. Command Interface

**Three complementary interfaces**:

1. **Text commands in chat** -- `/play <url>`, `/skip`, `/queue`. Parsed server-side from messages starting with `/`. Bot responses appear as system messages in chat.

2. **Player widget in UI** -- persistent panel (bottom bar or sidebar section) showing:
   - Album art / thumbnail
   - Track title and uploader
   - Progress bar with seek
   - Play/pause, skip, stop, volume controls
   - Queue list with drag-to-reorder
   - "Requested by" user attribution

3. **Inline "Now Playing" messages** -- when tracks change, a system message appears in chat showing what's playing. Provides context in chat history.

### 7. Bot Identity

Two approaches:

**A. System-level bot (simpler)**:
- Bot actions appear as system messages (like "user joined voice")
- No separate user account needed
- Commands parsed from regular messages with a prefix

**B. Bot user account (more Discord-like)**:
- Create a special "bot" user type in the database
- Bot has its own identity, avatar, display name
- Bot messages appear as regular messages from the bot user
- More extensible if adding other bots later

Recommendation: Start with system-level (A), migrate to (B) if/when a broader bot framework is needed.

---

## Implementation Phases

### Phase 1: Core Playback
- Add bot service to backend with in-memory state per voice channel
- Implement `/play <url>` command parsing from chat messages
- yt-dlp integration for URL resolution
- ffmpeg integration for transcoding
- HTTP streaming endpoint
- Basic WebSocket events for playback state
- Minimal frontend player (play/pause/skip, current track display)

### Phase 2: Queue and Controls
- Full queue management (add, remove, move, shuffle, clear)
- Loop modes (track, queue, off)
- Volume control
- Seek support
- `/queue` display command
- Frontend queue list UI

### Phase 3: Player Widget
- Persistent player widget in frontend UI
- Progress bar with seek
- Thumbnail/artwork display
- Visual queue with drag-to-reorder
- "Now Playing" inline messages in chat

### Phase 4: Polish
- Search support (`/play <search query>` resolves via yt-dlp search)
- Playlist support (YouTube playlists, SoundCloud sets)
- Vote-skip (require N users to agree)
- DJ role / permission system for bot commands
- Track history / recently played

---

## Server Dependencies

**Required on the server**:
- `yt-dlp` -- installable via pip or standalone binary
- `ffmpeg` -- installable via system package manager

**Rust crates to add**:
- `tokio::process` (already available via tokio) -- for spawning yt-dlp/ffmpeg
- Possibly `yt-dlp` crate for cleaner JSON parsing of metadata
- No new major framework dependencies needed -- Axum handles streaming responses natively

---

## Resource Requirements

| Resource | Per Stream | 100 Listeners |
|----------|-----------|---------------|
| CPU | Fractional core (transcoding at 100x+ real-time) | < 1 core |
| Memory | ~50-100MB per ffmpeg process + negligible per listener | ~100MB + 1.4MB |
| Bandwidth | 128kbps per listener (16 KB/s) | 12.8 Mbps |

A single small VPS ($5-10/month) handles dozens of concurrent streams with ease. The primary cost concern is egress bandwidth on cloud providers that charge per GB.

---

## Alternative Approaches Considered

### Client-Side Playback (URL Forwarding)
Server resolves URL, sends it to clients, clients fetch audio directly from YouTube/etc.
- **Rejected**: YouTube's CORS headers block browser playback from other origins. URLs expire quickly. Unreliable across sources.

### Embedded YouTube iframe
Server resolves video ID, clients render hidden YouTube iframe player.
- **Limited**: Only works for YouTube. May show ads. YouTube ToS may restrict hidden playback. Less control.
- **Could be offered as a supplementary option** alongside server-side streaming.

### WebRTC via Existing SFU
Bot produces audio as a mediasoup producer, users consume it like any other voice participant.
- **Possible but complex**: Would need to create a virtual "participant" that produces audio from ffmpeg into the SFU. More tightly coupled to voice infrastructure. Harder to add player controls (seek, pause affect the RTP stream). Could be explored later as an alternative delivery mechanism.

### Hybrid: WebRTC for Delivery, HTTP for Fallback
Use the existing mediasoup SFU to deliver bot audio (lowest latency), fall back to HTTP streaming.
- **Over-engineered for V1**: Music doesn't need sub-100ms latency. HTTP is simpler and sufficient. Could be a V2 enhancement.
