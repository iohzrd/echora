# Soundboard Feature

Research and implementation plan for a Discord-style soundboard in Echora.

## How Discord's Soundboard Works

### User Experience
- Accessible via a speaker/megaphone icon in voice channel controls
- Users click a sound to play it to everyone in the voice channel
- Sounds organized into tabs: Server sounds, Default sounds, Favorites
- Each sound has a name (2-32 chars), optional emoji, and volume level (0-1)
- Users can star/favorite sounds for quick access across devices

### Sound Constraints
| Constraint       | Value                    |
|------------------|--------------------------|
| Formats          | MP3, WAV, OGG            |
| Max file size    | 512 KB                   |
| Max duration     | 5.2 seconds              |
| Sample rate      | 44.1 kHz recommended     |
| Name length      | 2-32 characters          |

### Per-Server Sound Slots (Discord uses Boost tiers)
| Tier     | Slots |
|----------|-------|
| None     | 8     |
| Level 1  | 24    |
| Level 2  | 36    |
| Level 3  | 48    |

For Echora, we can pick a fixed limit (e.g. 48) or make it configurable per-server.

### Permissions
| Permission                | Controls                                      |
|---------------------------|-----------------------------------------------|
| `USE_SOUNDBOARD`          | Play sounds in voice channels                 |
| `CREATE_GUILD_EXPRESSIONS`| Upload new sounds                             |
| `MANAGE_GUILD_EXPRESSIONS`| Edit/delete any sound (not just your own)     |
| `SPEAK`                   | Required alongside USE_SOUNDBOARD             |

User must be in the voice channel and not deafened/server-muted to play sounds.

### Cooldown
- Per-sound cooldown of ~30 seconds (if anyone in the channel played it recently)
- Standard API rate limits also apply

---

## Technical Architecture (Discord's Approach)

### Key Insight: Client-Side Playback

Discord does NOT inject soundboard audio into the WebRTC/SFU voice stream. Instead:

```
1. User clicks sound
2. HTTP POST /channels/{channel_id}/send-soundboard-sound
3. Server validates permissions & voice state
4. Server dispatches VOICE_CHANNEL_EFFECT_SEND via WebSocket to all clients in channel
5. Each client fetches the audio file from CDN and plays it locally
```

This means:
- No server-side audio mixing needed
- No interaction with the SFU/mediasoup at all
- Bandwidth efficient (each client fetches a small cached file)
- Slight desync between clients is acceptable
- Works even if the triggering user is self-muted

### API Structure (Discord)

**Sound Object:**
```json
{
  "name": "Yay",
  "sound_id": "1106714396018884649",
  "volume": 1.0,
  "emoji_id": "989193655938064464",
  "emoji_name": null,
  "guild_id": "613425648685547541",
  "available": true,
  "user": { "id": "...", "username": "..." }
}
```

**Endpoints:**
| Method | Endpoint                                         | Purpose              |
|--------|--------------------------------------------------|----------------------|
| GET    | `/soundboard-default-sounds`                     | List default sounds  |
| GET    | `/guilds/{id}/soundboard-sounds`                 | List guild sounds    |
| GET    | `/guilds/{id}/soundboard-sounds/{sound_id}`      | Get specific sound   |
| POST   | `/guilds/{id}/soundboard-sounds`                 | Upload sound         |
| PATCH  | `/guilds/{id}/soundboard-sounds/{sound_id}`      | Update sound         |
| DELETE | `/guilds/{id}/soundboard-sounds/{sound_id}`      | Delete sound         |
| POST   | `/channels/{id}/send-soundboard-sound`           | Trigger playback     |

**WebSocket Events:**
| Event                             | When                          |
|-----------------------------------|-------------------------------|
| `VOICE_CHANNEL_EFFECT_SEND`       | Sound played in voice channel |
| `GUILD_SOUNDBOARD_SOUND_CREATE`   | New sound uploaded            |
| `GUILD_SOUNDBOARD_SOUND_UPDATE`   | Sound metadata modified       |
| `GUILD_SOUNDBOARD_SOUND_DELETE`   | Sound deleted                 |

---

## Echora Implementation Plan

### Existing Infrastructure We Can Leverage

1. **File storage** -- Already supports local filesystem and S3 via `object_store` crate (`backend/src/storage.rs`). Used for avatars (5MB), attachments (250MB), and custom emojis (1MB).
2. **WebSocket broadcast** -- Already has per-channel and global broadcasting for voice events in `backend/src/websocket.rs`.
3. **Voice state tracking** -- `AppState.voice_states` DashMap tracks who is in which voice channel, mute/deafen state, etc.
4. **Audio pipeline** -- Frontend already has Web Audio API setup in `voice.ts` with GainNodes and AnalyserNodes. Can reuse output device selection and volume controls.
5. **Permission system** -- Existing role-based permissions can be extended.
6. **Custom emoji upload pattern** -- Almost identical upload flow (small file, validate, store, serve).

### Backend Changes

#### Database Migration

```sql
CREATE TABLE soundboard_sounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(32) NOT NULL,
    volume DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    emoji_id UUID REFERENCES custom_emojis(id) ON DELETE SET NULL,
    emoji_name TEXT,                    -- unicode emoji fallback
    file_size INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT name_length CHECK (char_length(name) BETWEEN 2 AND 32),
    CONSTRAINT volume_range CHECK (volume >= 0.0 AND volume <= 1.0),
    CONSTRAINT file_size_limit CHECK (file_size <= 524288),
    CONSTRAINT duration_limit CHECK (duration_ms <= 5200)
);

CREATE TABLE soundboard_favorites (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sound_id UUID NOT NULL REFERENCES soundboard_sounds(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, sound_id)
);
```

Note: No `room_id`/`guild_id` since Echora is currently single-server. If multi-server support is added later, add a server/guild foreign key.

#### REST Endpoints

```
GET    /api/soundboard                     -- List all sounds
GET    /api/soundboard/{sound_id}          -- Get sound metadata
GET    /api/soundboard/{sound_id}/audio    -- Serve audio file (cacheable)
POST   /api/soundboard                     -- Upload new sound (multipart)
PATCH  /api/soundboard/{sound_id}          -- Update name/emoji/volume
DELETE /api/soundboard/{sound_id}          -- Delete sound + file

POST   /api/soundboard/{sound_id}/play     -- Trigger playback in voice channel
         Body: { "channel_id": "..." }

GET    /api/soundboard/favorites           -- List user's favorites
POST   /api/soundboard/{sound_id}/favorite -- Toggle favorite
```

#### Upload Processing

Follow the same pattern as custom emoji uploads (`backend/src/routes/custom_emojis.rs`):

1. Accept multipart form: audio file + name + optional emoji + volume
2. Validate content type (audio/mpeg, audio/ogg, audio/wav)
3. Validate file size <= 512KB
4. Measure duration using `symphonia` crate (pure Rust audio decoder, no ffmpeg dependency)
5. Validate duration <= 5.2 seconds
6. Store to object store at `soundboard/{sound_id}.{ext}`
7. Insert metadata into database
8. Broadcast `soundboard_sound_created` WebSocket event

#### Play Endpoint Logic

1. Authenticate user
2. Verify user is in a voice channel (check `voice_states`)
3. Verify user is not deafened
4. Check per-sound cooldown (in-memory HashMap with timestamps, ~30 second window)
5. Broadcast `soundboard_play` WebSocket event to all users in that voice channel
6. Return 204 No Content

#### WebSocket Events

```rust
// Server -> Client
"soundboard_play" {
    channel_id: Uuid,
    user_id: Uuid,
    sound_id: Uuid,
    sound_volume: f64,
}

"soundboard_sound_created" { sound: SoundboardSound }
"soundboard_sound_updated" { sound: SoundboardSound }
"soundboard_sound_deleted" { sound_id: Uuid }
```

### Frontend Changes

#### New Components

1. **SoundboardPanel.svelte** -- Main soundboard UI
   - Grid of sound buttons with name, emoji, play icon
   - Tabs: All Sounds | Favorites
   - Search/filter input
   - "Add Sound" button (if user has permission)
   - Opened from voice controls area (next to mute/deafen buttons)

2. **SoundUploadDialog.svelte** -- Upload form
   - File picker (accept=".mp3,.ogg,.wav")
   - Name input (2-32 chars)
   - Emoji picker (reuse existing)
   - Volume slider (0-1)
   - Client-side preview playback
   - File size/duration validation before upload

3. **SoundboardSettings.svelte** -- Admin management view
   - List all sounds with edit/delete controls
   - Slot usage indicator

#### Audio Playback (Client-Side)

When `soundboard_play` WebSocket event is received:

```typescript
async function handleSoundboardPlay(event: SoundboardPlayEvent) {
  // Skip if user is deafened
  if (voiceStore.isDeafened) return;

  const audioUrl = `${getApiBase()}/soundboard/${event.sound_id}/audio`;

  // Use Web Audio API for output device control
  const audioContext = new AudioContext();
  const response = await fetch(audioUrl);
  const buffer = await audioContext.decodeAudioData(await response.arrayBuffer());

  const source = audioContext.createBufferSource();
  source.buffer = buffer;

  const gainNode = audioContext.createGain();
  gainNode.gain.value = event.sound_volume * masterOutputVolume;

  source.connect(gainNode);
  gainNode.connect(audioContext.destination);
  source.start();
}
```

For output device routing, if using HTMLAudioElement instead:

```typescript
const audio = new Audio(audioUrl);
audio.volume = event.sound_volume * masterOutputVolume;
if (selectedOutputDevice) {
  await audio.setSinkId(selectedOutputDevice);
}
audio.play();
```

Consider pre-caching sound files (fetch on panel open, store in a Map) to reduce playback latency.

#### WebSocket Handler Additions

Add to `wsHandlers.ts`:
- `soundboard_play` -- trigger local audio playback
- `soundboard_sound_created` -- add to local sound list
- `soundboard_sound_updated` -- update local sound metadata
- `soundboard_sound_deleted` -- remove from local sound list

#### UI Placement

Add soundboard button to the voice controls bar in VoicePanel.svelte, alongside mute/deafen/disconnect buttons. The panel can slide up from the bottom or appear as a popover.

### Dependencies

**Backend:**
- `symphonia` -- Pure Rust audio decoding for duration measurement (supports MP3, OGG Vorbis, WAV, FLAC)
  - Already using `object_store` for file storage
  - No new infrastructure needed

**Frontend:**
- No new dependencies needed
  - Web Audio API is built into browsers
  - File validation can use existing patterns

### Storage Estimates

- Max 512KB per sound
- At 48 sounds: ~24MB total
- Highly cacheable (immutable content, change = new ID)
- Serve with `Cache-Control: public, max-age=31536000, immutable`

### Key Technical Challenges

1. **Audio duration measurement** -- Must decode the file server-side to measure actual duration. `symphonia` handles this without external dependencies like ffmpeg.

2. **Cooldown tracking** -- Per-sound cooldowns need in-memory tracking. A `DashMap<Uuid, Instant>` (sound_id -> last_played_at) per channel works. Clean up stale entries periodically.

3. **Output device routing** -- Soundboard audio should play through the same output device as voice chat. Reuse the output device selection from `VoiceManager`.

4. **Pre-caching** -- For low-latency playback, cache decoded audio buffers on the client. Fetch all sounds when the user joins a voice channel or opens the soundboard panel.

5. **Concurrent plays** -- Multiple sounds can play simultaneously. Web Audio API handles this naturally (multiple BufferSourceNodes).

6. **Permission checks on play** -- Must be fast since it runs on every play. Voice state lookup is O(1) via DashMap.

### Implementation Order

1. Database migration + model
2. Upload endpoint (with symphonia duration validation)
3. List/get/delete endpoints
4. Audio file serving endpoint
5. Play endpoint + WebSocket broadcast
6. Frontend: SoundboardPanel component
7. Frontend: WebSocket handler for `soundboard_play`
8. Frontend: Sound upload dialog
9. Favorites (lower priority)
10. Cooldown enforcement (lower priority)

### Entrance Sounds (Voice Join Sounds)

Discord also lets users set a soundboard sound to play automatically when they join a voice channel. This is a per-user preference (requires Nitro on Discord, but Echora can offer it to all users).

#### How Discord Does It

- Configured in User Settings > Voice and Video > Soundboard
- User picks a soundboard sound as their "entrance sound"
- Can set one sound for all servers, or customize per-server
- When the user joins a voice channel, the server automatically triggers the sound
- Plays even if the user joins self-muted, but NOT if they join deafened
- Uses the same `VOICE_CHANNEL_EFFECT_SEND` mechanism as manual soundboard plays

#### Echora Implementation

**Database:**
```sql
-- Add column to users table
ALTER TABLE users ADD COLUMN entrance_sound_id UUID REFERENCES soundboard_sounds(id) ON DELETE SET NULL;
```

**Backend logic (in voice join handler):**
1. User calls `POST /api/voice/join`
2. After successfully joining, check if user has `entrance_sound_id` set
3. If set and user is not deafened, broadcast `soundboard_play` event to the channel
4. Same WebSocket event as manual plays -- clients handle it identically

**Frontend:**
- Add entrance sound picker to user settings / audio settings
- Dropdown or grid showing available soundboard sounds + "None" option
- `PATCH /api/auth/profile` with `entrance_sound_id` field

**API:**
```
PATCH /api/auth/profile   -- existing endpoint, add entrance_sound_id field
```

This is lightweight to implement once the core soundboard is in place -- it's just a user preference column plus a few lines in the voice join handler.

---

### Implementation Order (Updated)

1. Database migration + model (soundboard_sounds table)
2. Upload endpoint (with symphonia duration validation)
3. List/get/delete endpoints
4. Audio file serving endpoint
5. Play endpoint + WebSocket broadcast
6. Frontend: SoundboardPanel component
7. Frontend: WebSocket handler for `soundboard_play`
8. Frontend: Sound upload dialog
9. Entrance sounds (user preference + auto-play on join)
10. Favorites (lower priority)
11. Cooldown enforcement (lower priority)

### Out of Scope (for now)

- Default/built-in sounds (can add later with seed data)
- Cross-server sound sharing (Echora is single-server)
- Sound categories/folders
- Animated emoji reactions on play (Discord shows emoji animation)
- Mobile-specific UI
