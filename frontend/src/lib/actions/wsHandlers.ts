import { get } from 'svelte/store';
import { goto } from '$app/navigation';
import { API } from '../api';
import { playSound } from '../sounds';
import AuthService, { user } from '../auth';
import { voiceManager } from '../voice';
import { getWs } from '../ws';
import { voiceStore } from '../stores/voiceStore';
import { serverState } from '../stores/serverState';
import { chatState } from '../stores/chatState';
import { selectChannel, populateAvatarsFromMessages } from './chat';

const TYPING_DISPLAY_MS = 5000;

let _rateLimitTimeout: ReturnType<typeof setTimeout> | null = null;

function updateMessageReaction(msgId: string, emoji: string, userId: string, add: boolean) {
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

export function setupWsHandlers() {
  getWs().onMessage((data) => {
    const cs = get(chatState);
    const vs = get(voiceStore);
    const ss = get(serverState);
    const currentUser = get(user);

    if (data.type === 'message' && data.data.channel_id === cs.selectedChannelId) {
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
      voiceStore.update((s) => {
        const next = new Set(s.speakingUsers);
        if (data.data.is_speaking) next.add(data.data.user_id);
        else next.delete(data.data.user_id);
        return { ...s, speakingUsers: next };
      });
    }

    if (data.type === 'screen_share_updated') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === data.data.user_id && v.channel_id === data.data.channel_id
            ? { ...v, is_screen_sharing: data.data.is_screen_sharing }
            : v,
        ),
        watchingScreenUserId:
          !data.data.is_screen_sharing && s.watchingScreenUserId === data.data.user_id
            ? null
            : s.watchingScreenUserId,
        watchingScreenUsername:
          !data.data.is_screen_sharing && s.watchingScreenUserId === data.data.user_id
            ? ''
            : s.watchingScreenUsername,
      }));
    }

    if (data.type === 'camera_updated') {
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === data.data.user_id && v.channel_id === data.data.channel_id
            ? { ...v, is_camera_sharing: data.data.is_camera_sharing }
            : v,
        ),
        watchingCameraUserId:
          !data.data.is_camera_sharing && s.watchingCameraUserId === data.data.user_id
            ? null
            : s.watchingCameraUserId,
        watchingCameraUsername:
          !data.data.is_camera_sharing && s.watchingCameraUserId === data.data.user_id
            ? ''
            : s.watchingCameraUsername,
      }));
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
