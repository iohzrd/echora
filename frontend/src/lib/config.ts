import { browser } from '$app/environment';
import { getActiveServer, isTauri } from './serverManager';

/** Static fallback for API base URL (safe during SSR). */
const FALLBACK_API_BASE = import.meta.env.VITE_API_BASE || '/api';

/** Compute WS fallback lazily since `location` is unavailable during SSR. */
function getFallbackWsBase(): string {
  if (!browser) return '';
  return import.meta.env.VITE_WS_BASE || `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}`;
}

// Static exports for web builds (single-server mode).
export const API_BASE = FALLBACK_API_BASE;
export const WS_BASE = browser ? getFallbackWsBase() : '';

/** Get the API base URL for the currently active server. */
export function getApiBase(): string {
  if (isTauri) {
    const server = getActiveServer();
    if (server) return `${server.url}/api`;
  }
  return FALLBACK_API_BASE;
}

/** Get the WebSocket base URL for the currently active server. */
export function getWsBase(): string {
  if (isTauri) {
    const server = getActiveServer();
    if (server) {
      const url = new URL(server.url);
      const protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
      return `${protocol}//${url.host}`;
    }
  }
  return getFallbackWsBase();
}
