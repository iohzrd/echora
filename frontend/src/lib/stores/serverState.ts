import { writable } from 'svelte/store';
import type { Channel, UserPresence, CustomEmoji } from '../api';

export interface ServerStateStore {
  channels: Channel[];
  onlineUsers: UserPresence[];
  userAvatars: Record<string, string | undefined>;
  userRolesMap: Record<string, string>;
  serverName: string;
  backendVersion: string;
  tauriVersion: string;
  customEmojis: CustomEmoji[];
}

export const serverState = writable<ServerStateStore>({
  channels: [],
  onlineUsers: [],
  userAvatars: {},
  userRolesMap: {},
  serverName: '',
  backendVersion: '',
  tauriVersion: '',
  customEmojis: [],
});
