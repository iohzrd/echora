import type { Message } from '../api';

export interface TypingUser {
  username: string;
  timeout: ReturnType<typeof setTimeout>;
}

export interface ChatStateStore {
  messages: Message[];
  selectedChannelId: string;
  selectedChannelName: string;
  hasMoreMessages: boolean;
  loadingMore: boolean;
  editingMessageId: string | null;
  editMessageContent: string;
  replyingTo: Message | null;
  typingUsers: Record<string, TypingUser>;
  rateLimitWarning: boolean;
  sendError: boolean;
}

export const chatState = $state<ChatStateStore>({
  messages: [],
  selectedChannelId: '',
  selectedChannelName: '',
  hasMoreMessages: true,
  loadingMore: false,
  editingMessageId: null,
  editMessageContent: '',
  replyingTo: null,
  typingUsers: {},
  rateLimitWarning: false,
  sendError: false,
});
