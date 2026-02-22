import { WebSocketManager } from "./api";
import { voiceManager } from "./voice";

// Module-level singleton. Reset via resetWs() on server switch.
let _wsManager = new WebSocketManager();
voiceManager.setWebSocketManager(_wsManager);

export function getWs(): WebSocketManager {
  return _wsManager;
}

export function resetWs(): WebSocketManager {
  _wsManager.disconnect();
  _wsManager = new WebSocketManager();
  voiceManager.setWebSocketManager(_wsManager);
  return _wsManager;
}
