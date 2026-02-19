# File/Image Uploads via S3-Compatible Storage

## Context

Echora needs file and image upload support. The solution must work on any VPS -- not coupled to AWS. Production runs in ephemeral containers (no local disk). The approach uses S3-compatible object storage, which works with AWS S3, MinIO, Backblaze B2, Cloudflare R2, or any S3-compatible provider. For local dev, MinIO runs in Docker alongside PostgreSQL.

## Design: Two-Phase Upload

1. User selects file(s) -> `POST /api/upload` (multipart) -> returns attachment metadata
2. User sends message with `attachment_ids` -> WS or REST -> backend links attachments to message

This decouples upload from messaging, enabling upload progress tracking and multiple files before sending.

## Storage: Download Proxy

`GET /api/files/{attachment_id}/{filename}` fetches from S3 and proxies to the client. This avoids public bucket policies, works with any S3 backend, and enforces auth. Response includes `Cache-Control: private, max-age=86400, immutable` for browser caching.

---

## Backend Changes

### 1. Dependencies (`backend/Cargo.toml`)

Add:
```toml
aws-sdk-s3 = "1"
aws-config = "1"
mime_guess = "2"
```

Update existing `axum-extra` to add multipart feature:
```toml
axum-extra = { version = "0", features = ["typed-header", "multipart"] }
```

### 2. Migration (`backend/migrations/20250105000000_add_attachments.sql`)

```sql
CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT REFERENCES messages(id) ON DELETE CASCADE,
    uploader_id TEXT NOT NULL REFERENCES users(id),
    filename TEXT NOT NULL,
    size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    s3_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_attachments_message_id ON attachments(message_id);
```

`message_id` is nullable -- set to NULL on upload, then updated when message is sent. `ON DELETE CASCADE` cleans up when messages are deleted.

### 3. New module: `backend/src/storage.rs`

S3 client wrapper with:
- `StorageService::new()` -- reads env vars, builds S3 client with optional custom endpoint (for MinIO), creates bucket if missing
- `upload(key, body, content_type)` -- PutObject
- `download(key)` -- GetObject, returns bytes + content type
- `delete(key)` -- DeleteObject

Env vars:
- `S3_ENDPOINT` -- custom endpoint URL (omit for real AWS S3)
- `S3_BUCKET` -- bucket name (required)
- `S3_REGION` -- default `us-east-1`
- `S3_ACCESS_KEY_ID` / `S3_SECRET_ACCESS_KEY` -- credentials
- `MAX_UPLOAD_SIZE` -- default 10MB

S3 key format: `attachments/{uuid}/{filename}`

### 4. Update `backend/src/models.rs`

- Add `Attachment` struct (id, message_id, uploader_id, filename, size, mime_type, s3_key, created_at)
- Add `attachments: Option<Vec<Attachment>>` to `Message` (skip_serializing_if None)
- Add `attachment_ids: Option<Vec<Uuid>>` to `SendMessageRequest`
- Add `storage: Arc<StorageService>` to `AppState`

### 5. Update `backend/src/database.rs`

New functions:
- `create_attachment(pool, attachment)` -- INSERT with message_id = NULL
- `assign_attachments_to_message(pool, message_id, attachment_ids, uploader_id)` -- UPDATE SET message_id WHERE id IN (...) AND uploader_id matches AND message_id IS NULL
- `get_attachments_for_messages(pool, message_ids)` -- batch fetch, returns HashMap (same pattern as reactions)
- `get_attachment_by_id(pool, id)` -- for download endpoint
- `get_attachment_s3_keys_for_message(pool, message_id)` -- for cleanup on delete

Modify existing:
- `get_messages()` -- batch-fetch attachments after messages (like reactions)
- `delete_channel()` -- attachments cascade via messages->attachments FK

### 6. Update `backend/src/routes.rs`

New endpoints:
- `POST /api/upload` -- multipart handler, validates size/type, uploads to S3, creates attachment rows, returns `Vec<Attachment>`
- `GET /api/files/{attachment_id}/{filename}` -- fetches from S3, proxies with proper Content-Type and Content-Disposition headers

Modify:
- `send_message()` -- relax empty content validation (allow attachment-only messages), call `assign_attachments_to_message` if attachment_ids provided
- `delete_message()` -- fetch S3 keys before delete, delete S3 objects after DB delete

### 7. Update `backend/src/websocket.rs`

- Add `attachment_ids: Option<Vec<Uuid>>` to `WebSocketMessage`
- In Message handler: assign attachments to message, fetch them, include in broadcast
- Relax content validation: allow empty content if attachments present

### 8. Update `backend/src/main.rs`

- Add `mod storage;`
- Initialize `StorageService` before `AppState`
- Register `/api/upload` and `/api/files/{attachment_id}/{filename}` routes
- Add body size limit layer on upload route

---

## Infrastructure Changes

### docker-compose.yml

Add MinIO service:
```yaml
minio:
  image: minio/minio:latest
  restart: unless-stopped
  environment:
    MINIO_ROOT_USER: minioadmin
    MINIO_ROOT_PASSWORD: minioadmin
  ports:
    - "9000:9000"
    - "9001:9001"
  command: server /data --console-address ":9001"
  volumes:
    - miniodata:/data
```

Add `miniodata:` to volumes.

### backend/.env

Add:
```
S3_ENDPOINT=http://localhost:9000
S3_BUCKET=echora
S3_REGION=us-east-1
S3_ACCESS_KEY_ID=minioadmin
S3_SECRET_ACCESS_KEY=minioadmin
MAX_UPLOAD_SIZE=10485760
```

---

## Frontend Changes

### `frontend/src/lib/api.ts`

- Add `Attachment` interface (id, filename, size, mime_type, s3_key, created_at)
- Add `attachments?: Attachment[]` to `Message`
- Add `API.uploadFiles(files, onProgress)` -- uses XMLHttpRequest for progress tracking
- Add `API.getFileUrl(attachmentId, filename)` -- returns `/api/files/{id}/{filename}`
- Extend `WebSocketManager.sendMessage()` to accept optional `attachmentIds`

### `frontend/src/lib/components/MessageInput.svelte`

- Hidden `<input type="file" multiple>` with attach button (+) next to textarea
- Drag-and-drop on input area (dragover/drop handlers with visual feedback)
- Paste support (check clipboardData for files on paste event)
- Upload files immediately on selection, show progress bar
- Pending attachments preview row above textarea (thumbnails for images, filename for others, X to remove)
- `onSend` callback changes to `(text: string, attachmentIds: string[]) => void`
- `onUpload: (files: File[]) => Promise<Attachment[]>` prop for upload handling

### `frontend/src/lib/components/MessageList.svelte`

After message text, before reactions:
- Images: inline `<img>` preview (max 400x300), clickable to open full size
- Video: `<video>` with controls
- Audio: `<audio>` with controls
- Other files: download link with filename + size
- `formatFileSize()` helper (B/KB/MB)

### `frontend/src/routes/+page.svelte`

- `handleSendMessage(text, attachmentIds)` -- pass attachment IDs to WS sendMessage
- `handleUpload(files)` -- calls `API.uploadFiles()`
- Wire new props to MessageInput and MessageList

### `frontend/src/app.css`

- `.attachment-previews` -- pending uploads row above input
- `.upload-progress` / `.upload-progress-bar` -- progress indicator
- `.attach-btn` -- file picker button
- `.input-row` -- flex row for button + textarea
- `.message-attachments` -- attachment display in messages
- `.attachment-image` -- inline image preview (max 400x300)
- `.attachment-file` -- file download link card
- `.message-input-area.dragover` -- drag-and-drop highlight

---

## Files to modify/create

**New:**
- `backend/migrations/20250105000000_add_attachments.sql`
- `backend/src/storage.rs`

**Backend modify:**
- `backend/Cargo.toml`
- `backend/src/models.rs`
- `backend/src/database.rs`
- `backend/src/routes.rs`
- `backend/src/websocket.rs`
- `backend/src/main.rs`

**Infrastructure modify:**
- `docker-compose.yml`
- `backend/.env`

**Frontend modify:**
- `frontend/src/lib/api.ts`
- `frontend/src/lib/components/MessageInput.svelte`
- `frontend/src/lib/components/MessageList.svelte`
- `frontend/src/routes/+page.svelte`
- `frontend/src/app.css`

## Verification

1. `docker compose up -d` -- PostgreSQL + MinIO running
2. `cd backend && cargo check && cargo fmt && cargo clippy` -- no errors
3. `cd frontend && npm run check && npm run build` -- no errors
4. Manual testing:
   - Upload an image via the attach button -- preview appears in input area
   - Send message with image -- inline preview renders in chat
   - Upload a non-image file -- download link renders in chat
   - Drag-and-drop a file onto the input area -- uploads and attaches
   - Paste an image from clipboard -- uploads and attaches
   - Send a message with only attachments (no text) -- works
   - Delete a message with attachments -- attachments cleaned from S3
   - Other users see attachments in real-time via WebSocket
