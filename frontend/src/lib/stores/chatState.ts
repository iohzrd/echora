import { writable } from 'svelte/store';
import type { Message } from '../api';

export interface ChatStateStore {
  messages: Message[];
  selectedChannelId: string;
  selectedChannelName: string;
  hasMoreMessages: boolean;
  loadingMore: boolean;
  editingMessageId: string | null;
  editMessageContent: string;
  replyingTo: Message | null;
  typingUsers: Map<string, { username: string; timeout: ReturnType<typeof setTimeout> }>;
  rateLimitWarning: boolean;
}

export const chatState = writable<ChatStateStore>({
  messages: [],
  selectedChannelId: '',
  selectedChannelName: '',
  hasMoreMessages: true,
  loadingMore: false,
  editingMessageId: null,
  editMessageContent: '',
  replyingTo: null,
  typingUsers: new Map(),
  rateLimitWarning: false,
});
