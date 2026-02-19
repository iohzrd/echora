# Tauri Multi-Server Support

Design notes for allowing the Echora Tauri app to connect to multiple self-hosted Echora instances, similar to how Discord lets users join different servers.

## Current State

- The frontend connects to a single backend configured at build time via environment variables (`VITE_API_BASE`, `VITE_WS_BASE`) in `frontend/src/lib/config.ts`
- Auth tokens are stored in `localStorage` under a single key (`echora_token`)
- The `API` class and `WebSocketManager` use these global config values directly -- there is no concept of switching between different backend hosts
- Each Echora instance is fully independent: separate database, separate users, separate channels, separate auth (JWT)
- A user must register separately on each instance they want to use

## Goal

Allow a single Tauri desktop client to:

1. Save multiple Echora instance URLs (servers)
2. Switch between them without re-entering connection details
3. Maintain separate auth credentials per instance
4. Show a server list sidebar (similar to Discord's left-hand server icon strip)
5. Optionally display unread indicators and notifications across all connected servers

## Data Model

### Server Entry

```typescript
interface EchoraServer {
  id: string;             // UUID, generated client-side
  name: string;           // User-chosen display name (e.g., "Home Server", "Work")
  url: string;            // Base URL (e.g., "https://echora.example.com")
  iconUrl?: string;       // Optional server icon (could be fetched from instance)
  token?: string;         // JWT auth token for this instance (null if not logged in)
  userId?: string;        // User ID on this instance
  username?: string;      // Username on this instance
  addedAt: string;        // ISO timestamp
  lastConnectedAt?: string;
}
```

### Server List Storage

```typescript
interface ServerStore {
  servers: EchoraServer[];
  activeServerId: string | null;  // Currently selected server
}
```

Store in `localStorage` under key `echora_servers` (or use Tauri's `tauri-plugin-store` for native encrypted storage on desktop).

## Architecture Changes

### 1. Config Module (`config.ts`)

Replace the static config exports with a reactive, switchable configuration:

```typescript
// Instead of static exports:
// export const API_BASE = import.meta.env.VITE_API_BASE || '/api';

// Provide a function that returns config for the active server:
export function getApiBase(): string {
  const server = getActiveServer();
  if (server) return `${server.url}/api`;
  return import.meta.env.VITE_API_BASE || '/api';
}

export function getWsBase(): string {
  const server = getActiveServer();
  if (server) {
    const url = new URL(server.url);
    const protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${url.host}`;
  }
  return import.meta.env.VITE_WS_BASE
    || `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}`;
}
```

### 2. Auth Module (`auth.ts`)

Auth must become server-scoped. Instead of a single `echora_token` in localStorage, tokens are stored per-server in the `ServerStore`:

```typescript
class AuthService {
  // Token key becomes server-specific
  private static getTokenKey(serverId: string): string {
    return `echora_token_${serverId}`;
  }

  static getToken(): string | null {
    const server = getActiveServer();
    if (!server) return localStorage.getItem('echora_token'); // fallback
    return server.token ?? null;
  }

  static setAuth(serverId: string, authResponse: AuthResponse) {
    updateServer(serverId, {
      token: authResponse.token,
      userId: authResponse.user.id,
      username: authResponse.user.username,
    });
    // Also set reactive stores for the active server
    token.set(authResponse.token);
    user.set(authResponse.user);
  }

  static logout() {
    const server = getActiveServer();
    if (server) {
      updateServer(server.id, { token: undefined, userId: undefined, username: undefined });
    }
    token.set(null);
    user.set(null);
  }
}
```

### 3. API Module (`api.ts`)

The `API` class must read the base URL dynamically rather than from a module-level constant:

```typescript
export class API {
  static async request<T>(path: string, options: RequestInit = {}, errorMessage = 'Request failed'): Promise<T> {
    const apiBase = getApiBase(); // Dynamic per active server
    const headers: Record<string, string> = {
      ...AuthService.getAuthHeaders(),
      ...(options.headers as Record<string, string> || {}),
    };
    const response = await fetch(`${apiBase}${path}`, { ...options, headers });
    // ... rest unchanged
  }
}
```

The `WebSocketManager` similarly needs to use `getWsBase()` dynamically in its `connect()` method.

### 4. Server Manager Module (New: `serverManager.ts`)

Central module for CRUD operations on the server list:

```typescript
import { writable, derived } from 'svelte/store';

export const serverStore = writable<ServerStore>(loadFromStorage());

export const activeServer = derived(serverStore, ($store) =>
  $store.servers.find(s => s.id === $store.activeServerId) ?? null
);

export function addServer(url: string, name: string): EchoraServer { ... }
export function removeServer(id: string): void { ... }
export function updateServer(id: string, updates: Partial<EchoraServer>): void { ... }
export function setActiveServer(id: string): void { ... }
export function getActiveServer(): EchoraServer | null { ... }
```

### 5. WebSocket Lifecycle

When switching servers:

1. Disconnect the current WebSocket
2. Leave any active voice channel on the current server
3. Update `activeServerId` in the store
4. Load auth state for the new server
5. If authenticated on the new server, connect WebSocket and load channels
6. If not authenticated, show login/register form for that server

### 6. Voice State Isolation

Voice connections are per-server. When switching servers:

- If the user is in a voice channel on Server A and switches to Server B, the voice connection to Server A should remain active (user stays in voice)
- The UI should indicate that voice is active on another server
- The user can only be in one voice channel across all servers at a time (or optionally allow multiple -- design decision)

## UI Design

### Server Sidebar (Tauri Only)

The server sidebar is **exclusively a Tauri desktop feature**. The web version is always served by and connected to a single Echora instance, so it has no server list -- it renders the existing channel/message layout unchanged.

In the Tauri build, detect the runtime environment and conditionally render a narrow vertical strip on the far left (before the channel list):

```
+---+------------------+------------------------+
| S |  Channels        |  Messages              |
| e |                  |                         |
| r |  # general       |  [message content]      |
| v |  # random        |                         |
| e |  # dev           |                         |
| r |                  |                         |
| s |  Voice           |                         |
|   |  > Lounge        |                         |
|   |                  |                         |
| + |                  |                         |
+---+------------------+------------------------+
```

Runtime detection:

```typescript
// Check if running inside Tauri
const isTauri = '__TAURI__' in window;
```

- Each server shown as a circular icon (first letter of name if no icon)
- Active server highlighted
- "+" button at bottom to add a new server
- Right-click context menu: Edit, Remove, Copy URL
- Unread dot indicator on servers with unread messages
- Green ring on servers where user is in a voice channel
- **Not rendered at all** in the web browser build -- the app behaves as single-server

### Add Server Dialog

Simple modal with:

- Server URL input field (e.g., `https://echora.example.com`)
- Display name input (auto-populated from server's `/api/health` response if possible)
- "Connect" button that validates the URL by hitting `/api/health`
- On success: show login/register form for that instance
- On failure: show error message (unreachable, not an Echora instance, etc.)

### Server Discovery (Optional Future Feature)

- Public server directory (would require a separate discovery service)
- Invite links that encode the server URL (e.g., `echora://join/example.com`)
- QR codes for mobile

### Web vs Tauri

- **Tauri (desktop)**: Full multi-server support. Server sidebar visible. Server list persisted via `tauri-plugin-store` (encrypted). All server management UI (add/remove/switch) is available.
- **Web browser**: Single-server only. The web frontend is served by a specific Echora instance and always connects to that instance's backend. No server sidebar, no server manager, no multi-server storage. The app behaves exactly as it does today. All multi-server code paths are gated behind `'__TAURI__' in window` checks.

## Tauri-Specific Features

### tauri-plugin-store

Use the Tauri store plugin for persistent, optionally encrypted storage of server credentials:

```bash
# Install
cd frontend/src-tauri
cargo add tauri-plugin-store

cd frontend
npm install @tauri-apps/plugin-store
```

```typescript
import { Store } from '@tauri-apps/plugin-store';

const store = new Store('servers.json');

// Save server list
await store.set('servers', serverList);
await store.save();

// Load server list
const servers = await store.get<EchoraServer[]>('servers');
```

This is preferable to localStorage because:
- Data persists across app updates
- Can be encrypted at rest
- Stored in the OS-appropriate app data directory

### Notification Badges

On desktop (Tauri), unread messages across servers could:
- Update the dock/taskbar badge count
- Show native OS notifications with server name context
- Flash the taskbar icon

### Deep Links / Protocol Handler

Register `echora://` as a custom protocol so server invite links work:

```
echora://connect/echora.example.com
```

Tauri supports custom protocol registration via `tauri-plugin-deep-link`.

## Implementation Order

1. Create `serverManager.ts` with store, CRUD operations, and persistence
2. Refactor `config.ts` to use dynamic `getApiBase()` / `getWsBase()`
3. Refactor `auth.ts` to scope tokens per server
4. Refactor `api.ts` and `WebSocketManager` to use dynamic config
5. Build the server sidebar UI component
6. Build the "Add Server" dialog with URL validation
7. Handle server switching (disconnect/reconnect WebSocket, reload channels)
8. Handle voice state across server switches
9. Add unread indicators per server (requires tracking last-read message per server per channel)
10. Add `tauri-plugin-store` for native credential storage (desktop only)
11. Add `tauri-plugin-deep-link` for `echora://` protocol handling
12. Test: add/remove/switch between multiple servers
13. Test: auth isolation (logging out of one server does not affect others)
14. Test: voice channel behavior during server switch

## Backend Considerations

The backend does not need significant changes. Each Echora instance is independent and unaware of the multi-server client feature. However, a few optional enhancements would help:

- **`GET /api/health`**: Already exists. Could be extended to return a server display name and icon URL, so the client can auto-populate these when adding a server.
- **CORS**: Each instance must have its CORS policy configured to allow the Tauri app's origin (or use permissive CORS, which is already the default).
- **Server metadata endpoint**: Optional new `GET /api/server/info` returning `{ name, description, icon_url, user_count }` for the "Add Server" dialog preview.

## References

- [tauri-plugin-store](https://v2.tauri.app/plugin/store/)
- [tauri-plugin-deep-link](https://v2.tauri.app/plugin/deep-link/)
- [Discord Server List UX](https://support.discord.com/hc/en-us/articles/360034842871)
