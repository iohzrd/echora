import { get } from "svelte/store";
import { goto } from "$app/navigation";
import { API, type WsIncomingMessage } from "../api";
import { playSound } from "../sounds";
import AuthService, { user } from "../auth";
import { voiceManager } from "../voice";
import { getWs } from "../ws";
import { voiceStore } from "../stores/voiceStore";
import { serverState } from "../stores/serverState";
import { chatState } from "../stores/chatState";
import { selectChannel, populateAvatarsFromMessages } from "./chat";

const TYPING_DISPLAY_MS = 5000;

let _rateLimitTimeout: ReturnType<typeof setTimeout> | null = null;
let _activeHandler: ((data: WsIncomingMessage) => void) | null = null;

export function teardownWsHandlers() {
  if (_rateLimitTimeout) {
    clearTimeout(_rateLimitTimeout);
    _rateLimitTimeout = null;
  }
  if (_activeHandler) {
    getWs().offMessage(_activeHandler);
    _activeHandler = null;
  }
}

function updateMessageReaction(
  msgId: string,
  emoji: string,
  userId: string,
  username: string,
  add: boolean,
) {
  const currentUser = get(user);
  chatState.update((s) => ({
    ...s,
    messages: s.messages.map((m) => {
      if (m.id !== msgId) return m;
      const reactions = m.reactions ? [...m.reactions] : [];
      const existingIdx = reactions.findIndex((r) => r.emoji === emoji);
      let newReactions;
      if (add) {
        if (existingIdx >= 0) {
          newReactions = reactions.map((r, i) =>
            i === existingIdx
              ? {
                  ...r,
                  count: r.count + 1,
                  reacted: r.reacted || userId === currentUser?.id,
                  users: [...r.users, username],
                }
              : r,
          );
        } else {
          newReactions = [
            ...reactions,
            {
              emoji,
              count: 1,
              reacted: userId === currentUser?.id,
              users: [username],
            },
          ];
        }
      } else if (existingIdx >= 0) {
        const updated = {
          ...reactions[existingIdx],
          count: reactions[existingIdx].count - 1,
          users: reactions[existingIdx].users.filter((u) => u !== username),
        };
        if (userId === currentUser?.id) updated.reacted = false;
        if (updated.count <= 0) {
          newReactions = reactions.filter((_, i) => i !== existingIdx);
        } else {
          newReactions = reactions.map((r, i) =>
            i === existingIdx ? updated : r,
          );
        }
      } else {
        newReactions = reactions;
      }
      return {
        ...m,
        reactions: newReactions.length > 0 ? newReactions : undefined,
      };
    }),
  }));
}

export function setupWsHandlers() {
  if (_activeHandler) {
    getWs().offMessage(_activeHandler);
  }
  _activeHandler = (data) => {
    const cs = get(chatState);
    const vs = get(voiceStore);
    const currentUser = get(user);

    if (data.type === "message") {
      const msg = data.data;
      if (msg.channel_id === cs.selectedChannelId) {
        populateAvatarsFromMessages([msg]);
        chatState.update((s) => {
          const authorId = msg.author_id;
          if (authorId && s.typingUsers[authorId]) {
            clearTimeout(s.typingUsers[authorId].timeout);
            const { [authorId]: _, ...typingUsers } = s.typingUsers;
            return { ...s, messages: [...s.messages, msg], typingUsers };
          }
          return { ...s, messages: [...s.messages, msg] };
        });
      }
    }

    if (data.type === "channel_created") {
      const ch = data.data;
      serverState.update((s) => ({ ...s, channels: [...s.channels, ch] }));
    }
    if (data.type === "channel_updated") {
      const ch = data.data;
      serverState.update((s) => ({
        ...s,
        channels: s.channels.map((c) => (c.id === ch.id ? ch : c)),
      }));
      if (cs.selectedChannelId === ch.id) {
        chatState.update((s) => ({ ...s, selectedChannelName: ch.name }));
      }
    }
    if (data.type === "channel_deleted") {
      const { id } = data.data;
      serverState.update((s) => ({
        ...s,
        channels: s.channels.filter((c) => c.id !== id),
      }));
      if (cs.selectedChannelId === id) {
        const updatedChannels = get(serverState).channels;
        const firstText = updatedChannels.find(
          (c) => c.channel_type === "text",
        );
        if (firstText) {
          selectChannel(firstText.id, firstText.name);
        } else {
          chatState.update((s) => ({
            ...s,
            selectedChannelId: "",
            selectedChannelName: "",
            messages: [],
          }));
        }
      }
    }

    if (data.type === "user_online") {
      const presence = data.data;
      serverState.update((s) => {
        const exists = s.onlineUsers.find(
          (u) => u.user_id === presence.user_id,
        );
        const onlineUsers = exists
          ? s.onlineUsers
          : [...s.onlineUsers, presence];
        const userAvatars = presence.avatar_url
          ? {
              ...s.userAvatars,
              [presence.user_id]: API.getAvatarUrl(presence.user_id),
            }
          : s.userAvatars;
        return { ...s, onlineUsers, userAvatars };
      });
    }
    if (data.type === "user_offline") {
      const { user_id } = data.data;
      serverState.update((s) => ({
        ...s,
        onlineUsers: s.onlineUsers.filter((u) => u.user_id !== user_id),
      }));
    }

    if (data.type === "user_avatar_updated") {
      const { user_id, avatar_url } = data.data;
      serverState.update((s) => {
        if (avatar_url) {
          return {
            ...s,
            userAvatars: {
              ...s.userAvatars,
              [user_id]: API.getAvatarUrl(user_id) + "?t=" + Date.now(),
            },
          };
        } else {
          const { [user_id]: _, ...rest } = s.userAvatars;
          return { ...s, userAvatars: rest };
        }
      });
    }
    if (data.type === "user_profile_updated") {
      const { user_id, username, display_name, avatar_url } = data.data;
      serverState.update((s) => ({
        ...s,
        onlineUsers: s.onlineUsers.map((u) =>
          u.user_id === user_id
            ? { ...u, username, display_name: display_name ?? undefined }
            : u,
        ),
        userAvatars: avatar_url
          ? {
              ...s.userAvatars,
              [user_id]: API.getAvatarUrl(user_id) + "?t=" + Date.now(),
            }
          : s.userAvatars,
      }));
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === user_id
            ? { ...v, username, display_name: display_name ?? undefined }
            : v,
        ),
      }));
    }

    if (data.type === "message_edited") {
      const msg = data.data;
      if (msg.channel_id === cs.selectedChannelId) {
        chatState.update((s) => ({
          ...s,
          messages: s.messages.map((m) => (m.id === msg.id ? msg : m)),
        }));
      }
    }
    if (data.type === "message_deleted") {
      const { id, channel_id } = data.data;
      if (channel_id === cs.selectedChannelId) {
        chatState.update((s) => ({
          ...s,
          messages: s.messages.filter((m) => m.id !== id),
        }));
      }
    }

    if (data.type === "reaction_added" && cs.selectedChannelId) {
      const { message_id, emoji, user_id, username } = data.data;
      updateMessageReaction(message_id, emoji, user_id, username, true);
    }
    if (data.type === "reaction_removed" && cs.selectedChannelId) {
      const { message_id, emoji, user_id, username } = data.data;
      updateMessageReaction(message_id, emoji, user_id, username, false);
    }

    if (data.type === "link_preview_ready") {
      const { channel_id, message_id, link_previews } = data.data;
      if (channel_id === cs.selectedChannelId) {
        chatState.update((s) => ({
          ...s,
          messages: s.messages.map((m) =>
            m.id === message_id ? { ...m, link_previews } : m,
          ),
        }));
      }
    }

    if (data.type === "voice_user_joined") {
      const vs_data = data.data;
      voiceStore.update((s) => ({
        ...s,
        voiceStates: [
          ...s.voiceStates.filter((v) => v.user_id !== vs_data.user_id),
          vs_data,
        ],
      }));
      if (
        vs_data.channel_id === vs.currentVoiceChannel ||
        vs_data.user_id === currentUser?.id
      ) {
        playSound("connect");
      }
    }

    if (data.type === "new_producer") {
      const prod = data.data;
      if (
        prod.channel_id === vs.currentVoiceChannel &&
        prod.user_id !== currentUser?.id
      ) {
        if (prod.label !== "screen" && prod.label !== "camera") {
          voiceManager.consumeProducer(
            prod.producer_id,
            prod.user_id,
            prod.label,
          );
        } else if (
          prod.label === "screen" &&
          vs.watchingScreenUserId === prod.user_id
        ) {
          voiceManager.consumeProducer(
            prod.producer_id,
            prod.user_id,
            prod.label,
          );
        } else if (
          prod.label === "camera" &&
          vs.watchingCameraUserId === prod.user_id
        ) {
          voiceManager.consumeProducer(
            prod.producer_id,
            prod.user_id,
            prod.label,
          );
        }
      }
    }

    if (data.type === "voice_user_left") {
      const { user_id, channel_id } = data.data;
      if (channel_id === vs.currentVoiceChannel) {
        playSound("disconnect");
      }
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.filter(
          (v) => !(v.user_id === user_id && v.channel_id === channel_id),
        ),
      }));
      voiceManager.removeUserAudio(user_id);
    }

    if (data.type === "voice_state_updated") {
      const vs_data = data.data;
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === vs_data.user_id && v.channel_id === vs_data.channel_id
            ? vs_data
            : v,
        ),
      }));
    }

    if (data.type === "voice_speaking") {
      const { user_id, is_speaking } = data.data;
      voiceStore.update((s) => {
        const speakingUsers = is_speaking
          ? s.speakingUsers.includes(user_id)
            ? s.speakingUsers
            : [...s.speakingUsers, user_id]
          : s.speakingUsers.filter((id) => id !== user_id);
        return { ...s, speakingUsers };
      });
    }

    if (data.type === "screen_share_updated") {
      const { user_id, channel_id, is_screen_sharing } = data.data;
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === user_id && v.channel_id === channel_id
            ? { ...v, is_screen_sharing }
            : v,
        ),
        watchingScreenUserId:
          !is_screen_sharing && s.watchingScreenUserId === user_id
            ? null
            : s.watchingScreenUserId,
        watchingScreenUsername:
          !is_screen_sharing && s.watchingScreenUserId === user_id
            ? ""
            : s.watchingScreenUsername,
      }));
    }

    if (data.type === "camera_updated") {
      const { user_id, channel_id, is_camera_sharing } = data.data;
      voiceStore.update((s) => ({
        ...s,
        voiceStates: s.voiceStates.map((v) =>
          v.user_id === user_id && v.channel_id === channel_id
            ? { ...v, is_camera_sharing }
            : v,
        ),
        watchingCameraUserId:
          !is_camera_sharing && s.watchingCameraUserId === user_id
            ? null
            : s.watchingCameraUserId,
        watchingCameraUsername:
          !is_camera_sharing && s.watchingCameraUserId === user_id
            ? ""
            : s.watchingCameraUsername,
      }));
    }

    if (data.type === "user_kicked") {
      if (data.data.user_id === currentUser?.id) {
        alert("You have been kicked from the server.");
        AuthService.logout();
        goto("/auth");
        return;
      }
    }
    if (data.type === "user_banned") {
      if (data.data.user_id === currentUser?.id) {
        alert("You have been banned from the server.");
        AuthService.logout();
        goto("/auth");
        return;
      }
    }

    if (data.type === "user_role_changed") {
      const { user_id, new_role } = data.data;
      if (user_id === currentUser?.id && currentUser) {
        user.set({ ...currentUser, role: new_role });
      }
      serverState.update((s) => ({
        ...s,
        userRolesMap: { ...s.userRolesMap, [user_id]: new_role },
      }));
    }

    if (data.type === "user_renamed") {
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

    if (data.type === "error" && data.data.code === "rate_limited") {
      chatState.update((s) => ({ ...s, rateLimitWarning: true }));
      if (_rateLimitTimeout) clearTimeout(_rateLimitTimeout);
      _rateLimitTimeout = setTimeout(() => {
        chatState.update((s) => ({ ...s, rateLimitWarning: false }));
        _rateLimitTimeout = null;
      }, 3000);
    }

    if (data.type === "sync_required") {
      // The client fell too far behind on the broadcast channel.
      // Re-load the current channel's message history to recover state.
      const { selectedChannelId, selectedChannelName } = cs;
      if (selectedChannelId) {
        selectChannel(selectedChannelId, selectedChannelName);
      }
    }

    if (data.type === "typing") {
      const { user_id: userId, channel_id, username } = data.data;
      if (channel_id === cs.selectedChannelId) {
        if (userId === currentUser?.id) return;
        chatState.update((s) => {
          if (s.typingUsers[userId])
            clearTimeout(s.typingUsers[userId].timeout);
          const timeout = setTimeout(() => {
            chatState.update((inner) => {
              const { [userId]: _, ...typingUsers } = inner.typingUsers;
              return { ...inner, typingUsers };
            });
          }, TYPING_DISPLAY_MS);
          return {
            ...s,
            typingUsers: { ...s.typingUsers, [userId]: { username, timeout } },
          };
        });
      }
    }
  };
  getWs().onMessage(_activeHandler);
}
