import { get } from 'svelte/store';
import { goto } from '$app/navigation';
import { API } from '../api';
import { playSound } from '../sounds';
import AuthService, { user } from '../auth';
import { voiceManager } from '../voice';
import { isTauri, activeServer } from '../serverManager';
import { getWs, resetWs } from '../ws';
import { voiceStore } from '../stores/voiceStore';
import { serverState } from '../stores/serverState';
import { chatState } from '../stores/chatState';
import { uiState } from '../stores/uiState';
import { selectChannel, populateAvatarsFromMessages } from './chat';
import { initPTT, switchInputMode as pttSwitchInputMode, changePTTKey as pttChangePTTKey } from '../ptt';
import type { VoiceInputMode } from '../voice';

const TYPING_DISPLAY_MS = 5000;

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

function updateSpeaking(userId: string, speaking: boolean) {
  voiceStore.update((s) => {
    const next = new Set(s.speakingUsers);
    if (speaking) next.add(userId);
    else next.delete(userId);
    return { ...s, speakingUsers: next };
  });
}

function updateMessageReaction(
  msgId: string,
  emoji: string,
  userId: string,
  add: boolean,
) {
  const currentUser = get(user);
  chatState.update((s) => ({
    ...s,
    messages: s.messages.map((m) => {
      if (m.id !== msgId) return m;
      let reactions = m.reactions ? [...m.reactions] : [];
      const existing = reactions.find((r) => r.emoji === emoji);
      if (add) {
        if (existing) {
          existing.count += 1;
          if (userId === currentUser?.id) existing.reacted = true;
        } else {
          reactions.push({ emoji, count: 1, reacted: userId === currentUser?.id });
        }
      } else if (existing) {
        existing.count -= 1;
        if (userId === currentUser?.id) existing.reacted = false;
        if (existing.count <= 0) {
          reactions = reactions.filter((r) => r.emoji !== emoji);
        }
      }
      return { ...m, reactions: reactions.length > 0 ? reactions : undefined };
    }),
  }));
}

// scrollToBottom is called from the WS handler for new messages.
// We accept it as a callback since it's a DOM operation owned by MessageList.
let _scrollToBottom: (() => void) | null = null;
let _isNearBottom: (() => boolean) | null = null;

export function registerScrollCallbacks(
  scrollToBottom: () => void,
  isNearBottom: () => boolean,
) {
  _scrollToBottom = scrollToBottom;
  _isNearBottom = isNearBottom;
}

let _rateLimitTimeout: ReturnType<typeof setTimeout> | null = null;

export function setupWsHandlers() {
  getWs().onMessage((data) => {
    const cs = get(chatState);
    const vs = get(voiceStore);
    const ss = get(serverState);
    const currentUser = get(user);

    if (data.type === 'message' && data.data.channel_id === cs.selectedChannelId) {
      const shouldScroll = _isNearBottom?.() ?? false;
      populateAvatarsFromMessages([data.data]);
      chatState.update((s) => {
        const typingUsers = new Map(s.typingUsers);
        const authorId = data.data.author_id;
        if (authorId && typingUsers.has(authorId)) {
          clearTimeout(typingUsers.get(authorId)!.timeout);
          typingUsers.delete(authorId);
        }
        return { ...s, messages: [...s.messages, data.data], typingUsers };
      });
      if (shouldScroll) {
        requestAnimationFrame(() => _scrollToBottom?.());
      }
    }

    if (data.type === 'channel_created') {
      serverState.update((s) => ({ ...s, channels: [...s.channels, data.data] }));
    }
    if (data.type === 'channel_updated') {
      serverState.update((s) => ({
        ...s,
        channels: s.channels.map((c) => (c.id === data.data.id ? data.data : c)),
      }));
      if (cs.selectedChannelId === data.data.id) {
        chatState.update((s) => ({ ...s, selectedChannelName: data.data.name }));
      }
    }
    if (data.type === 'channel_deleted') {
      const updatedChannels = ss.channels.filter((c) => c.id !== data.data.id);
      serverState.update((s) => ({ ...s, channels: updatedChannels }));
      if (cs.selectedChannelId === data.data.id) {
        const firstText = updatedChannels.find((c) => c.channel_type === 'text');
        if (firstText) {
          selectChannel(firstText.id, firstText.name);
        } else {
          chatState.update((s) => ({
            ...s,
            selectedChannelId: '',
            selectedChannelName: '',
            messages: [],
          }));
        }
      }
    }

    if (data.type === 'user_online') {
      serverState.update((s) => {
        const exists = s.onlineUsers.find((u) => u.user_id === data.data.user_id);
        const onlineUsers = exists ? s.onlineUsers : [...s.onlineUsers, data.data];
        const userAvatars = data.data.avatar_url
          ? { ...s.userAvatars, [data.data.user_id]: API.getAvatarUrl(data.data.user_id) }
          : s.userAvatars;
        return { ...s, onlineUsers, userAvatars };
      });
    }
    if (data.type === 'user_offline') {
      serverState.update((s) => ({
        ...s,
        onlineUsers: s.onlineUsers.filter((u) => u.user_id !== data.data.user_id),
      }));
    }

    if (data.type === 'user_avatar_updated') {
      const { user_id, avatar_url } = data.data;
      serverState.update((s) => {
        if (avatar_url) {
          return {
            ...s,
            userAvatars: {
              ...s.userAvatars,
              [user_id]: API.getAvatarUrl(user_id) + '?t=' + Date.now(),
            },
          };
        } else {
          const { [user_id]: _, ...rest } = s.userAvatars;
          return { ...s, userAvatars: rest };
        }
      });
    }
    if (data.type === 'user_profile_updated') {
      const { user_id, username, avatar_url } = data.data;
      serverState.update((s) => ({
        ...s,
        onlineUsers: s.onlineUsers.map((u) =>
          u.user_id === user_id ? { ...u, username } : u,
        ),
        userAvatars: avatar_url
          ? { ...s.userAvatars, [user_id]: API.getAvatarUrl(user_id) + '?t=' + Date.now() }
          : s.userAvatars,
      }));
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === user_id ? { ...v, username } : v,
        ),
      }));
    }

    if (data.type === 'message_edited' && data.data.channel_id === cs.selectedChannelId) {
      chatState.update((s) => ({
        ...s,
        messages: s.messages.map((m) => (m.id === data.data.id ? data.data : m)),
      }));
    }
    if (data.type === 'message_deleted' && data.data.channel_id === cs.selectedChannelId) {
      chatState.update((s) => ({
        ...s,
        messages: s.messages.filter((m) => m.id !== data.data.id),
      }));
    }

    if (data.type === 'reaction_added' && cs.selectedChannelId) {
      updateMessageReaction(data.data.message_id, data.data.emoji, data.data.user_id, true);
    }
    if (data.type === 'reaction_removed' && cs.selectedChannelId) {
      updateMessageReaction(data.data.message_id, data.data.emoji, data.data.user_id, false);
    }

    if (data.type === 'link_preview_ready' && data.data.channel_id === cs.selectedChannelId) {
      const { message_id, link_previews } = data.data;
      chatState.update((s) => ({
        ...s,
        messages: s.messages.map((m) =>
          m.id === message_id ? { ...m, link_previews } : m,
        ),
      }));
    }

    if (data.type === 'voice_user_joined') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: [
          ...s.voiceStates.filter((v) => v.user_id !== data.data.user_id),
          data.data,
        ],
      }));
      if (
        data.data.channel_id === vs.currentVoiceChannel ||
        data.data.user_id === currentUser?.id
      ) {
        playSound('connect');
      }
    }

    if (
      data.type === 'new_producer' &&
      data.data.channel_id === vs.currentVoiceChannel &&
      data.data.user_id !== currentUser?.id
    ) {
      if (data.data.label !== 'screen' && data.data.label !== 'camera') {
        voiceManager.consumeProducer(data.data.producer_id, data.data.user_id, data.data.label);
      } else if (data.data.label === 'screen' && vs.watchingScreenUserId === data.data.user_id) {
        voiceManager.consumeProducer(data.data.producer_id, data.data.user_id, data.data.label);
      } else if (data.data.label === 'camera' && vs.watchingCameraUserId === data.data.user_id) {
        voiceManager.consumeProducer(data.data.producer_id, data.data.user_id, data.data.label);
      }
    }

    if (data.type === 'voice_user_left') {
      if (data.data.channel_id === vs.currentVoiceChannel) {
        playSound('disconnect');
      }
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.filter(
          (v) => !(v.user_id === data.data.user_id && v.channel_id === data.data.channel_id),
        ),
      }));
      voiceManager.removeUserAudio(data.data.user_id);
    }

    if (data.type === 'voice_state_updated') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === data.data.user_id && v.channel_id === data.data.channel_id
            ? data.data
            : v,
        ),
      }));
    }

    if (data.type === 'voice_speaking') {
      updateSpeaking(data.data.user_id, data.data.is_speaking);
    }

    if (data.type === 'screen_share_updated') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === data.data.user_id && v.channel_id === data.data.channel_id
            ? { ...v, is_screen_sharing: data.data.is_screen_sharing }
            : v,
        ),
      }));
      if (!data.data.is_screen_sharing && vs.watchingScreenUserId === data.data.user_id) {
        uiState.update((s) => ({ ...s, stopWatchingScreen: true }));
      }
    }

    if (data.type === 'camera_updated') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === data.data.user_id && v.channel_id === data.data.channel_id
            ? { ...v, is_camera_sharing: data.data.is_camera_sharing }
            : v,
        ),
      }));
      if (!data.data.is_camera_sharing && vs.watchingCameraUserId === data.data.user_id) {
        uiState.update((s) => ({ ...s, stopWatchingCamera: true }));
      }
    }

    if (data.type === 'user_kicked' && data.data.user_id === currentUser?.id) {
      alert('You have been kicked from the server.');
      AuthService.logout();
      goto('/auth');
      return;
    }
    if (data.type === 'user_banned' && data.data.user_id === currentUser?.id) {
      alert('You have been banned from the server.');
      AuthService.logout();
      goto('/auth');
      return;
    }

    if (data.type === 'user_role_changed') {
      if (data.data.user_id === currentUser?.id && currentUser) {
        user.set({ ...currentUser, role: data.data.new_role });
      }
      serverState.update((s) => ({
        ...s,
        userRolesMap: { ...s.userRolesMap, [data.data.user_id]: data.data.new_role },
      }));
    }

    if (data.type === 'user_renamed') {
      const { user_id, new_username } = data.data;
      serverState.update((s) => ({
        ...s,
        onlineUsers: s.onlineUsers.map((u) =>
          u.user_id === user_id ? { ...u, username: new_username } : u,
        ),
      }));
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === user_id ? { ...v, username: new_username } : v,
        ),
      }));
    }

    if (data.type === 'error' && data.data.code === 'rate_limited') {
      chatState.update((s) => ({ ...s, rateLimitWarning: true }));
      if (_rateLimitTimeout) clearTimeout(_rateLimitTimeout);
      _rateLimitTimeout = setTimeout(() => {
        chatState.update((s) => ({ ...s, rateLimitWarning: false }));
        _rateLimitTimeout = null;
      }, 3000);
    }

    if (data.type === 'typing' && data.data.channel_id === cs.selectedChannelId) {
      const userId = data.data.user_id;
      if (userId === currentUser?.id) return;
      chatState.update((s) => {
        const typingUsers = new Map(s.typingUsers);
        const existing = typingUsers.get(userId);
        if (existing) clearTimeout(existing.timeout);
        const timeout = setTimeout(() => {
          chatState.update((inner) => {
            const m = new Map(inner.typingUsers);
            m.delete(userId);
            return { ...inner, typingUsers: m };
          });
        }, TYPING_DISPLAY_MS);
        typingUsers.set(userId, { username: data.data.username, timeout });
        return { ...s, typingUsers };
      });
    }
  });
}

export function setupVoiceHandlers(
  screenVideoElement: () => HTMLVideoElement | null,
  cameraVideoElement: () => HTMLVideoElement | null,
  screenAudioRef: { el: HTMLAudioElement | null },
) {
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

  voiceManager.onSpeakingChange(updateSpeaking);
  voiceManager.onStateChange(syncVoiceState);

  voiceManager.onScreenTrack((track) => {
    if (track.kind === 'video') {
      const el = screenVideoElement();
      if (el) {
        el.srcObject = new MediaStream([track]);
        el.play().catch((e) => console.warn('Screen video autoplay prevented:', e));
      }
    } else if (track.kind === 'audio') {
      if (screenAudioRef.el) {
        screenAudioRef.el.srcObject = null;
        screenAudioRef.el.remove();
      }
      screenAudioRef.el = document.createElement('audio');
      screenAudioRef.el.autoplay = true;
      screenAudioRef.el.volume = 1.0;
      screenAudioRef.el.srcObject = new MediaStream([track]);
      document.body.appendChild(screenAudioRef.el);
      screenAudioRef.el
        .play()
        .catch((e) => console.warn('Screen audio autoplay prevented:', e));
    }
  });

  voiceManager.onCameraTrack((track) => {
    if (track.kind === 'video') {
      const el = cameraVideoElement();
      if (el) {
        el.srcObject = new MediaStream([track]);
        el.play().catch((e) => console.warn('Camera video autoplay prevented:', e));
      }
    }
  });
}

export async function connectToServer() {
  chatState.update((s) => {
    s.typingUsers.forEach((u) => clearTimeout(u.timeout));
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
      typingUsers: new Map(),
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
    tauriVersion: get(serverState).tauriVersion,
    customEmojis: [],
  });
  voiceStore.update((s) => ({ ...s, voiceStates: [], speakingUsers: new Set() }));

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
