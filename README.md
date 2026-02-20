# Echora

A self-hosted real-time chat platform with text, voice, and screen sharing -- built with Svelte 5 and Rust.

## Features

- **Text Channels** -- Real-time messaging via WebSocket with per-channel broadcasting
- **Voice Channels** -- SFU (Selective Forwarding Unit) architecture using WebRTC
- **Channel Management** -- Create, rename, and delete text/voice channels with real-time sync
- **Message Edit/Delete** -- Edit and delete your own messages with ownership enforcement
- **Online Presence** -- See who is online with real-time connect/disconnect tracking
- **Typing Indicators** -- See when others are typing with debounced indicators
- **File Uploads** -- Attach images, video, audio, and documents to messages with inline previews
- **Authentication** -- JWT-based auth with Argon2 password hashing
- **PostgreSQL** -- Persistent storage with automatic migrations via sqlx
- **Message Pagination** -- Cursor-based pagination with scroll-to-load-more
- **Mobile Responsive** -- Hamburger menu sidebar, adaptive layout for tablet and phone
- **Tauri-Ready** -- Frontend designed for native desktop/mobile via Tauri

## Tech Stack

| Component | Technology                      |
| --------- | ------------------------------- |
| Frontend  | Svelte 5, TypeScript, Vite      |
| Backend   | Rust, Axum 0.8, Tokio           |
| Database  | PostgreSQL, sqlx                |
| Voice     | WebRTC, SFU architecture        |
| Storage   | S3-compatible, local filesystem |
| Auth      | JWT, Argon2                     |
| Desktop   | Tauri                           |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18+)
- [Docker](https://docs.docker.com/get-docker/) (for PostgreSQL)

### 1. Start PostgreSQL

```bash
docker compose up -d
```

### 2. Start the Backend

```bash
cd backend
cargo run
```

The backend starts on `http://127.0.0.1:3000`. Database migrations run automatically on startup, and default channels are seeded if the database is empty.

### 3. Start the Frontend

```bash
cd frontend
npm install
npm run dev
```

The frontend dev server starts on `http://localhost:1420`.

### 4. Tauri Desktop App (Optional)

```bash
cd frontend
npm run tauri:dev
```

## Configuration

Backend environment variables (set in `backend/.env`):

| Variable       | Description                     | Default                                          |
| -------------- | ------------------------------- | ------------------------------------------------ |
| `DATABASE_URL` | PostgreSQL connection string    | `postgres://echora:echora@localhost:5432/echora` |
| `JWT_SECRET`   | Secret key for JWT signing      | (required)                                       |
| `BIND_ADDR`    | Server bind address             | `127.0.0.1:3000`                                 |
| `CORS_ORIGINS` | Comma-separated allowed origins | Permissive (all origins)                         |
| `RUST_LOG`     | Log level filter                | `info`                                           |

#### File Storage

File uploads are disabled by default. Set `STORAGE_BACKEND` to enable.

| Variable                | Description                                                   | Default              |
| ----------------------- | ------------------------------------------------------------- | -------------------- |
| `STORAGE_BACKEND`       | Storage backend: `s3` or `local`                              | (unset = disabled)   |
| `S3_BUCKET`             | S3 bucket name                                                | (required when `s3`) |
| `S3_REGION`             | S3 region                                                     | (required when `s3`) |
| `S3_ENDPOINT`           | Custom S3 endpoint (for DigitalOcean Spaces, MinIO, R2, etc.) | (unset = AWS S3)     |
| `AWS_ACCESS_KEY_ID`     | S3 access key (not needed on ECS/EC2 with IAM roles)          | (from env/IAM)       |
| `AWS_SECRET_ACCESS_KEY` | S3 secret key                                                 | (from env/IAM)       |
| `STORAGE_PATH`          | Local filesystem path for uploads                             | `./uploads`          |

**S3-compatible providers** (DigitalOcean Spaces, MinIO, Backblaze B2, Cloudflare R2, Wasabi, etc.) work by setting `S3_ENDPOINT`:

```bash
# DigitalOcean Spaces example
STORAGE_BACKEND=s3
S3_BUCKET=my-space
S3_REGION=nyc3
S3_ENDPOINT=https://nyc3.digitaloceanspaces.com
AWS_ACCESS_KEY_ID=your-spaces-key
AWS_SECRET_ACCESS_KEY=your-spaces-secret
```

Frontend environment (set in `frontend/.env` / `.env.production`):

| Variable            | Description                      | Default                        |
| ------------------- | -------------------------------- | ------------------------------ |
| `VITE_API_BASE`     | Backend API URL                  | `/api`                         |
| `VITE_WS_BASE`      | WebSocket base URL               | Auto-detected from page URL    |
| `VITE_STUN_SERVERS` | Comma-separated STUN server URLs | `stun:stun.l.google.com:19302` |

## Architecture

### Voice (SFU)

Echora uses a Selective Forwarding Unit architecture for voice chat. The server forwards WebRTC signaling (offers, answers, ICE candidates) between clients without decoding or re-encoding audio. Clients capture audio via `getUserMedia`, establish peer connections through the server, and mix received streams locally. This avoids P2P complexity while keeping server CPU usage low.

### Authentication

Users register/login via REST endpoints. The backend returns a JWT token (7-day expiry) which the frontend stores in localStorage. Protected REST endpoints expect `Authorization: Bearer <token>`. WebSocket endpoints accept the token via query parameter (`?token=...`). Passwords are hashed with Argon2 using a random salt.

### Database

PostgreSQL with sqlx. Migrations run automatically on startup from `backend/migrations/`. IDs use native UUID columns (UUID v7 for time-ordering). Voice state is managed in-memory with DashMaps and is not persisted.

## Deployment

The backend runs as a container behind a load balancer, the frontend is static files served via CDN.

| Component | Approach                               |
| --------- | -------------------------------------- |
| Frontend  | Static hosting + CDN                   |
| Backend   | Containerized (Docker) + load balancer |
| Database  | Managed PostgreSQL                     |
| Storage   | S3-compatible bucket or local disk     |

### Deploy Backend

```bash
docker build -t echora .
# Tag and push to your container registry, then trigger a redeployment
```

### Deploy Frontend

```bash
cd frontend && npm run build
# Sync build/ to your static hosting provider and invalidate CDN cache
```

## Roadmap

- [x] Channel creation/management
- [x] Message editing & deletion
- [x] Typing indicators
- [x] User online presence
- [x] Mobile responsive layout
- [x] Screen sharing
- [x] Markdown & code blocks
- [x] Message replies & threads
- [x] Reactions & custom emoji
- [x] Roles & permissions
- [x] Moderation (kick, ban, mute/timeout, audit log)
- [x] Invite system
- [x] File uploads
- [ ] Message pinning
- [ ] Search
- [ ] Notifications (@mentions, unread indicators)
- [ ] Direct messages & group DMs
- [ ] Polls
- [ ] User profiles & avatars
- [ ] One-click deployment(s)
- [ ] Webhooks

## License

[AGPL-3.0](LICENSE)
