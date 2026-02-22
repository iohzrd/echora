import { get } from 'svelte/store';
import { API } from '../api';
import { user } from '../auth';
import { getWs } from '../ws';
import { chatState } from '../stores/chatState';
import { serverState } from '../stores/serverState';
import type { Message } from '../api';

const TYPING_DEBOUNCE_MS = 3000;
let lastTypingSent = 0;
let _sendErrorTimeout: ReturnType<typeof setTimeout> | null = null;

export function resetChatActionState() {
  lastTypingSent = 0;
  if (_sendErrorTimeout) {
    clearTimeout(_sendErrorTimeout);
    _sendErrorTimeout = null;
  }
}

export function populateAvatarsFromMessages(msgs: Message[]) {
  const current = get(serverState);
  const avatars = { ...current.userAvatars };
  let changed = false;
  for (const m of msgs) {
    if (!(m.author_id in avatars)) {
      avatars[m.author_id] = API.getAvatarUrl(m.author_id);
      changed = true;
    }
  }
  if (changed) serverState.update((s) => ({ ...s, userAvatars: avatars }));
}

export async function selectChannel(channelId: string, channelName: string) {
  chatState.update((s) => {
    Object.values(s.typingUsers).forEach((u) => clearTimeout(u.timeout));
    return {
      ...s,
      selectedChannelId: channelId,
      selectedChannelName: channelName,
      hasMoreMessages: true,
      replyingTo: null,
      typingUsers: {},
    };
  });
  getWs().joinChannel(channelId);

  try {
    const msgs = await API.getMessages(channelId, 50);
    populateAvatarsFromMessages(msgs);
    // Merge REST history with any WS events already received during the fetch.
    // Deduplicate by ID so messages that arrived via both paths appear once.
    // REST provides the authoritative ordered list; WS-only additions are appended.
    chatState.update((s) => {
      if (s.selectedChannelId !== channelId) return s;
      const restIds = new Set(msgs.map((m) => m.id));
      const wsOnly = s.messages.filter((m) => !restIds.has(m.id));
      const merged = [...msgs, ...wsOnly].sort(
        (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
      );
      return {
        ...s,
        messages: merged,
        hasMoreMessages: msgs.length >= 50,
      };
    });
    return true;
  } catch (error) {
    console.error('Failed to load messages:', error);
    chatState.update((s) => ({ ...s, messages: [] }));
    return false;
  }
}

export async function loadOlderMessages(
  scrollToPreserve: () => () => void,
) {
  const cs = get(chatState);
  if (
    cs.loadingMore ||
    !cs.hasMoreMessages ||
    !cs.selectedChannelId ||
    cs.messages.length === 0
  )
    return;

  chatState.update((s) => ({ ...s, loadingMore: true }));
  const oldestTimestamp = cs.messages[0]?.timestamp;

  try {
    const olderMessages = await API.getMessages(
      cs.selectedChannelId,
      50,
      oldestTimestamp,
    );
    populateAvatarsFromMessages(olderMessages);
    if (olderMessages.length > 0) {
      // Capture scroll position before DOM update, then restore after
      const restoreScroll = scrollToPreserve();
      chatState.update((s) => {
        // Guard: channel may have changed during the async fetch
        if (s.selectedChannelId !== cs.selectedChannelId) return s;
        return {
          ...s,
          hasMoreMessages: olderMessages.length >= 50,
          messages: [...olderMessages, ...s.messages],
        };
      });
      restoreScroll();
    } else {
      chatState.update((s) => {
        if (s.selectedChannelId !== cs.selectedChannelId) return s;
        return { ...s, hasMoreMessages: false };
      });
    }
  } catch (error) {
    console.error('Failed to load older messages:', error);
  } finally {
    chatState.update((s) => ({ ...s, loadingMore: false }));
  }
}

export function sendMessage(text: string, attachmentIds?: string[]) {
  const { selectedChannelId, replyingTo } = get(chatState);
  const currentUser = get(user);
  if (selectedChannelId && currentUser) {
    const sent = getWs().sendMessage(selectedChannelId, text, replyingTo?.id, attachmentIds);
    if (sent) {
      chatState.update((s) => ({ ...s, replyingTo: null, sendError: false }));
    } else {
      chatState.update((s) => ({ ...s, sendError: true }));
      if (_sendErrorTimeout) clearTimeout(_sendErrorTimeout);
      _sendErrorTimeout = setTimeout(() => {
        chatState.update((s) => ({ ...s, sendError: false }));
        _sendErrorTimeout = null;
      }, 4000);
    }
  }
}

export function sendTyping() {
  const now = Date.now();
  const { selectedChannelId } = get(chatState);
  if (selectedChannelId && now - lastTypingSent > TYPING_DEBOUNCE_MS) {
    lastTypingSent = now;
    getWs().sendTyping(selectedChannelId);
  }
}

export function startEditMessage(message: Message) {
  chatState.update((s) => ({
    ...s,
    editingMessageId: message.id,
    editMessageContent: message.content,
  }));
}

export function cancelEditMessage() {
  chatState.update((s) => ({
    ...s,
    editingMessageId: null,
    editMessageContent: '',
  }));
}

export async function saveEditMessage() {
  const { editingMessageId, editMessageContent, selectedChannelId } = get(chatState);
  if (!editingMessageId || !editMessageContent.trim()) return;
  try {
    await API.editMessage(selectedChannelId, editingMessageId, editMessageContent.trim());
    chatState.update((s) => ({
      ...s,
      editingMessageId: null,
      editMessageContent: '',
    }));
  } catch (error) {
    console.error('Failed to edit message:', error);
  }
}

export async function deleteMessage(messageId: string) {
  if (!confirm('Delete this message?')) return;
  const { selectedChannelId } = get(chatState);
  try {
    await API.deleteMessage(selectedChannelId, messageId);
  } catch (error) {
    console.error('Failed to delete message:', error);
  }
}

export function startReply(message: Message) {
  chatState.update((s) => ({ ...s, replyingTo: message }));
}

export function cancelReply() {
  chatState.update((s) => ({ ...s, replyingTo: null }));
}

export async function toggleReaction(messageId: string, emoji: string) {
  const { selectedChannelId, messages } = get(chatState);
  if (!selectedChannelId) return;
  try {
    const msg = messages.find((m) => m.id === messageId);
    const existingReaction = msg?.reactions?.find((r) => r.emoji === emoji);
    if (existingReaction?.reacted) {
      await API.removeReaction(selectedChannelId, messageId, emoji);
    } else {
      await API.addReaction(selectedChannelId, messageId, emoji);
    }
  } catch (error) {
    console.error('Failed to toggle reaction:', error);
  }
}

export async function createChannel(name: string, type: 'text' | 'voice') {
  try {
    await API.createChannel(name, type);
  } catch (error) {
    console.error('Failed to create channel:', error);
  }
}

export async function updateChannel(channelId: string, name: string) {
  try {
    await API.updateChannel(channelId, name);
  } catch (error) {
    console.error('Failed to update channel:', error);
  }
}

export async function deleteChannel(channelId: string) {
  if (!confirm('Delete this channel and all its messages?')) return;
  try {
    await API.deleteChannel(channelId);
  } catch (error) {
    console.error('Failed to delete channel:', error);
  }
}

export function updateEditMessageContent(content: string) {
  chatState.update((s) => ({ ...s, editMessageContent: content }));
}
