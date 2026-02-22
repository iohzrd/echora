import { goto } from "$app/navigation";
import { browser } from "$app/environment";
import { API } from "../api";
import { getWs } from "../ws";
import { chatState } from "../stores/chatState.svelte";
import { serverState } from "../stores/serverState.svelte";
import { authState } from "../stores/authState.svelte";
import type { Message } from "../api";

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
  for (const m of msgs) {
    if (!(m.author_id in serverState.userAvatars)) {
      serverState.userAvatars[m.author_id] = API.getAvatarUrl(m.author_id);
    }
  }
}

export async function selectChannel(channelId: string, channelName: string) {
  Object.values(chatState.typingUsers).forEach((u) => clearTimeout(u.timeout));
  chatState.selectedChannelId = channelId;
  chatState.selectedChannelName = channelName;
  chatState.messages = [];
  chatState.hasMoreMessages = true;
  chatState.replyingTo = null;
  chatState.typingUsers = {};

  getWs().joinChannel(channelId);

  if (browser)
    goto(`/channels/${channelId}`, {
      replaceState: false,
      noScroll: true,
      keepFocus: true,
    });

  try {
    const msgs = await API.getMessages(channelId, 50);
    populateAvatarsFromMessages(msgs);
    // Guard: channel may have changed during the async fetch
    if (chatState.selectedChannelId !== channelId) return true;
    // Merge REST history with any WS events already received during the fetch.
    const restIds = new Set(msgs.map((m) => m.id));
    const wsOnly = chatState.messages.filter((m) => !restIds.has(m.id));
    const merged = [...msgs, ...wsOnly].sort(
      (a, b) =>
        new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
    );
    chatState.messages = merged;
    chatState.hasMoreMessages = msgs.length >= 50;
    return true;
  } catch (error) {
    console.error("Failed to load messages:", error);
    chatState.messages = [];
    return false;
  }
}

export async function loadOlderMessages(scrollToPreserve: () => () => void) {
  if (
    chatState.loadingMore ||
    !chatState.hasMoreMessages ||
    !chatState.selectedChannelId ||
    chatState.messages.length === 0
  )
    return;

  chatState.loadingMore = true;
  const channelId = chatState.selectedChannelId;
  const oldestTimestamp = chatState.messages[0]?.timestamp;

  try {
    const olderMessages = await API.getMessages(channelId, 50, oldestTimestamp);
    populateAvatarsFromMessages(olderMessages);
    if (olderMessages.length > 0) {
      const restoreScroll = scrollToPreserve();
      // Guard: channel may have changed during the async fetch
      if (chatState.selectedChannelId !== channelId) return;
      const existingIds = new Set(chatState.messages.map((m) => m.id));
      const uniqueOlder = olderMessages.filter((m) => !existingIds.has(m.id));
      chatState.messages = [...uniqueOlder, ...chatState.messages];
      chatState.hasMoreMessages = olderMessages.length >= 50;
      restoreScroll();
    } else {
      if (chatState.selectedChannelId === channelId) {
        chatState.hasMoreMessages = false;
      }
    }
  } catch (error) {
    console.error("Failed to load older messages:", error);
  } finally {
    chatState.loadingMore = false;
  }
}

export function sendMessage(text: string, attachmentIds?: string[]) {
  const { selectedChannelId, replyingTo } = chatState;
  const currentUser = authState.user;
  if (selectedChannelId && currentUser) {
    const sent = getWs().sendMessage(
      selectedChannelId,
      text,
      replyingTo?.id,
      attachmentIds,
    );
    if (sent) {
      chatState.replyingTo = null;
      chatState.sendError = false;
    } else {
      chatState.sendError = true;
      if (_sendErrorTimeout) clearTimeout(_sendErrorTimeout);
      _sendErrorTimeout = setTimeout(() => {
        chatState.sendError = false;
        _sendErrorTimeout = null;
      }, 4000);
    }
  }
}

export function sendTyping() {
  const now = Date.now();
  const { selectedChannelId } = chatState;
  if (selectedChannelId && now - lastTypingSent > TYPING_DEBOUNCE_MS) {
    lastTypingSent = now;
    getWs().sendTyping(selectedChannelId);
  }
}

export function startEditMessage(message: Message) {
  chatState.editingMessageId = message.id;
  chatState.editMessageContent = message.content;
}

export function cancelEditMessage() {
  chatState.editingMessageId = null;
  chatState.editMessageContent = "";
}

export async function saveEditMessage() {
  const { editingMessageId, editMessageContent, selectedChannelId } = chatState;
  if (!editingMessageId || !editMessageContent.trim() || !selectedChannelId)
    return;
  try {
    await API.editMessage(
      selectedChannelId,
      editingMessageId,
      editMessageContent.trim(),
    );
    chatState.editingMessageId = null;
    chatState.editMessageContent = "";
  } catch (error) {
    console.error("Failed to edit message:", error);
  }
}

export async function deleteMessage(messageId: string) {
  if (!confirm("Delete this message?")) return;
  const { selectedChannelId } = chatState;
  if (!selectedChannelId) return;
  try {
    await API.deleteMessage(selectedChannelId, messageId);
  } catch (error) {
    console.error("Failed to delete message:", error);
  }
}

export function startReply(message: Message) {
  chatState.replyingTo = message;
}

export function cancelReply() {
  chatState.replyingTo = null;
}

export async function toggleReaction(messageId: string, emoji: string) {
  const { selectedChannelId, messages } = chatState;
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
    console.error("Failed to toggle reaction:", error);
  }
}

export async function createChannel(name: string, type: "text" | "voice") {
  try {
    await API.createChannel(name, type);
  } catch (error) {
    console.error("Failed to create channel:", error);
  }
}

export async function updateChannel(channelId: string, name: string) {
  try {
    await API.updateChannel(channelId, name);
  } catch (error) {
    console.error("Failed to update channel:", error);
  }
}

export async function deleteChannel(channelId: string) {
  if (!confirm("Delete this channel and all its messages?")) return;
  try {
    await API.deleteChannel(channelId);
  } catch (error) {
    console.error("Failed to delete channel:", error);
  }
}

export function updateEditMessageContent(content: string) {
  chatState.editMessageContent = content;
}
