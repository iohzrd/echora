<script lang="ts">
  import "../app.css";
  import { API, WebSocketManager, type Message } from "../lib/api";
  import { voiceManager } from "../lib/voice";
  import { playSound } from "../lib/sounds";
  import type { VoiceInputMode } from "../lib/voice";
  import { getChannelProducers } from "../lib/mediasoup";
  import { initPTT, switchInputMode, changePTTKey } from "../lib/ptt";
  import {
    loadAudioSettings,
    saveAudioSettings,
    enumerateAudioDevices,
    onDeviceChange,
    loadPerUserVolumes,
    savePerUserVolume,
  } from "../lib/audioSettings";
  import AuthService, { user } from "../lib/auth";
  import {
    isTauri,
    activeServer,
    servers,
    addServer,
    removeServer,
    setActiveServer,
    type EchoraServer,
  } from "../lib/serverManager";
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { get } from "svelte/store";

  import ServerSidebar from "../lib/components/ServerSidebar.svelte";
  import AddServerDialog from "../lib/components/AddServerDialog.svelte";
  import LoginForm from "../lib/components/LoginForm.svelte";
  import RegisterForm from "../lib/components/RegisterForm.svelte";
  import AdminPanel from "../lib/components/AdminPanel.svelte";
  import PasskeySettings from "../lib/components/PasskeySettings.svelte";
  import ProfileModal from "../lib/components/ProfileModal.svelte";
  import AppSidebar from "../lib/components/AppSidebar.svelte";
  import ChatArea from "../lib/components/ChatArea.svelte";
  import type MessageList from "../lib/components/MessageList.svelte";

  import { voiceStore, audioSettingsStore } from "../lib/stores/voiceStore";
  import { serverState } from "../lib/stores/serverState";
  import { chatState } from "../lib/stores/chatState";

  // Pure UI state that stays local
  let showAdminPanel = false;
  let showPasskeySettings = false;
  let showProfileModal = false;
  let profileViewUserId: string | null = null;
  let sidebarOpen = false;
  let showAddServerDialog = false;
  let needsServerAuth = false;
  let tauriAuthIsLogin = true;

  // Non-reactive refs / timers
  let wsManager = new WebSocketManager();
  voiceManager.setWebSocketManager(wsManager);
  let removeDeviceListener: (() => void) | null = null;
  let screenAudioElement: HTMLAudioElement | null = null;
  let messageList: MessageList;
  let screenVideoElement: HTMLVideoElement;
  let cameraVideoElement: HTMLVideoElement;
  let lastTypingSent = 0;
  let rateLimitTimeout: ReturnType<typeof setTimeout> | null = null;
  const TYPING_DEBOUNCE_MS = 3000;
  const TYPING_DISPLAY_MS = 5000;

  function populateAvatarsFromMessages(msgs: Message[]) {
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

  function syncVoiceState() {
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
            reactions.push({
              emoji,
              count: 1,
              reacted: userId === currentUser?.id,
            });
          }
        } else if (existing) {
          existing.count -= 1;
          if (userId === currentUser?.id) existing.reacted = false;
          if (existing.count <= 0) {
            reactions = reactions.filter((r) => r.emoji !== emoji);
          }
        }
        return {
          ...m,
          reactions: reactions.length > 0 ? reactions : undefined,
        };
      }),
    }));
  }

  function setupWsHandlers() {
    wsManager.onMessage((data) => {
      const cs = get(chatState);
      const vs = get(voiceStore);
      const ss = get(serverState);
      const currentUser = get(user);

      if (
        data.type === "message" &&
        data.data.channel_id === cs.selectedChannelId
      ) {
        const shouldScroll = messageList?.isNearBottom() ?? false;
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
          requestAnimationFrame(() => messageList?.scrollToBottom());
        }
      }

      if (data.type === "channel_created") {
        serverState.update((s) => ({
          ...s,
          channels: [...s.channels, data.data],
        }));
      }
      if (data.type === "channel_updated") {
        serverState.update((s) => ({
          ...s,
          channels: s.channels.map((c) =>
            c.id === data.data.id ? data.data : c,
          ),
        }));
        if (cs.selectedChannelId === data.data.id) {
          chatState.update((s) => ({
            ...s,
            selectedChannelName: data.data.name,
          }));
        }
      }
      if (data.type === "channel_deleted") {
        const updatedChannels = ss.channels.filter(
          (c) => c.id !== data.data.id,
        );
        serverState.update((s) => ({ ...s, channels: updatedChannels }));
        if (cs.selectedChannelId === data.data.id) {
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
        serverState.update((s) => {
          const exists = s.onlineUsers.find(
            (u) => u.user_id === data.data.user_id,
          );
          const onlineUsers = exists
            ? s.onlineUsers
            : [...s.onlineUsers, data.data];
          const userAvatars = data.data.avatar_url
            ? {
                ...s.userAvatars,
                [data.data.user_id]: API.getAvatarUrl(data.data.user_id),
              }
            : s.userAvatars;
          return { ...s, onlineUsers, userAvatars };
        });
      }
      if (data.type === "user_offline") {
        serverState.update((s) => ({
          ...s,
          onlineUsers: s.onlineUsers.filter(
            (u) => u.user_id !== data.data.user_id,
          ),
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
        const { user_id, username, avatar_url } = data.data;
        serverState.update((s) => ({
          ...s,
          onlineUsers: s.onlineUsers.map((u) =>
            u.user_id === user_id ? { ...u, username } : u,
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
            v.user_id === user_id ? { ...v, username } : v,
          ),
        }));
      }

      if (
        data.type === "message_edited" &&
        data.data.channel_id === cs.selectedChannelId
      ) {
        chatState.update((s) => ({
          ...s,
          messages: s.messages.map((m) =>
            m.id === data.data.id ? data.data : m,
          ),
        }));
      }
      if (
        data.type === "message_deleted" &&
        data.data.channel_id === cs.selectedChannelId
      ) {
        chatState.update((s) => ({
          ...s,
          messages: s.messages.filter((m) => m.id !== data.data.id),
        }));
      }

      if (data.type === "reaction_added" && cs.selectedChannelId) {
        updateMessageReaction(
          data.data.message_id,
          data.data.emoji,
          data.data.user_id,
          true,
        );
      }
      if (data.type === "reaction_removed" && cs.selectedChannelId) {
        updateMessageReaction(
          data.data.message_id,
          data.data.emoji,
          data.data.user_id,
          false,
        );
      }

      if (
        data.type === "link_preview_ready" &&
        data.data.channel_id === cs.selectedChannelId
      ) {
        const { message_id, link_previews } = data.data;
        chatState.update((s) => ({
          ...s,
          messages: s.messages.map((m) =>
            m.id === message_id ? { ...m, link_previews } : m,
          ),
        }));
      }

      if (data.type === "voice_user_joined") {
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
          playSound("connect");
        }
      }

      if (
        data.type === "new_producer" &&
        data.data.channel_id === vs.currentVoiceChannel &&
        data.data.user_id !== currentUser?.id
      ) {
        if (data.data.label !== "screen" && data.data.label !== "camera") {
          voiceManager.consumeProducer(
            data.data.producer_id,
            data.data.user_id,
            data.data.label,
          );
        } else if (
          data.data.label === "screen" &&
          vs.watchingScreenUserId === data.data.user_id
        ) {
          voiceManager.consumeProducer(
            data.data.producer_id,
            data.data.user_id,
            data.data.label,
          );
        } else if (
          data.data.label === "camera" &&
          vs.watchingCameraUserId === data.data.user_id
        ) {
          voiceManager.consumeProducer(
            data.data.producer_id,
            data.data.user_id,
            data.data.label,
          );
        }
      }

      if (data.type === "voice_user_left") {
        if (data.data.channel_id === vs.currentVoiceChannel) {
          playSound("disconnect");
        }
        voiceStore.update((s) => ({
          ...s,
          voiceStates: s.voiceStates.filter(
            (v) =>
              !(
                v.user_id === data.data.user_id &&
                v.channel_id === data.data.channel_id
              ),
          ),
        }));
        voiceManager.removeUserAudio(data.data.user_id);
      }

      if (data.type === "voice_state_updated") {
        voiceStore.update((s) => ({
          ...s,
          voiceStates: s.voiceStates.map((v) =>
            v.user_id === data.data.user_id &&
            v.channel_id === data.data.channel_id
              ? data.data
              : v,
          ),
        }));
      }

      if (data.type === "voice_speaking") {
        updateSpeaking(data.data.user_id, data.data.is_speaking);
      }

      if (data.type === "screen_share_updated") {
        voiceStore.update((s) => ({
          ...s,
          voiceStates: s.voiceStates.map((v) =>
            v.user_id === data.data.user_id &&
            v.channel_id === data.data.channel_id
              ? { ...v, is_screen_sharing: data.data.is_screen_sharing }
              : v,
          ),
        }));
        if (
          !data.data.is_screen_sharing &&
          vs.watchingScreenUserId === data.data.user_id
        ) {
          stopWatching();
        }
      }

      if (data.type === "camera_updated") {
        voiceStore.update((s) => ({
          ...s,
          voiceStates: s.voiceStates.map((v) =>
            v.user_id === data.data.user_id &&
            v.channel_id === data.data.channel_id
              ? { ...v, is_camera_sharing: data.data.is_camera_sharing }
              : v,
          ),
        }));
        if (
          !data.data.is_camera_sharing &&
          vs.watchingCameraUserId === data.data.user_id
        ) {
          stopWatchingCamera();
        }
      }

      if (
        data.type === "user_kicked" &&
        data.data.user_id === currentUser?.id
      ) {
        alert("You have been kicked from the server.");
        AuthService.logout();
        goto("/auth");
        return;
      }
      if (
        data.type === "user_banned" &&
        data.data.user_id === currentUser?.id
      ) {
        alert("You have been banned from the server.");
        AuthService.logout();
        goto("/auth");
        return;
      }

      if (data.type === "user_role_changed") {
        if (data.data.user_id === currentUser?.id && currentUser) {
          user.set({ ...currentUser, role: data.data.new_role });
        }
        serverState.update((s) => ({
          ...s,
          userRolesMap: {
            ...s.userRolesMap,
            [data.data.user_id]: data.data.new_role,
          },
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
        if (rateLimitTimeout) clearTimeout(rateLimitTimeout);
        rateLimitTimeout = setTimeout(() => {
          chatState.update((s) => ({ ...s, rateLimitWarning: false }));
          rateLimitTimeout = null;
        }, 3000);
      }

      if (
        data.type === "typing" &&
        data.data.channel_id === cs.selectedChannelId
      ) {
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

  function setupVoiceHandlers() {
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
      if (track.kind === "video") {
        if (screenVideoElement) {
          screenVideoElement.srcObject = new MediaStream([track]);
          screenVideoElement
            .play()
            .catch((e) => console.warn("Screen video autoplay prevented:", e));
        }
      } else if (track.kind === "audio") {
        if (screenAudioElement) {
          screenAudioElement.srcObject = null;
          screenAudioElement.remove();
        }
        screenAudioElement = document.createElement("audio");
        screenAudioElement.autoplay = true;
        screenAudioElement.volume = 1.0;
        screenAudioElement.srcObject = new MediaStream([track]);
        document.body.appendChild(screenAudioElement);
        screenAudioElement
          .play()
          .catch((e) => console.warn("Screen audio autoplay prevented:", e));
      }
    });

    voiceManager.onCameraTrack((track) => {
      if (track.kind === "video" && cameraVideoElement) {
        cameraVideoElement.srcObject = new MediaStream([track]);
        cameraVideoElement
          .play()
          .catch((e) => console.warn("Camera video autoplay prevented:", e));
      }
    });
  }

  async function connectToServer() {
    // Reset stores for new server connection
    chatState.update((s) => {
      s.typingUsers.forEach((u) => clearTimeout(u.timeout));
      return {
        ...s,
        messages: [],
        selectedChannelId: "",
        selectedChannelName: "",
        hasMoreMessages: true,
        loadingMore: false,
        editingMessageId: null,
        editMessageContent: "",
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
      serverName: "",
      backendVersion: "",
      tauriVersion: get(serverState).tauriVersion,
      customEmojis: [],
    });
    voiceStore.update((s) => ({
      ...s,
      voiceStates: [],
      speakingUsers: new Set(),
    }));
    needsServerAuth = false;

    if (isTauri && !$activeServer) {
      showAddServerDialog = true;
      return;
    }

    await AuthService.init();

    if (!$user) {
      if (isTauri) {
        needsServerAuth = true;
        return;
      }
      goto("/auth");
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
      if ($user?.avatar_url) avatarMap[$user.id] = API.getAvatarUrl($user.id);

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

      wsManager = new WebSocketManager();
      voiceManager.setWebSocketManager(wsManager);
      setupWsHandlers();
      wsManager.onReconnect(() => voiceManager.reconcileProducers());

      await wsManager.connect();
      syncVoiceState();

      const { channels } = get(serverState);
      if (channels.length > 0) {
        const firstTextChannel = channels.find(
          (c) => c.channel_type === "text",
        );
        if (firstTextChannel) {
          await selectChannel(firstTextChannel.id, firstTextChannel.name);
        }
      }
    } catch (error) {
      console.error("Failed to load initial data:", error);
      if (error instanceof Error && error.message.includes("401")) {
        AuthService.logout();
        goto("/auth");
      }
    }
  }

  async function initAudioSettings() {
    const settings = loadAudioSettings();
    audioSettingsStore.set({
      inputDeviceId: settings.inputDeviceId,
      outputDeviceId: settings.outputDeviceId,
      inputGain: settings.inputGain,
      outputVolume: settings.outputVolume,
      vadSensitivity: settings.vadSensitivity,
      noiseSuppression: settings.noiseSuppression,
      inputDevices: [],
      outputDevices: [],
    });

    voiceManager.setInputGain(settings.inputGain);
    voiceManager.setOutputVolume(settings.outputVolume);
    voiceManager.setSpeakingThreshold(settings.vadSensitivity);

    const perUserVols = loadPerUserVolumes();
    for (const [userId, vol] of Object.entries(perUserVols)) {
      voiceManager.setUserVolume(userId, vol);
    }

    await refreshDeviceList();
    removeDeviceListener = onDeviceChange(() => refreshDeviceList());
  }

  async function refreshDeviceList() {
    const devices = await enumerateAudioDevices();
    audioSettingsStore.update((s) => ({
      ...s,
      inputDevices: devices.inputs,
      outputDevices: devices.outputs,
    }));
  }

  onMount(async () => {
    setupVoiceHandlers();
    const pttSettings = await initPTT();
    voiceStore.update((s) => ({
      ...s,
      voiceInputMode: pttSettings.inputMode,
      pttKey: pttSettings.pttKey,
    }));

    if (isTauri) {
      import("@tauri-apps/api/app")
        .then((m) => m.getVersion())
        .then((v) => serverState.update((s) => ({ ...s, tauriVersion: v })))
        .catch(() => {});
    }

    await initAudioSettings();
    await connectToServer();
  });

  onDestroy(() => {
    wsManager.disconnect();
    removeDeviceListener?.();
  });

  async function tryAction(action: () => Promise<unknown>, context: string) {
    try {
      await action();
    } catch (error) {
      console.error(`Failed to ${context}:`, error);
    }
  }

  async function handleSelectServer(server: EchoraServer) {
    if ($activeServer?.id === server.id) return;
    wsManager.disconnect();
    const { currentVoiceChannel } = get(voiceStore);
    if (currentVoiceChannel) {
      await voiceManager.leaveVoiceChannel().catch(() => {});
    }
    setActiveServer(server.id);
    await connectToServer();
  }

  function handleAddServer(url: string, name: string) {
    const server = addServer(url, name);
    showAddServerDialog = false;
    setActiveServer(server.id);
    connectToServer();
  }

  function handleTauriAuthSuccess() {
    needsServerAuth = false;
    tauriAuthIsLogin = true;
    connectToServer();
  }

  function handleRemoveServer(id: string) {
    if (!confirm("Remove this server from your list?")) return;
    const wasActive = $activeServer?.id === id;
    removeServer(id);
    if (wasActive && $servers.length > 0) {
      connectToServer();
    }
  }

  function handleCreateChannel(name: string, type: "text" | "voice") {
    tryAction(() => API.createChannel(name, type), "create channel");
  }

  function handleUpdateChannel(channelId: string, name: string) {
    tryAction(() => API.updateChannel(channelId, name), "update channel");
  }

  async function handleDeleteChannel(channelId: string) {
    if (!confirm("Delete this channel and all its messages?")) return;
    tryAction(() => API.deleteChannel(channelId), "delete channel");
  }

  function startEditMessage(message: Message) {
    chatState.update((s) => ({
      ...s,
      editingMessageId: message.id,
      editMessageContent: message.content,
    }));
  }

  function cancelEditMessage() {
    chatState.update((s) => ({
      ...s,
      editingMessageId: null,
      editMessageContent: "",
    }));
  }

  async function saveEditMessage() {
    const { editingMessageId, editMessageContent, selectedChannelId } =
      get(chatState);
    if (!editingMessageId || !editMessageContent.trim()) return;
    tryAction(async () => {
      await API.editMessage(
        selectedChannelId,
        editingMessageId,
        editMessageContent.trim(),
      );
      chatState.update((s) => ({
        ...s,
        editingMessageId: null,
        editMessageContent: "",
      }));
    }, "edit message");
  }

  async function deleteMessage(messageId: string) {
    if (!confirm("Delete this message?")) return;
    const { selectedChannelId } = get(chatState);
    tryAction(
      () => API.deleteMessage(selectedChannelId, messageId),
      "delete message",
    );
  }

  function startReply(message: Message) {
    chatState.update((s) => ({ ...s, replyingTo: message }));
  }

  function cancelReply() {
    chatState.update((s) => ({ ...s, replyingTo: null }));
  }

  async function toggleReaction(messageId: string, emoji: string) {
    const { selectedChannelId, messages } = get(chatState);
    if (!selectedChannelId) return;
    tryAction(async () => {
      const msg = messages.find((m) => m.id === messageId);
      const existingReaction = msg?.reactions?.find((r) => r.emoji === emoji);
      if (existingReaction?.reacted) {
        await API.removeReaction(selectedChannelId, messageId, emoji);
      } else {
        await API.addReaction(selectedChannelId, messageId, emoji);
      }
    }, "toggle reaction");
  }

  function joinVoiceChannel(channelId: string) {
    tryAction(
      () => voiceManager.joinVoiceChannel(channelId),
      "join voice channel",
    );
  }

  function leaveVoiceChannel() {
    tryAction(() => voiceManager.leaveVoiceChannel(), "leave voice channel");
  }

  function toggleMute() {
    tryAction(() => voiceManager.toggleMute(), "toggle mute");
  }

  function toggleDeafen() {
    tryAction(() => voiceManager.toggleDeafen(), "toggle deafen");
  }

  async function handleSwitchInputMode(mode: VoiceInputMode) {
    const { pttKey } = get(voiceStore);
    await switchInputMode(mode, pttKey);
    voiceStore.update((s) => ({ ...s, voiceInputMode: mode }));
    syncVoiceState();
  }

  async function handleChangePTTKey(key: string) {
    voiceStore.update((s) => ({ ...s, pttKey: key }));
    await changePTTKey(key);
  }

  function saveCurrentAudioSettings() {
    const s = get(audioSettingsStore);
    saveAudioSettings({
      inputDeviceId: s.inputDeviceId,
      outputDeviceId: s.outputDeviceId,
      inputGain: s.inputGain,
      outputVolume: s.outputVolume,
      vadSensitivity: s.vadSensitivity,
      noiseSuppression: s.noiseSuppression,
    });
  }

  function handleInputDeviceChange(deviceId: string) {
    audioSettingsStore.update((s) => ({ ...s, inputDeviceId: deviceId }));
    voiceManager.setInputDevice(deviceId);
    saveCurrentAudioSettings();
  }

  function handleOutputDeviceChange(deviceId: string) {
    audioSettingsStore.update((s) => ({ ...s, outputDeviceId: deviceId }));
    voiceManager.setOutputDevice(deviceId);
    saveCurrentAudioSettings();
  }

  function handleInputGainChange(gain: number) {
    audioSettingsStore.update((s) => ({ ...s, inputGain: gain }));
    voiceManager.setInputGain(gain);
    saveCurrentAudioSettings();
  }

  function handleOutputVolumeChange(volume: number) {
    audioSettingsStore.update((s) => ({ ...s, outputVolume: volume }));
    voiceManager.setOutputVolume(volume);
    saveCurrentAudioSettings();
  }

  function handleVadSensitivityChange(sensitivity: number) {
    audioSettingsStore.update((s) => ({ ...s, vadSensitivity: sensitivity }));
    voiceManager.setSpeakingThreshold(sensitivity);
    saveCurrentAudioSettings();
  }

  function handleNoiseSuppressionToggle(enabled: boolean) {
    audioSettingsStore.update((s) => ({ ...s, noiseSuppression: enabled }));
    voiceManager.setNoiseSuppression(enabled);
    saveCurrentAudioSettings();
  }

  function handleUserVolumeChange(userId: string, volume: number) {
    voiceManager.setUserVolume(userId, volume);
    savePerUserVolume(userId, volume);
  }

  function handleGetUserVolume(userId: string): number {
    return voiceManager.getUserVolume(userId);
  }

  async function toggleScreenShare() {
    const { isScreenSharing } = get(voiceStore);
    try {
      if (isScreenSharing) {
        await voiceManager.stopScreenShare();
      } else {
        await voiceManager.startScreenShare();
      }
    } catch (error) {
      if (error instanceof Error && error.name === "NotAllowedError") return;
      console.error("Failed to toggle screen share:", error);
    }
  }

  async function watchScreen(userId: string, username: string) {
    voiceStore.update((s) => ({
      ...s,
      watchingScreenUserId: userId,
      watchingScreenUsername: username,
    }));
    const { currentVoiceChannel } = get(voiceStore);
    if (!currentVoiceChannel) return;
    try {
      const producers = await getChannelProducers(currentVoiceChannel);
      for (const producer of producers) {
        if (producer.user_id === userId && producer.label === "screen") {
          await voiceManager.consumeProducer(
            producer.producer_id,
            userId,
            producer.label,
          );
        }
      }
    } catch (error) {
      console.error("Failed to consume screen share producer:", error);
    }
  }

  function stopWatching() {
    voiceStore.update((s) => ({
      ...s,
      watchingScreenUserId: null,
      watchingScreenUsername: "",
    }));
    if (screenVideoElement) screenVideoElement.srcObject = null;
    if (screenAudioElement) {
      screenAudioElement.srcObject = null;
      screenAudioElement.remove();
      screenAudioElement = null;
    }
  }

  async function toggleCamera() {
    const { isCameraSharing } = get(voiceStore);
    try {
      if (isCameraSharing) {
        await voiceManager.stopCamera();
      } else {
        await voiceManager.startCamera();
      }
    } catch (error) {
      if (error instanceof Error && error.name === "NotAllowedError") return;
      console.error("Failed to toggle camera:", error);
    }
  }

  async function watchCamera(userId: string, username: string) {
    voiceStore.update((s) => ({
      ...s,
      watchingCameraUserId: userId,
      watchingCameraUsername: username,
    }));
    const { currentVoiceChannel } = get(voiceStore);
    if (!currentVoiceChannel) return;
    try {
      const producers = await getChannelProducers(currentVoiceChannel);
      for (const producer of producers) {
        if (producer.user_id === userId && producer.label === "camera") {
          await voiceManager.consumeProducer(
            producer.producer_id,
            userId,
            producer.label,
          );
        }
      }
    } catch (error) {
      console.error("Failed to consume camera producer:", error);
    }
  }

  function stopWatchingCamera() {
    voiceStore.update((s) => ({
      ...s,
      watchingCameraUserId: null,
      watchingCameraUsername: "",
    }));
    if (cameraVideoElement) cameraVideoElement.srcObject = null;
  }

  async function selectChannel(channelId: string, channelName: string) {
    sidebarOpen = false;
    chatState.update((s) => {
      s.typingUsers.forEach((u) => clearTimeout(u.timeout));
      return {
        ...s,
        selectedChannelId: channelId,
        selectedChannelName: channelName,
        hasMoreMessages: true,
        replyingTo: null,
        typingUsers: new Map(),
      };
    });
    wsManager.joinChannel(channelId);

    try {
      const msgs = await API.getMessages(channelId, 50);
      populateAvatarsFromMessages(msgs);
      chatState.update((s) => ({
        ...s,
        messages: msgs,
        hasMoreMessages: msgs.length >= 50,
      }));
      requestAnimationFrame(() => messageList?.scrollToBottom());
    } catch (error) {
      console.error("Failed to load messages:", error);
      chatState.update((s) => ({ ...s, messages: [] }));
    }
  }

  async function loadOlderMessages() {
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
      chatState.update((s) => ({
        ...s,
        hasMoreMessages: olderMessages.length >= 50,
        messages:
          olderMessages.length > 0
            ? (() => {
                messageList?.preserveScroll(() => {});
                return [...olderMessages, ...s.messages];
              })()
            : s.messages,
      }));
      if (olderMessages.length > 0) {
        messageList?.preserveScroll(() => {});
      }
    } catch (error) {
      console.error("Failed to load older messages:", error);
    } finally {
      chatState.update((s) => ({ ...s, loadingMore: false }));
    }
  }

  function handleSendMessage(text: string, attachmentIds?: string[]) {
    const { selectedChannelId, replyingTo } = get(chatState);
    if (selectedChannelId && $user) {
      try {
        wsManager.sendMessage(
          selectedChannelId,
          text,
          replyingTo?.id,
          attachmentIds,
        );
        chatState.update((s) => ({ ...s, replyingTo: null }));
      } catch (error) {
        console.error("Failed to send message:", error);
      }
    }
  }

  function handleTyping() {
    const now = Date.now();
    const { selectedChannelId } = get(chatState);
    if (selectedChannelId && now - lastTypingSent > TYPING_DEBOUNCE_MS) {
      lastTypingSent = now;
      wsManager.sendTyping(selectedChannelId);
    }
  }

  function handleUserClick(userId: string) {
    if (userId === $user?.id) {
      showProfileModal = true;
    } else {
      profileViewUserId = userId;
    }
  }

  function logout() {
    AuthService.logout();
    goto("/auth");
  }

  $: isMod =
    $user?.role === "moderator" ||
    $user?.role === "admin" ||
    $user?.role === "owner";
  $: activeServerName =
    $serverState.serverName || $activeServer?.name || "Echora";
</script>

<div class="layout">
  {#if sidebarOpen}
    <div
      class="sidebar-overlay"
      on:click={() => (sidebarOpen = false)}
      role="presentation"
    ></div>
  {/if}

  {#if isTauri}
    <ServerSidebar
      onSelectServer={handleSelectServer}
      onAddServer={() => (showAddServerDialog = true)}
      onRemoveServer={handleRemoveServer}
    />
  {/if}

  {#if isTauri && !$activeServer}
    <div class="main-content tauri-empty-state">
      <div class="empty-state-message">
        <h2>Welcome to Echora</h2>
        <p>Add a server to get started.</p>
        <button
          class="submit-btn"
          on:click={() => (showAddServerDialog = true)}
        >
          Add Server
        </button>
      </div>
    </div>
  {:else if isTauri && needsServerAuth}
    <div class="sidebar">
      <div class="channels-area">
        <div class="server-header">
          <span>{activeServerName}</span>
        </div>
      </div>
    </div>
    <div class="main-content tauri-auth-state">
      <div class="auth-container">
        <div class="auth-content">
          {#if tauriAuthIsLogin}
            <LoginForm onSuccess={handleTauriAuthSuccess} />
          {:else}
            <RegisterForm onSuccess={handleTauriAuthSuccess} />
          {/if}
          <div class="auth-toggle">
            {#if tauriAuthIsLogin}
              <span>Need an account?</span>
              <button
                on:click={() => (tauriAuthIsLogin = false)}
                class="toggle-btn">Register</button
              >
            {:else}
              <span>Already have an account?</span>
              <button
                on:click={() => (tauriAuthIsLogin = true)}
                class="toggle-btn">Login</button
              >
            {/if}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <AppSidebar
      {isMod}
      {sidebarOpen}
      onShowAdminPanel={() => (showAdminPanel = true)}
      onShowPasskeySettings={() => (showPasskeySettings = true)}
      onLogout={logout}
      onShowProfileModal={() => (showProfileModal = true)}
      onSelectChannel={selectChannel}
      onCreateChannel={handleCreateChannel}
      onUpdateChannel={handleUpdateChannel}
      onDeleteChannel={handleDeleteChannel}
      onJoinVoice={joinVoiceChannel}
      onWatchScreen={watchScreen}
      onWatchCamera={watchCamera}
      onUserVolumeChange={handleUserVolumeChange}
      getUserVolume={handleGetUserVolume}
      onUserClick={handleUserClick}
      onLeaveVoice={leaveVoiceChannel}
      onToggleMute={toggleMute}
      onToggleDeafen={toggleDeafen}
      onToggleScreenShare={toggleScreenShare}
      onToggleCamera={toggleCamera}
      onSwitchInputMode={handleSwitchInputMode}
      onChangePTTKey={handleChangePTTKey}
      onInputDeviceChange={handleInputDeviceChange}
      onOutputDeviceChange={handleOutputDeviceChange}
      onInputGainChange={handleInputGainChange}
      onOutputVolumeChange={handleOutputVolumeChange}
      onVadSensitivityChange={handleVadSensitivityChange}
      onNoiseSuppressionToggle={handleNoiseSuppressionToggle}
    />

    <ChatArea
      bind:messageList
      bind:screenVideoElement
      bind:cameraVideoElement
      onToggleSidebar={() => (sidebarOpen = !sidebarOpen)}
      onStopWatching={stopWatching}
      onStopWatchingCamera={stopWatchingCamera}
      onScrollTop={loadOlderMessages}
      onStartEdit={startEditMessage}
      onSaveEdit={saveEditMessage}
      onCancelEdit={cancelEditMessage}
      onDeleteMessage={deleteMessage}
      onReply={startReply}
      onToggleReaction={toggleReaction}
      onUserClick={handleUserClick}
      onSend={handleSendMessage}
      onTyping={handleTyping}
      onCancelReply={cancelReply}
    />
  {/if}
</div>

{#if showAddServerDialog}
  <AddServerDialog
    onAdd={handleAddServer}
    onCancel={() => (showAddServerDialog = false)}
  />
{/if}

{#if showAdminPanel}
  <AdminPanel onClose={() => (showAdminPanel = false)} />
{/if}

{#if showPasskeySettings}
  <PasskeySettings onClose={() => (showPasskeySettings = false)} />
{/if}

{#if showProfileModal}
  <ProfileModal onClose={() => (showProfileModal = false)} />
{/if}

{#if profileViewUserId}
  <ProfileModal
    viewUserId={profileViewUserId}
    onClose={() => (profileViewUserId = null)}
  />
{/if}
