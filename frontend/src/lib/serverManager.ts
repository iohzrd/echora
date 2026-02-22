import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';

export interface EchoraServer {
  id: string;
  name: string;
  url: string;
  token?: string;
  userId?: string;
  username?: string;
  addedAt: string;
  lastConnectedAt?: string;
}

export interface ServerStore {
  servers: EchoraServer[];
  activeServerId: string | null;
}

const STORAGE_KEY = 'echora_servers';

export const isTauri = browser && '__TAURI__' in window;

/**
 * Fetch wrapper that routes through Tauri's HTTP plugin in desktop mode.
 * Desktop apps don't share the browser's same-origin policy, so the
 * Rust-side HTTP client is the standard approach for cross-origin requests.
 */
export async function appFetch(input: string | URL | Request, init?: RequestInit): Promise<Response> {
  if (isTauri) {
    const { fetch: tauriFetch } = await import('@tauri-apps/plugin-http');
    return tauriFetch(input, init);
  }
  return fetch(input, init);
}

function generateId(): string {
  return crypto.randomUUID();
}

function loadFromStorage(): ServerStore {
  if (!browser) return { servers: [], activeServerId: null };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch {
    // Corrupted data, start fresh
  }
  return { servers: [], activeServerId: null };
}

function saveToStorage(store: ServerStore): void {
  if (!browser) return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
}

const initialStore = loadFromStorage();

export const serverStore = writable<ServerStore>(initialStore);

// Save to localStorage on every change
serverStore.subscribe(saveToStorage);

export const activeServer = derived(serverStore, ($store) =>
  $store.servers.find(s => s.id === $store.activeServerId) ?? null
);

export const servers = derived(serverStore, ($store) => $store.servers);

export function getActiveServer(): EchoraServer | null {
  return get(activeServer);
}

export function addServer(url: string, name: string): EchoraServer {
  // Normalize URL: strip trailing slashes
  const normalizedUrl = url.replace(/\/+$/, '');

  const server: EchoraServer = {
    id: generateId(),
    name,
    url: normalizedUrl,
    addedAt: new Date().toISOString(),
  };

  serverStore.update(store => ({
    ...store,
    servers: [...store.servers, server],
  }));

  return server;
}

export function removeServer(id: string): void {
  serverStore.update(store => {
    const updated: ServerStore = {
      servers: store.servers.filter(s => s.id !== id),
      activeServerId: store.activeServerId === id ? null : store.activeServerId,
    };
    // If we removed the active server, select the first remaining one
    if (updated.activeServerId === null && updated.servers.length > 0) {
      updated.activeServerId = updated.servers[0].id;
    }
    return updated;
  });
}

export function updateServer(id: string, updates: Partial<EchoraServer>): void {
  serverStore.update(store => ({
    ...store,
    servers: store.servers.map(s =>
      s.id === id ? { ...s, ...updates } : s
    ),
  }));
}

export function setActiveServer(id: string): void {
  serverStore.update(store => ({
    ...store,
    activeServerId: id,
  }));

  // Update lastConnectedAt
  updateServer(id, { lastConnectedAt: new Date().toISOString() });
}

async function checkHealth(baseUrl: string): Promise<{ ok: true; version: string; name?: string; baseUrl: string } | { ok: false }> {
  try {
    const response = await appFetch(`${baseUrl}/api/health`, {
      method: 'GET',
      // connectTimeout is forwarded to Tauri's HTTP plugin; signal is used by the browser fetch.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ...(isTauri ? { connectTimeout: 10000 } as any : { signal: AbortSignal.timeout(10000) }),
    });
    if (!response.ok) return { ok: false };
    const data = await response.json();
    if (!data.version) return { ok: false };
    return { ok: true, version: data.version, name: data.name || undefined, baseUrl };
  } catch {
    return { ok: false };
  }
}

/**
 * Validate that a URL points to a running Echora instance.
 * Tries the URL directly first, then falls back to `api.{hostname}` for
 * split-domain setups (e.g. echnaos.com -> api.echnaos.com).
 * Returns the resolved API base URL so the caller can store the correct one.
 */
export async function validateServerUrl(url: string): Promise<{ valid: boolean; resolvedUrl?: string; name?: string; error?: string }> {
  const normalizedUrl = url.replace(/\/+$/, '');

  // Try the URL as-is first
  const direct = await checkHealth(normalizedUrl);
  if (direct.ok) {
    return { valid: true, resolvedUrl: direct.baseUrl, name: direct.name };
  }

  // Try api.{hostname} for split-domain setups
  try {
    const parsed = new URL(normalizedUrl);
    if (!parsed.hostname.startsWith('api.')) {
      const apiUrl = `${parsed.protocol}//api.${parsed.hostname}${parsed.port ? ':' + parsed.port : ''}`;
      const apiFallback = await checkHealth(apiUrl);
      if (apiFallback.ok) {
        return { valid: true, resolvedUrl: apiFallback.baseUrl, name: apiFallback.name };
      }
    }
  } catch {
    // Invalid URL
  }

  return { valid: false, error: 'Could not reach server' };
}
