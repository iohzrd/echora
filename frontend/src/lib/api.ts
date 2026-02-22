import AuthService, { type AuthResponse } from './auth';
import { getApiBase, getWsBase } from './config';
import { appFetch } from './serverManager';

export class ApiError extends Error {
  constructor(message: string, public readonly status: number) {
    super(message);
    this.name = 'ApiError';
  }
}

export interface Channel {
  id: string;
  name: string;
  channel_type: 'text' | 'voice';
}

export interface ReplyPreview {
  id: string;
  author: string;
  content: string;
}

export interface ReactionData {
  emoji: string;
  count: number;
  reacted: boolean;
}

export interface LinkPreview {
  id: string;
  url: string;
  title?: string;
  description?: string;
  image_url?: string;
  site_name?: string;
}

export interface Attachment {
  id: string;
  filename: string;
  content_type: string;
  size: number;
  storage_path: string;
  uploader_id: string;
  message_id?: string;
  created_at: string;
}

export interface Message {
  id: string;
  content: string;
  author: string;
  author_id: string;
  channel_id: string;
  timestamp: string;
  edited_at?: string;
  reply_to_id?: string;
  reply_to?: ReplyPreview;
  reactions?: ReactionData[];
  link_previews?: LinkPreview[];
  attachments?: Attachment[];
}

export interface SendMessageRequest {
  content?: string;
  reply_to_id?: string;
  attachment_ids?: string[];
}

export interface VoiceState {
  user_id: string;
  username: string;
  avatar_url?: string;
  channel_id: string;
  session_id: string;
  is_muted: boolean;
  is_deafened: boolean;
  is_screen_sharing: boolean;
  is_camera_sharing: boolean;
  joined_at: string;
}

export interface JoinVoiceRequest {
  channel_id: string;
}

export interface UserPresence {
  user_id: string;
  username: string;
  avatar_url?: string;
  connected_at: string;
}

export interface UserSummary {
  id: string;
  username: string;
  email: string;
  role: 'owner' | 'admin' | 'moderator' | 'member';
  created_at: string;
  avatar_url?: string;
}

export interface PublicProfile {
  id: string;
  username: string;
  display_name?: string;
  avatar_url?: string;
  role: string;
  created_at: string;
}

export interface Ban {
  id: string;
  user_id: string;
  banned_by: string;
  reason?: string;
  expires_at?: string;
  created_at: string;
}

export interface Mute {
  id: string;
  user_id: string;
  muted_by: string;
  reason?: string;
  expires_at?: string;
  created_at: string;
}

export interface Invite {
  id: string;
  code: string;
  created_by: string;
  max_uses?: number;
  uses: number;
  expires_at?: string;
  revoked: boolean;
  created_at: string;
}

export interface ModLogEntry {
  id: string;
  action: string;
  moderator_id: string;
  target_user_id: string;
  reason?: string;
  details?: string;
  created_at: string;
}

export interface CustomEmoji {
  id: string;
  name: string;
  uploaded_by: string;
  storage_path: string;
  content_type: string;
  created_at: string;
}

export interface ServerSettings {
  [key: string]: string;
}

import { version } from '$app/environment';
export const FRONTEND_VERSION = version;

export class API {
  static async request<T>(
    path: string,
    options: RequestInit = {},
    errorMessage = 'Request failed',
  ): Promise<T> {
    const headers: Record<string, string> = {
      ...AuthService.getAuthHeaders(),
      ...(options.headers as Record<string, string> || {}),
    };
    const response = await appFetch(`${getApiBase()}${path}`, { ...options, headers });
    if (!response.ok) {
      const err = await response.json().catch(() => ({}));
      throw new ApiError(err.error || errorMessage, response.status);
    }
    const text = await response.text();
    return text ? JSON.parse(text) : undefined as T;
  }

  static async jsonRequest<T>(
    path: string,
    method: string,
    body: unknown,
    errorMessage = 'Request failed',
  ): Promise<T> {
    return this.request<T>(path, {
      method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }, errorMessage);
  }

  private static async multipartRequest<T>(
    path: string,
    formData: FormData,
    errorMessage: string,
  ): Promise<T> {
    const headers: Record<string, string> = { ...AuthService.getAuthHeaders() };
    const response = await appFetch(`${getApiBase()}${path}`, {
      method: 'POST',
      headers,
      body: formData,
    });
    if (!response.ok) {
      const err = await response.json().catch(() => ({}));
      throw new ApiError(err.error || errorMessage, response.status);
    }
    return response.json();
  }

  static async getInit(): Promise<{
    server_name: string;
    version: string;
    channels: Channel[];
    online_users: UserPresence[];
    voice_states: VoiceState[];
    users?: UserSummary[];
  }> {
    return this.request('/init', {}, 'Failed to initialize');
  }

  static async getChannels(): Promise<Channel[]> {
    return this.request('/channels', {}, 'Failed to fetch channels');
  }

  static async getMessages(channelId: string, limit = 50, before?: string): Promise<Message[]> {
    const params = new URLSearchParams({ limit: String(limit) });
    if (before) {
      params.set('before', before);
    }
    return this.request(`/channels/${channelId}/messages?${params}`, {}, 'Failed to fetch messages');
  }

  static async sendMessage(channelId: string, message: SendMessageRequest): Promise<Message> {
    return this.jsonRequest(`/channels/${channelId}/messages`, 'POST', message, 'Failed to send message');
  }

  static async createChannel(name: string, channelType: 'text' | 'voice'): Promise<Channel> {
    return this.jsonRequest('/channels', 'POST', { name, channel_type: channelType }, 'Failed to create channel');
  }

  static async updateChannel(channelId: string, name: string): Promise<Channel> {
    return this.jsonRequest(`/channels/${channelId}`, 'PUT', { name }, 'Failed to update channel');
  }

  static async deleteChannel(channelId: string): Promise<void> {
    return this.request(`/channels/${channelId}`, { method: 'DELETE' }, 'Failed to delete channel');
  }

  static async editMessage(channelId: string, messageId: string, content: string): Promise<Message> {
    return this.jsonRequest(`/channels/${channelId}/messages/${messageId}`, 'PUT', { content }, 'Failed to edit message');
  }

  static async deleteMessage(channelId: string, messageId: string): Promise<void> {
    return this.request(`/channels/${channelId}/messages/${messageId}`, { method: 'DELETE' }, 'Failed to delete message');
  }

  static async addReaction(channelId: string, messageId: string, emoji: string): Promise<void> {
    return this.request(
      `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      { method: 'PUT' },
      'Failed to add reaction',
    );
  }

  static async removeReaction(channelId: string, messageId: string, emoji: string): Promise<void> {
    return this.request(
      `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      { method: 'DELETE' },
      'Failed to remove reaction',
    );
  }

  static async getOnlineUsers(): Promise<UserPresence[]> {
    return this.request('/users/online', {}, 'Failed to fetch online users');
  }

  static async joinVoiceChannel(request: JoinVoiceRequest): Promise<VoiceState> {
    return this.jsonRequest('/voice/join', 'POST', request, 'Failed to join voice channel');
  }

  static async leaveVoiceChannel(request: JoinVoiceRequest): Promise<void> {
    return this.jsonRequest('/voice/leave', 'POST', request, 'Failed to leave voice channel');
  }

  static async getVoiceStates(channelId: string): Promise<VoiceState[]> {
    return this.request(`/voice/channels/${channelId}/states`, {}, 'Failed to fetch voice states');
  }

  static async getAllVoiceStates(): Promise<VoiceState[]> {
    return this.request('/voice/states', {}, 'Failed to fetch all voice states');
  }

  // --- Profile ---

  static async updateUsername(username: string): Promise<AuthResponse> {
    return this.jsonRequest('/auth/me', 'PUT', { username }, 'Failed to update username');
  }

  static async updateProfile(data: { username?: string; display_name?: string | null }): Promise<AuthResponse> {
    return this.jsonRequest('/auth/me', 'PUT', data, 'Failed to update profile');
  }

  static async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return this.jsonRequest('/auth/password', 'POST', {
      current_password: currentPassword,
      new_password: newPassword,
    }, 'Failed to change password');
  }

  static async uploadAvatar(file: File): Promise<import('./auth').User> {
    const formData = new FormData();
    formData.append('file', file);
    return this.multipartRequest('/auth/avatar', formData, 'Failed to upload avatar');
  }

  static async deleteAvatar(): Promise<import('./auth').User> {
    return this.request('/auth/avatar', { method: 'DELETE' }, 'Failed to delete avatar');
  }

  static getAvatarUrl(userId: string): string {
    return `${getApiBase()}/users/${userId}/avatar`;
  }

  static async getUserProfile(userId: string): Promise<PublicProfile> {
    return this.request(`/users/${userId}/profile`, {}, 'Failed to load profile');
  }

  // --- Admin: Users ---

  static async getUsers(): Promise<UserSummary[]> {
    return this.request('/admin/users', {}, 'Failed to fetch users');
  }

  static async changeUserRole(userId: string, role: string): Promise<void> {
    return this.jsonRequest(`/admin/users/${userId}/role`, 'PUT', { role }, 'Failed to change role');
  }

  // --- Moderation ---

  static async kickUser(userId: string, reason?: string): Promise<void> {
    return this.jsonRequest('/admin/kick', 'POST', { user_id: userId, reason }, 'Failed to kick user');
  }

  static async banUser(userId: string, reason?: string, durationHours?: number): Promise<void> {
    return this.jsonRequest('/admin/ban', 'POST', {
      user_id: userId, reason, duration_hours: durationHours
    }, 'Failed to ban user');
  }

  static async unbanUser(userId: string): Promise<void> {
    return this.request(`/admin/bans/${userId}`, { method: 'DELETE' }, 'Failed to unban user');
  }

  static async getBans(): Promise<Ban[]> {
    return this.request('/admin/bans', {}, 'Failed to fetch bans');
  }

  static async muteUser(userId: string, reason?: string, durationHours?: number): Promise<void> {
    return this.jsonRequest('/admin/mute', 'POST', {
      user_id: userId, reason, duration_hours: durationHours
    }, 'Failed to mute user');
  }

  static async unmuteUser(userId: string): Promise<void> {
    return this.request(`/admin/mutes/${userId}`, { method: 'DELETE' }, 'Failed to unmute user');
  }

  static async getMutes(): Promise<Mute[]> {
    return this.request('/admin/mutes', {}, 'Failed to fetch mutes');
  }

  // --- Invites ---

  static async createInvite(maxUses?: number, expiresInHours?: number): Promise<Invite> {
    return this.jsonRequest('/invites', 'POST', {
      max_uses: maxUses, expires_in_hours: expiresInHours
    }, 'Failed to create invite');
  }

  static async getInvites(): Promise<Invite[]> {
    return this.request('/invites', {}, 'Failed to fetch invites');
  }

  static async revokeInvite(inviteId: string): Promise<void> {
    return this.request(`/invites/${inviteId}`, { method: 'DELETE' }, 'Failed to revoke invite');
  }

  // --- Settings ---

  static async getSettings(): Promise<ServerSettings> {
    return this.request('/admin/settings', {}, 'Failed to fetch settings');
  }

  static async updateSetting(key: string, value: string): Promise<void> {
    return this.jsonRequest('/admin/settings', 'PUT', { key, value }, 'Failed to update setting');
  }

  // --- Mod Log ---

  static async getModLog(): Promise<ModLogEntry[]> {
    return this.request('/admin/modlog', {}, 'Failed to fetch moderation log');
  }

  // --- Attachments ---

  static async uploadAttachment(file: File): Promise<Attachment> {
    const formData = new FormData();
    formData.append('file', file);
    return this.multipartRequest('/attachments', formData, 'Failed to upload file');
  }

  static getAttachmentUrl(attachmentId: string, filename: string): string {
    return `${getApiBase()}/attachments/${attachmentId}/${encodeURIComponent(filename)}`;
  }

  // --- Custom Emojis ---

  static async getCustomEmojis(): Promise<CustomEmoji[]> {
    return this.request('/custom-emojis', {}, 'Failed to fetch custom emojis');
  }

  static async uploadCustomEmoji(name: string, file: File): Promise<CustomEmoji> {
    const formData = new FormData();
    formData.append('name', name);
    formData.append('file', file);
    return this.multipartRequest('/custom-emojis', formData, 'Failed to upload custom emoji');
  }

  static async deleteCustomEmoji(emojiId: string): Promise<void> {
    return this.request(`/custom-emojis/${emojiId}`, { method: 'DELETE' }, 'Failed to delete custom emoji');
  }

  static getCustomEmojiUrl(emojiId: string): string {
    return `${getApiBase()}/custom-emojis/${emojiId}/image`;
  }
}

export type WsOutgoingMessage =
  | { message_type: 'join'; channel_id: string; content: '' }
  | { message_type: 'typing'; channel_id: string; content: '' }
  | { message_type: 'message'; channel_id: string; content: string; reply_to_id?: string; attachment_ids?: string[] }
  | { message_type: 'voice_state_update'; channel_id: string; is_muted?: boolean; is_deafened?: boolean }
  | { message_type: 'voice_speaking'; channel_id: string; is_speaking: boolean }
  | { message_type: 'screen_share_update'; channel_id: string; is_screen_sharing: boolean }
  | { message_type: 'camera_update'; channel_id: string; is_camera_sharing: boolean }
  | { message_type: 'ping'; channel_id: ''; content: '' };

export type WsIncomingMessage =
  | { type: 'message'; data: Message }
  | { type: 'message_edited'; data: Message }
  | { type: 'message_deleted'; data: { id: string; channel_id: string } }
  | { type: 'channel_created'; data: Channel }
  | { type: 'channel_updated'; data: Channel }
  | { type: 'channel_deleted'; data: { id: string } }
  | { type: 'user_online'; data: UserPresence }
  | { type: 'user_offline'; data: { user_id: string } }
  | { type: 'user_avatar_updated'; data: { user_id: string; avatar_url?: string } }
  | { type: 'user_profile_updated'; data: { user_id: string; username: string; avatar_url?: string } }
  | { type: 'user_renamed'; data: { user_id: string; new_username: string } }
  | { type: 'user_role_changed'; data: { user_id: string; new_role: string } }
  | { type: 'user_kicked'; data: { user_id: string } }
  | { type: 'user_banned'; data: { user_id: string } }
  | { type: 'reaction_added'; data: { message_id: string; emoji: string; user_id: string } }
  | { type: 'reaction_removed'; data: { message_id: string; emoji: string; user_id: string } }
  | { type: 'link_preview_ready'; data: { message_id: string; channel_id: string; link_previews: LinkPreview[] } }
  | { type: 'voice_user_joined'; data: VoiceState }
  | { type: 'voice_user_left'; data: { user_id: string; channel_id: string } }
  | { type: 'voice_state_updated'; data: VoiceState }
  | { type: 'voice_speaking'; data: { user_id: string; channel_id: string; is_speaking: boolean } }
  | { type: 'screen_share_updated'; data: { user_id: string; channel_id: string; is_screen_sharing: boolean } }
  | { type: 'camera_updated'; data: { user_id: string; channel_id: string; is_camera_sharing: boolean } }
  | { type: 'new_producer'; data: { producer_id: string; user_id: string; channel_id: string; label?: string } }
  | { type: 'typing'; data: { user_id: string; channel_id: string; username: string } }
  | { type: 'error'; data: { code: string; message?: string } }
  | { type: 'pong'; data: Record<string, never> };

export class WebSocketManager {
  private ws: WebSocket | null = null;
  private messageHandlers: ((data: WsIncomingMessage) => void)[] = [];
  private reconnectCallbacks: (() => void)[] = [];
  private intentionalClose = false;
  private reconnectAttempts = 0;
  private maxReconnectDelay = 30000;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private currentChannelId: string | null = null;

  connect(): Promise<void> {
    this.intentionalClose = false;
    const token = AuthService.getToken();
    this.ws = new WebSocket(`${getWsBase()}/ws?token=${token}`);

    return new Promise<void>((resolve) => {
      this.ws!.onopen = () => {
        const isReconnect = this.reconnectAttempts > 0;
        this.reconnectAttempts = 0;
        if (this.currentChannelId) {
          this.joinChannel(this.currentChannelId);
        }
        this.startHeartbeat();
        if (isReconnect) {
          this.reconnectCallbacks.forEach(cb => cb());
        }
        resolve();
      };

      this.ws!.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as WsIncomingMessage;
          if (data.type === 'pong') return;
          this.messageHandlers.forEach(handler => handler(data));
        } catch {
          // Ignore malformed messages
        }
      };

      this.ws!.onclose = () => {
        this.stopHeartbeat();
        if (!this.intentionalClose) {
          const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), this.maxReconnectDelay);
          this.reconnectAttempts++;
          this.reconnectTimer = setTimeout(() => this.connect(), delay);
        }
      };

      this.ws!.onerror = () => {
        // Connection errors are handled by onclose
      };
    });
  }

  disconnect() {
    this.intentionalClose = true;
    this.stopHeartbeat();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  onReconnect(callback: () => void) {
    this.reconnectCallbacks.push(callback);
  }

  private startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      this.send({ message_type: 'ping', channel_id: '', content: '' } as WsOutgoingMessage);
    }, 30000);
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  onMessage(handler: (data: WsIncomingMessage) => void) {
    this.messageHandlers.push(handler);
  }

  offMessage(handler: (data: WsIncomingMessage) => void) {
    this.messageHandlers = this.messageHandlers.filter(h => h !== handler);
  }

  private send(message: WsOutgoingMessage): boolean {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
      return true;
    }
    return false;
  }

  joinChannel(channelId: string) {
    this.currentChannelId = channelId;
    this.send({ message_type: 'join', channel_id: channelId, content: '' });
  }

  sendTyping(channelId: string) {
    this.send({ message_type: 'typing', channel_id: channelId, content: '' });
  }

  sendMessage(channelId: string, content: string, replyToId?: string, attachmentIds?: string[]): boolean {
    const message: WsOutgoingMessage = {
      message_type: 'message',
      channel_id: channelId,
      content,
      ...(replyToId ? { reply_to_id: replyToId } : {}),
      ...(attachmentIds?.length ? { attachment_ids: attachmentIds } : {}),
    };
    return this.send(message);
  }

  sendVoiceStateUpdate(channelId: string, update: { is_muted?: boolean; is_deafened?: boolean }) {
    this.send({ message_type: 'voice_state_update', channel_id: channelId, ...update });
  }

  sendVoiceSpeaking(channelId: string, isSpeaking: boolean) {
    this.send({ message_type: 'voice_speaking', channel_id: channelId, is_speaking: isSpeaking });
  }

  sendScreenShareUpdate(channelId: string, isScreenSharing: boolean) {
    this.send({ message_type: 'screen_share_update', channel_id: channelId, is_screen_sharing: isScreenSharing });
  }

  sendCameraUpdate(channelId: string, isCameraSharing: boolean) {
    this.send({ message_type: 'camera_update', channel_id: channelId, is_camera_sharing: isCameraSharing });
  }
}

