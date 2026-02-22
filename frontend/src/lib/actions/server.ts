import { get } from 'svelte/store';
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { API, ApiError } from '../api';
import AuthService, { user } from '../auth';
import { voiceManager } from '../voice';
import { isTauri, activeServer } from '../serverManager';
import { getWs, resetWs } from '../ws';
import { voiceStore } from '../stores/voiceStore.svelte';
import { serverState } from '../stores/serverState.svelte';
import { chatState } from '../stores/chatState.svelte';
import { uiState } from '../stores/uiState.svelte';
import { selectChannel, resetChatActionState } from './chat';
import { setupWsHandlers, teardownWsHandlers } from './wsHandlers';
import { initPTT, switchInputMode as pttSwitchInputMode, changePTTKey as pttChangePTTKey } from '../ptt';
import type { VoiceInputMode } from '../voice';

export function syncVoiceState() {
  voiceStore.currentVoiceChannel = voiceManager.currentChannel;
  voiceStore.isMuted = voiceManager.isMutedState;
  voiceStore.isDeafened = voiceManager.isDeafenedState;
  voiceStore.isScreenSharing = voiceManager.isScreenSharingState;
  voiceStore.isCameraSharing = voiceManager.isCameraSharingState;
  voiceStore.voiceInputMode = voiceManager.currentInputMode;
  voiceStore.pttActive = voiceManager.isPTTActive;
}

function setupVoiceStateHandlers() {
  voiceManager.onVoiceStatesChange((states) => {
    if (states.length > 0) {
      const channelId = states[0].channel_id;
      voiceStore.voiceStates = [
        ...voiceStore.voiceStates.filter((v) => v.channel_id !== channelId),
        ...states,
      ];
    }
  });

  voiceManager.onSpeakingChange((userId, speaking) => {
    if (speaking) {
      if (!voiceStore.speakingUsers.includes(userId)) {
        voiceStore.speakingUsers = [...voiceStore.speakingUsers, userId];
      }
    } else {
      voiceStore.speakingUsers = voiceStore.speakingUsers.filter((id) => id !== userId);
    }
  });

  voiceManager.onStateChange(syncVoiceState);
}

let _connecting = false;

export async function connectToServer() {
  if (_connecting) return;
  _connecting = true;
  try {
    await _connectToServer();
  } finally {
    _connecting = false;
  }
}

async function _connectToServer() {
  Object.values(chatState.typingUsers).forEach((u) => clearTimeout(u.timeout));
  chatState.messages = [];
  chatState.selectedChannelId = '';
  chatState.selectedChannelName = '';
  chatState.hasMoreMessages = true;
  chatState.loadingMore = false;
  chatState.editingMessageId = null;
  chatState.editMessageContent = '';
  chatState.replyingTo = null;
  chatState.typingUsers = {};
  chatState.rateLimitWarning = false;
  chatState.sendError = false;

  serverState.channels = [];
  serverState.onlineUsers = [];
  serverState.userAvatars = {};
  serverState.userRolesMap = {};
  serverState.serverName = '';
  serverState.backendVersion = '';
  serverState.customEmojis = [];

  voiceStore.voiceStates = [];
  voiceStore.speakingUsers = [];

  if (isTauri && !get(activeServer)) {
    uiState.showAddServerDialog = true;
    return;
  }

  await AuthService.init();

  if (!get(user)) {
    if (isTauri) {
      uiState.needsServerAuth = true;
      return;
    }
    goto('/auth');
    return;
  }

  try {
    const init = await API.getInit();

    const avatarMap: Record<string, string | undefined> = {};
    for (const u of init.online_users) {
      if (u.avatar_url) avatarMap[u.user_id] = API.getAvatarUrl(u.user_id);
    }
    for (const vs of init.voice_states) {
      if (vs.avatar_url) avatarMap[vs.user_id] = API.getAvatarUrl(vs.user_id);
    }
    const currentUser = get(user);
    if (currentUser?.avatar_url) avatarMap[currentUser.id] = API.getAvatarUrl(currentUser.id);

    serverState.channels = init.channels;
    serverState.onlineUsers = init.online_users;
    serverState.userAvatars = avatarMap;
    serverState.userRolesMap = init.users
      ? Object.fromEntries(init.users.map((u) => [u.id, u.role]))
      : {};
    serverState.serverName = init.server_name;
    serverState.backendVersion = init.version;

    voiceStore.voiceStates = init.voice_states;

    try {
      const emojis = await API.getCustomEmojis();
      serverState.customEmojis = emojis;
    } catch {
      // Custom emojis may not be available
    }

    teardownWsHandlers();
    resetChatActionState();
    resetWs();
    setupWsHandlers();
    setupVoiceStateHandlers();
    getWs().onReconnect(() => voiceManager.reconcileProducers());

    // Connect WS before fetching messages so the subscription is established
    // before the REST fetch. Any messages that arrive during the REST fetch
    // will be buffered via WS and deduplicated in selectChannel.
    await getWs().connect();
    syncVoiceState();

    const { channels } = serverState;
    const urlChannelId = page.params.channelId;
    const targetChannel = urlChannelId ? channels.find((c) => c.id === urlChannelId) : null;
    const channelToSelect = targetChannel ?? channels.find((c) => c.channel_type === 'text');
    if (channelToSelect) {
      await selectChannel(channelToSelect.id, channelToSelect.name);
    }
  } catch (error) {
    console.error('Failed to load initial data:', error);
    if (error instanceof ApiError && error.status === 401) {
      AuthService.logout();
      goto('/auth');
    }
  }
}

export async function switchVoiceInputMode(mode: VoiceInputMode) {
  const { pttKey } = voiceStore;
  await pttSwitchInputMode(mode, pttKey);
  voiceStore.voiceInputMode = mode;
  syncVoiceState();
}

export async function changePTTKey(key: string) {
  voiceStore.pttKey = key;
  await pttChangePTTKey(key);
}

export async function initPTTSettings() {
  const pttSettings = await initPTT();
  voiceStore.voiceInputMode = pttSettings.inputMode;
  voiceStore.pttKey = pttSettings.pttKey;
}

export async function uploadCustomEmoji(name: string, file: File): Promise<void> {
  const emoji = await API.uploadCustomEmoji(name, file);
  serverState.customEmojis = [...serverState.customEmojis, emoji];
}

export async function deleteCustomEmoji(id: string): Promise<void> {
  await API.deleteCustomEmoji(id);
  serverState.customEmojis = serverState.customEmojis.filter((e) => e.id !== id);
}
