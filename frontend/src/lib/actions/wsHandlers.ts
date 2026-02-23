import { goto } from "$app/navigation";
import { API, type MemberInfo, type WsIncomingMessage } from "../api";
import { playSound } from "../sounds";
import { playSoundboardAudio } from "../soundboardAudio";
import AuthService from "../auth";
import { voiceManager } from "../voice";
import { getWs } from "../ws";
import { voiceStore } from "../stores/voiceStore.svelte";
import { serverState } from "../stores/serverState.svelte";
import { chatState } from "../stores/chatState.svelte";
import { authState } from "../stores/authState.svelte";
import { soundboardStore } from "../stores/soundboardStore.svelte";
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
  const currentUser = authState.user;
  chatState.messages = chatState.messages.map((m) => {
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
  });
}

export function setupWsHandlers() {
  if (_activeHandler) {
    getWs().offMessage(_activeHandler);
  }
  _activeHandler = (data) => {
    const currentUser = authState.user;

    if (data.type === "message") {
      const msg = data.data;
      if (msg.channel_id === chatState.selectedChannelId) {
        populateAvatarsFromMessages([msg]);
        const authorId = msg.author_id;
        if (authorId && chatState.typingUsers[authorId]) {
          clearTimeout(chatState.typingUsers[authorId].timeout);
          const { [authorId]: _, ...typingUsers } = chatState.typingUsers;
          chatState.typingUsers = typingUsers;
        }
        chatState.messages = [...chatState.messages, msg];
      }
    }

    if (data.type === "channel_created") {
      const ch = data.data;
      serverState.channels = [...serverState.channels, ch];
    }
    if (data.type === "channel_updated") {
      const ch = data.data;
      serverState.channels = serverState.channels.map((c) =>
        c.id === ch.id ? ch : c,
      );
      if (chatState.selectedChannelId === ch.id) {
        chatState.selectedChannelName = ch.name;
      }
    }
    if (data.type === "channel_deleted") {
      const { id } = data.data;
      serverState.channels = serverState.channels.filter((c) => c.id !== id);
      if (chatState.selectedChannelId === id) {
        const firstText = serverState.channels.find(
          (c) => c.channel_type === "text",
        );
        if (firstText) {
          selectChannel(firstText.id, firstText.name);
        } else {
          chatState.selectedChannelId = "";
          chatState.selectedChannelName = "";
          chatState.messages = [];
        }
      }
    }

    if (data.type === "user_online") {
      const presence = data.data;
      const exists = serverState.onlineUsers.find(
        (u) => u.user_id === presence.user_id,
      );
      if (!exists) {
        serverState.onlineUsers = [...serverState.onlineUsers, presence];
      }
      if (presence.avatar_url) {
        serverState.userAvatars[presence.user_id] = API.getAvatarUrl(
          presence.user_id,
        );
      }
    }
    if (data.type === "user_offline") {
      const { user_id } = data.data;
      serverState.onlineUsers = serverState.onlineUsers.filter(
        (u) => u.user_id !== user_id,
      );
    }

    if (data.type === "user_avatar_updated") {
      const { user_id, avatar_url } = data.data;
      if (avatar_url) {
        serverState.userAvatars[user_id] =
          API.getAvatarUrl(user_id) + "?t=" + Date.now();
      } else {
        delete serverState.userAvatars[user_id];
      }
    }
    if (data.type === "user_profile_updated") {
      const { user_id, username, display_name, avatar_url } = data.data;
      serverState.onlineUsers = serverState.onlineUsers.map((u) =>
        u.user_id === user_id
          ? { ...u, username, display_name: display_name ?? undefined }
          : u,
      );
      serverState.members = serverState.members.map((m) =>
        m.id === user_id
          ? { ...m, username, display_name: display_name ?? undefined }
          : m,
      );
      if (avatar_url) {
        serverState.userAvatars[user_id] =
          API.getAvatarUrl(user_id) + "?t=" + Date.now();
      }
      voiceStore.voiceStates = voiceStore.voiceStates.map((v) =>
        v.user_id === user_id
          ? { ...v, username, display_name: display_name ?? undefined }
          : v,
      );
      chatState.messages = chatState.messages.map((m) =>
        m.author_id === user_id
          ? { ...m, username, display_name: display_name ?? undefined }
          : m,
      );
    }

    if (data.type === "message_edited") {
      const msg = data.data;
      if (msg.channel_id === chatState.selectedChannelId) {
        chatState.messages = chatState.messages.map((m) =>
          m.id === msg.id ? msg : m,
        );
      }
    }
    if (data.type === "message_deleted") {
      const { id, channel_id } = data.data;
      if (channel_id === chatState.selectedChannelId) {
        chatState.messages = chatState.messages.filter((m) => m.id !== id);
      }
    }

    if (data.type === "reaction_added" && chatState.selectedChannelId) {
      const { message_id, emoji, user_id, username } = data.data;
      updateMessageReaction(message_id, emoji, user_id, username, true);
    }
    if (data.type === "reaction_removed" && chatState.selectedChannelId) {
      const { message_id, emoji, user_id, username } = data.data;
      updateMessageReaction(message_id, emoji, user_id, username, false);
    }

    if (data.type === "link_preview_ready") {
      const { channel_id, message_id, link_previews } = data.data;
      if (channel_id === chatState.selectedChannelId) {
        chatState.messages = chatState.messages.map((m) =>
          m.id === message_id ? { ...m, link_previews } : m,
        );
      }
    }

    if (data.type === "voice_user_joined") {
      const vs_data = data.data;
      voiceStore.voiceStates = [
        ...voiceStore.voiceStates.filter((v) => v.user_id !== vs_data.user_id),
        vs_data,
      ];
      if (
        vs_data.channel_id === voiceStore.currentVoiceChannel ||
        vs_data.user_id === currentUser?.id
      ) {
        playSound("connect");
      }
    }

    if (data.type === "new_producer") {
      const prod = data.data;
      if (
        prod.channel_id === voiceStore.currentVoiceChannel &&
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
          voiceStore.watchingScreenUserId === prod.user_id
        ) {
          voiceManager.consumeProducer(
            prod.producer_id,
            prod.user_id,
            prod.label,
          );
        } else if (
          prod.label === "camera" &&
          voiceStore.watchingCameraUserId === prod.user_id
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
      if (channel_id === voiceStore.currentVoiceChannel) {
        playSound("disconnect");
      }
      voiceStore.voiceStates = voiceStore.voiceStates.filter(
        (v) => !(v.user_id === user_id && v.channel_id === channel_id),
      );
      voiceManager.removeUserAudio(user_id);
    }

    if (data.type === "voice_state_updated") {
      const vs_data = data.data;
      voiceStore.voiceStates = voiceStore.voiceStates.map((v) =>
        v.user_id === vs_data.user_id && v.channel_id === vs_data.channel_id
          ? vs_data
          : v,
      );
    }

    if (data.type === "voice_speaking") {
      const { user_id, is_speaking } = data.data;
      if (is_speaking) {
        if (!voiceStore.speakingUsers.includes(user_id)) {
          voiceStore.speakingUsers = [...voiceStore.speakingUsers, user_id];
        }
      } else {
        voiceStore.speakingUsers = voiceStore.speakingUsers.filter(
          (id) => id !== user_id,
        );
      }
    }

    if (data.type === "screen_share_updated") {
      const { user_id, channel_id, is_screen_sharing } = data.data;
      voiceStore.voiceStates = voiceStore.voiceStates.map((v) =>
        v.user_id === user_id && v.channel_id === channel_id
          ? { ...v, is_screen_sharing }
          : v,
      );
      if (!is_screen_sharing && voiceStore.watchingScreenUserId === user_id) {
        voiceStore.watchingScreenUserId = null;
        voiceStore.watchingScreenUsername = "";
      }
    }

    if (data.type === "camera_updated") {
      const { user_id, channel_id, is_camera_sharing } = data.data;
      voiceStore.voiceStates = voiceStore.voiceStates.map((v) =>
        v.user_id === user_id && v.channel_id === channel_id
          ? { ...v, is_camera_sharing }
          : v,
      );
      if (!is_camera_sharing && voiceStore.watchingCameraUserId === user_id) {
        voiceStore.watchingCameraUserId = null;
        voiceStore.watchingCameraUsername = "";
      }
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
        authState.user = { ...currentUser, role: new_role };
      }
      serverState.userRolesMap = {
        ...serverState.userRolesMap,
        [user_id]: new_role,
      };
      serverState.members = serverState.members.map((m) =>
        m.id === user_id ? { ...m, role: new_role as MemberInfo["role"] } : m,
      );
    }

    if (data.type === "user_renamed") {
      const { user_id, new_username } = data.data;
      serverState.onlineUsers = serverState.onlineUsers.map((u) =>
        u.user_id === user_id ? { ...u, username: new_username } : u,
      );
      serverState.members = serverState.members.map((m) =>
        m.id === user_id ? { ...m, username: new_username } : m,
      );
      voiceStore.voiceStates = voiceStore.voiceStates.map((v) =>
        v.user_id === user_id ? { ...v, username: new_username } : v,
      );
    }

    if (data.type === "error" && data.data.code === "rate_limited") {
      chatState.rateLimitWarning = true;
      if (_rateLimitTimeout) clearTimeout(_rateLimitTimeout);
      _rateLimitTimeout = setTimeout(() => {
        chatState.rateLimitWarning = false;
        _rateLimitTimeout = null;
      }, 3000);
    }

    if (data.type === "sync_required") {
      const { selectedChannelId, selectedChannelName } = chatState;
      if (selectedChannelId) {
        selectChannel(selectedChannelId, selectedChannelName);
      }
    }

    if (data.type === "typing") {
      const { user_id: userId, channel_id, username } = data.data;
      if (channel_id === chatState.selectedChannelId) {
        if (userId === currentUser?.id) return;
        if (chatState.typingUsers[userId])
          clearTimeout(chatState.typingUsers[userId].timeout);
        const timeout = setTimeout(() => {
          const { [userId]: _, ...typingUsers } = chatState.typingUsers;
          chatState.typingUsers = typingUsers;
        }, TYPING_DISPLAY_MS);
        chatState.typingUsers = {
          ...chatState.typingUsers,
          [userId]: { username, timeout },
        };
      }
    }

    // --- Soundboard events ---

    if (data.type === "soundboard_sound_created") {
      soundboardStore.sounds = [...soundboardStore.sounds, data.data];
    }

    if (data.type === "soundboard_sound_updated") {
      soundboardStore.sounds = soundboardStore.sounds.map((s) =>
        s.id === data.data.id ? data.data : s,
      );
    }

    if (data.type === "soundboard_sound_deleted") {
      soundboardStore.sounds = soundboardStore.sounds.filter(
        (s) => s.id !== data.data.sound_id,
      );
      soundboardStore.favorites = soundboardStore.favorites.filter(
        (id) => id !== data.data.sound_id,
      );
    }

    if (data.type === "soundboard_play") {
      const { channel_id, sound_id, sound_volume } = data.data;
      // Play only if user is in the same voice channel and not deafened
      if (
        channel_id === voiceStore.currentVoiceChannel &&
        !voiceStore.isDeafened
      ) {
        playSoundboardAudio(sound_id, sound_volume);
      }
    }
  };
  getWs().onMessage(_activeHandler);
}
