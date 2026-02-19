import AuthService from './auth';
import { getApiBase, getWsBase } from './config';
import { appFetch } from './serverManager';

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
}

export interface SendMessageRequest {
  content: string;
  reply_to_id?: string;
}

export interface VoiceState {
  user_id: string;
  username: string;
  channel_id: string;
  session_id: string;
  is_muted: boolean;
  is_deafened: boolean;
  is_screen_sharing: boolean;
  joined_at: string;
}

export interface JoinVoiceRequest {
  channel_id: string;
}

export interface UserPresence {
  user_id: string;
  username: string;
  connected_at: string;
}

export const FRONTEND_VERSION = '0.2.5';

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
      throw new Error(err.error || errorMessage);
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

  static async getBackendVersion(): Promise<string> {
    try {
      const response = await appFetch(`${getApiBase()}/health`);
      if (!response.ok) return 'unknown';
      const data = await response.json();
      return data.version || 'unknown';
    } catch {
      return 'unknown';
    }
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
}

export type WsOutgoingMessage =
  | { message_type: 'join'; channel_id: string; content: '' }
  | { message_type: 'typing'; channel_id: string; content: '' }
  | { message_type: 'message'; channel_id: string; content: string; reply_to_id?: string }
  | { message_type: 'voice_state_update'; channel_id: string; is_muted?: boolean; is_deafened?: boolean }
  | { message_type: 'voice_speaking'; channel_id: string; is_speaking: boolean }
  | { message_type: 'screen_share_update'; channel_id: string; is_screen_sharing: boolean };

export interface WsIncomingMessage {
  type: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: any;
}

export class WebSocketManager {
  private ws: WebSocket | null = null;
  private messageHandlers: ((data: WsIncomingMessage) => void)[] = [];
  private intentionalClose = false;
  private reconnectAttempts = 0;
  private maxReconnectDelay = 30000;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private currentChannelId: string | null = null;

  connect(): Promise<void> {
    this.intentionalClose = false;
    const token = AuthService.getToken();
    this.ws = new WebSocket(`${getWsBase()}/ws?token=${token}`);

    return new Promise<void>((resolve) => {
      this.ws!.onopen = () => {
        this.reconnectAttempts = 0;
        if (this.currentChannelId) {
          this.joinChannel(this.currentChannelId);
        }
        resolve();
      };

      this.ws!.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as WsIncomingMessage;
          this.messageHandlers.forEach(handler => handler(data));
        } catch {
          // Ignore malformed messages
        }
      };

      this.ws!.onclose = () => {
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
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  onMessage(handler: (data: WsIncomingMessage) => void) {
    this.messageHandlers.push(handler);
  }

  offMessage(handler: (data: WsIncomingMessage) => void) {
    this.messageHandlers = this.messageHandlers.filter(h => h !== handler);
  }

  private send(message: WsOutgoingMessage) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  joinChannel(channelId: string) {
    this.currentChannelId = channelId;
    this.send({ message_type: 'join', channel_id: channelId, content: '' });
  }

  sendTyping(channelId: string) {
    this.send({ message_type: 'typing', channel_id: channelId, content: '' });
  }

  sendMessage(channelId: string, content: string, replyToId?: string) {
    const message: WsOutgoingMessage = {
      message_type: 'message',
      channel_id: channelId,
      content,
      ...(replyToId ? { reply_to_id: replyToId } : {}),
    };
    this.send(message);
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
}

