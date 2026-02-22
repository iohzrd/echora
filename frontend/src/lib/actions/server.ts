import { get } from 'svelte/store';
import { goto } from '$app/navigation';
import { API } from '../api';
import AuthService, { user } from '../auth';
import { voiceManager } from '../voice';
import { isTauri, activeServer } from '../serverManager';
import { getWs, resetWs } from '../ws';
import { voiceStore } from '../stores/voiceStore';
import { serverState } from '../stores/serverState';
import { chatState } from '../stores/chatState';
import { uiState } from '../stores/uiState';
import { selectChannel } from './chat';
import { setupWsHandlers } from './wsHandlers';
import { initPTT, switchInputMode as pttSwitchInputMode, changePTTKey as pttChangePTTKey } from '../ptt';
import type { VoiceInputMode } from '../voice';

export function syncVoiceState() {
  voiceStore.update((s) => ({
    ...s,
    currentVoiceChannel: voiceManager.currentChannel,
    isMuted: voiceManager.isMutedState,
    isDeafened: voiceManager.isDeafenedState,
    isScreenSharing: voiceManager.isScreenSharingState,
    isCameraSharing: voiceManager.isCameraSharingState,
    voiceInputMode: voiceManager.currentInputMode,
    pttActive: voiceManager.isPTTActive,
  }));
}

function setupVoiceStateHandlers() {
  voiceManager.onVoiceStatesChange((states) => {
    if (states.length > 0) {
      const channelId = states[0].channel_id;
      voiceStore.update((s) => ({
        ...s,
        voiceStates: [
          ...s.voiceStates.filter((v) => v.channel_id !== channelId),
          ...states,
        ],
      }));
    }
  });

  voiceManager.onSpeakingChange((userId, speaking) => {
    voiceStore.update((s) => {
      const speakingUsers = speaking
        ? s.speakingUsers.includes(userId)
          ? s.speakingUsers
          : [...s.speakingUsers, userId]
        : s.speakingUsers.filter((id) => id !== userId);
      return { ...s, speakingUsers };
    });
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
  chatState.update((s) => {
    Object.values(s.typingUsers).forEach((u) => clearTimeout(u.timeout));
    return {
      ...s,
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
    };
  });
  serverState.set({
    channels: [],
    onlineUsers: [],
    userAvatars: {},
    userRolesMap: {},
    serverName: '',
    backendVersion: '',
    customEmojis: [],
  });
  voiceStore.update((s) => ({ ...s, voiceStates: [], speakingUsers: [] }));

  if (isTauri && !get(activeServer)) {
    uiState.update((s) => ({ ...s, showAddServerDialog: true }));
    return;
  }

  await AuthService.init();

  if (!get(user)) {
    if (isTauri) {
      uiState.update((s) => ({ ...s, needsServerAuth: true }));
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

    serverState.update((s) => ({
      ...s,
      channels: init.channels,
      onlineUsers: init.online_users,
      userAvatars: avatarMap,
      userRolesMap: init.users
        ? Object.fromEntries(init.users.map((u) => [u.id, u.role]))
        : {},
      serverName: init.server_name,
      backendVersion: init.version,
    }));

    voiceStore.update((s) => ({ ...s, voiceStates: init.voice_states }));

    try {
      const emojis = await API.getCustomEmojis();
      serverState.update((s) => ({ ...s, customEmojis: emojis }));
    } catch {
      // Custom emojis may not be available
    }

    resetWs();
    setupWsHandlers();
    setupVoiceStateHandlers();
    getWs().onReconnect(() => voiceManager.reconcileProducers());
    await getWs().connect();
    syncVoiceState();

    const { channels } = get(serverState);
    const firstTextChannel = channels.find((c) => c.channel_type === 'text');
    if (firstTextChannel) {
      await selectChannel(firstTextChannel.id, firstTextChannel.name);
    }
  } catch (error) {
    console.error('Failed to load initial data:', error);
    if (error instanceof Error && error.message.includes('401')) {
      AuthService.logout();
      goto('/auth');
    }
  }
}

export async function switchVoiceInputMode(mode: VoiceInputMode) {
  const { pttKey } = get(voiceStore);
  await pttSwitchInputMode(mode, pttKey);
  voiceStore.update((s) => ({ ...s, voiceInputMode: mode }));
  syncVoiceState();
}

export async function changePTTKey(key: string) {
  voiceStore.update((s) => ({ ...s, pttKey: key }));
  await pttChangePTTKey(key);
}

export async function initPTTSettings() {
  const pttSettings = await initPTT();
  voiceStore.update((s) => ({
    ...s,
    voiceInputMode: pttSettings.inputMode,
    pttKey: pttSettings.pttKey,
  }));
}
