import type { Channel, UserPresence, CustomEmoji } from "../api";

export interface ServerStateStore {
  channels: Channel[];
  onlineUsers: UserPresence[];
  userAvatars: Record<string, string | undefined>;
  userRolesMap: Record<string, string>;
  serverName: string;
  backendVersion: string;
  customEmojis: CustomEmoji[];
}

export const serverState = $state<ServerStateStore>({
  channels: [],
  onlineUsers: [],
  userAvatars: {},
  userRolesMap: {},
  serverName: "",
  backendVersion: "",
  customEmojis: [],
});
