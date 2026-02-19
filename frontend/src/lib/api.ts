import AuthService from './auth';

const API_BASE = import.meta.env.VITE_API_BASE || '/api';

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

export interface UpdateVoiceStateRequest {
  is_muted?: boolean;
  is_deafened?: boolean;
}

export interface UpdateSpeakingRequest {
  is_speaking: boolean;
}

export interface UserPresence {
  user_id: string;
  username: string;
  connected_at: string;
}

export const FRONTEND_VERSION = '0.2.5';

export class API {
  static async getBackendVersion(): Promise<string> {
    try {
      const response = await fetch(`${API_BASE}/health`);
      if (!response.ok) return 'unknown';
      const data = await response.json();
      return data.version || 'unknown';
    } catch {
      return 'unknown';
    }
  }

  static async getChannels(): Promise<Channel[]> {
    const response = await fetch(`${API_BASE}/channels`, {
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to fetch channels');
    }
    return response.json();
  }

  static async getMessages(channelId: string, limit = 50, before?: string): Promise<Message[]> {
    const params = new URLSearchParams({ limit: String(limit) });
    if (before) {
      params.set('before', before);
    }
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages?${params}`, {
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to fetch messages');
    }
    return response.json();
  }

  static async sendMessage(channelId: string, message: SendMessageRequest): Promise<Message> {
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(message),
    });

    if (!response.ok) {
      throw new Error('Failed to send message');
    }
    return response.json();
  }

  static async createChannel(name: string, channelType: 'text' | 'voice'): Promise<Channel> {
    const response = await fetch(`${API_BASE}/channels`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify({ name, channel_type: channelType }),
    });
    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || 'Failed to create channel');
    }
    return response.json();
  }

  static async updateChannel(channelId: string, name: string): Promise<Channel> {
    const response = await fetch(`${API_BASE}/channels/${channelId}`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify({ name }),
    });
    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || 'Failed to update channel');
    }
    return response.json();
  }

  static async deleteChannel(channelId: string): Promise<void> {
    const response = await fetch(`${API_BASE}/channels/${channelId}`, {
      method: 'DELETE',
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || 'Failed to delete channel');
    }
  }

  static async editMessage(channelId: string, messageId: string, content: string): Promise<Message> {
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages/${messageId}`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify({ content }),
    });
    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || 'Failed to edit message');
    }
    return response.json();
  }

  static async deleteMessage(channelId: string, messageId: string): Promise<void> {
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages/${messageId}`, {
      method: 'DELETE',
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || 'Failed to delete message');
    }
  }

  static async addReaction(channelId: string, messageId: string, emoji: string): Promise<void> {
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`, {
      method: 'PUT',
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to add reaction');
    }
  }

  static async removeReaction(channelId: string, messageId: string, emoji: string): Promise<void> {
    const response = await fetch(`${API_BASE}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`, {
      method: 'DELETE',
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to remove reaction');
    }
  }

  static async getOnlineUsers(): Promise<UserPresence[]> {
    const response = await fetch(`${API_BASE}/users/online`, {
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to fetch online users');
    }
    return response.json();
  }

  // Voice API methods
  static async joinVoiceChannel(request: JoinVoiceRequest): Promise<VoiceState> {
    const response = await fetch(`${API_BASE}/voice/join`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error('Failed to join voice channel');
    }
    return response.json();
  }

  static async leaveVoiceChannel(request: JoinVoiceRequest): Promise<void> {
    const response = await fetch(`${API_BASE}/voice/leave`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error('Failed to leave voice channel');
    }
  }

  static async getVoiceStates(channelId: string): Promise<VoiceState[]> {
    const response = await fetch(`${API_BASE}/voice/channels/${channelId}/states`, {
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to fetch voice states');
    }
    return response.json();
  }

  static async getAllVoiceStates(): Promise<VoiceState[]> {
    const response = await fetch(`${API_BASE}/voice/states`, {
      headers: AuthService.getAuthHeaders(),
    });
    if (!response.ok) {
      throw new Error('Failed to fetch all voice states');
    }
    return response.json();
  }

  static async updateVoiceState(channelId: string, request: UpdateVoiceStateRequest): Promise<VoiceState> {
    const response = await fetch(`${API_BASE}/voice/channels/${channelId}/state`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error('Failed to update voice state');
    }
    return response.json();
  }

  static async updateSpeakingStatus(channelId: string, request: UpdateSpeakingRequest): Promise<void> {
    const response = await fetch(`${API_BASE}/voice/channels/${channelId}/speaking`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error('Failed to update speaking status');
    }
  }

  static async updateScreenShareState(channelId: string, isScreenSharing: boolean): Promise<VoiceState> {
    const response = await fetch(`${API_BASE}/voice/channels/${channelId}/screen-share`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify({ is_screen_sharing: isScreenSharing }),
    });

    if (!response.ok) {
      throw new Error('Failed to update screen share state');
    }
    return response.json();
  }
}

export class WebSocketManager {
  private ws: WebSocket | null = null;
  private messageHandlers: ((data: any) => void)[] = [];
  private intentionalClose = false;
  private reconnectAttempts = 0;
  private maxReconnectDelay = 30000;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private currentChannelId: string | null = null;

  connect(): Promise<void> {
    this.intentionalClose = false;
    const token = AuthService.getToken();
    const wsBase = import.meta.env.VITE_WS_BASE || `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}`;
    this.ws = new WebSocket(`${wsBase}/ws?token=${token}`);

    return new Promise<void>((resolve) => {
      this.ws!.onopen = () => {
        console.log('WebSocket connected');
        this.reconnectAttempts = 0;
        if (this.currentChannelId) {
          this.joinChannel(this.currentChannelId);
        }
        resolve();
      };

      this.ws!.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          this.messageHandlers.forEach(handler => handler(data));
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.ws!.onclose = () => {
        console.log('WebSocket disconnected');
        if (!this.intentionalClose) {
          const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), this.maxReconnectDelay);
          this.reconnectAttempts++;
          console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
          this.reconnectTimer = setTimeout(() => this.connect(), delay);
        }
      };

      this.ws!.onerror = (error) => {
        console.error('WebSocket error:', error);
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

  onMessage(handler: (data: any) => void) {
    this.messageHandlers.push(handler);
  }

  joinChannel(channelId: string) {
    this.currentChannelId = channelId;
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const message = {
        message_type: 'join',
        channel_id: channelId,
        content: ''
      };
      this.ws.send(JSON.stringify(message));
    }
  }

  sendTyping(channelId: string) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const message = {
        message_type: 'typing',
        channel_id: channelId,
        content: ''
      };
      this.ws.send(JSON.stringify(message));
    }
  }

  sendMessage(channelId: string, content: string, replyToId?: string) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const message: Record<string, unknown> = {
        message_type: 'message',
        channel_id: channelId,
        content
      };
      if (replyToId) {
        message.reply_to_id = replyToId;
      }
      this.ws.send(JSON.stringify(message));
    }
  }
}

