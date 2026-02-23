import type { Channel, UserPresence, MemberInfo, CustomEmoji } from "../api";

export interface ServerStateStore {
  channels: Channel[];
  onlineUsers: UserPresence[];
  members: MemberInfo[];
  userAvatars: Record<string, string | undefined>;
  userRolesMap: Record<string, string>;
  serverName: string;
  backendVersion: string;
  customEmojis: CustomEmoji[];
}

export const serverState = $state<ServerStateStore>({
  channels: [],
  onlineUsers: [],
  members: [],
  userAvatars: {},
  userRolesMap: {},
  serverName: "",
  backendVersion: "",
  customEmojis: [],
});
