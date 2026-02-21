<script lang="ts">
  import "../app.css";
  import {
    API,
    FRONTEND_VERSION,
    WebSocketManager,
    type Channel,
    type Message,
    type VoiceState,
    type UserPresence,
  } from "../lib/api";
  import { voiceManager } from "../lib/voice";
  import type { VoiceInputMode } from "../lib/voice";
  import { getChannelProducers } from "../lib/mediasoup";
  import {
    initPTT,
    switchInputMode,
    changePTTKey,
    loadVoiceSettings,
  } from "../lib/ptt";
  import {
    loadAudioSettings,
    saveAudioSettings,
    enumerateAudioDevices,
    onDeviceChange,
    loadPerUserVolumes,
    savePerUserVolume,
    getPerUserVolume,
    type AudioDevice,
  } from "../lib/audioSettings";
  import AuthService, { user, token } from "../lib/auth";
  import {
    isTauri,
    activeServer,
    servers,
    addServer,
    removeServer,
    setActiveServer,
    updateServer,
    type EchoraServer,
  } from "../lib/serverManager";
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";

  import ChannelList from "../lib/components/ChannelList.svelte";
  import OnlineUsers from "../lib/components/OnlineUsers.svelte";
  import MessageList from "../lib/components/MessageList.svelte";
  import MessageInput from "../lib/components/MessageInput.svelte";
  import ScreenShareViewer from "../lib/components/ScreenShareViewer.svelte";
  import ServerSidebar from "../lib/components/ServerSidebar.svelte";
  import AddServerDialog from "../lib/components/AddServerDialog.svelte";
  import LoginForm from "../lib/components/LoginForm.svelte";
  import RegisterForm from "../lib/components/RegisterForm.svelte";
  import AdminPanel from "../lib/components/AdminPanel.svelte";
  import VoicePanel from "../lib/components/VoicePanel.svelte";

  let selectedChannelId = "";
  let showAdminPanel = false;
  let selectedChannelName = "";

  let channels: Channel[] = [];
  let messages: Message[] = [];
  let voiceStates: VoiceState[] = [];
  let speakingUsers: Set<string> = new Set();
  let onlineUsers: UserPresence[] = [];
  let wsManager = new WebSocketManager();
  voiceManager.setWebSocketManager(wsManager);
  let hasMoreMessages = true;
  let loadingMore = false;
  let messageList: MessageList;

  // Local reactive state mirrors
  let currentVoiceChannel: string | null = null;
  let isMuted = false;
  let isDeafened = false;
  let isScreenSharing = false;

  // PTT state
  let voiceInputMode: VoiceInputMode = "voice-activity";
  let pttKey = "Space";
  let pttActive = false;

  // Audio settings state
  let inputDeviceId = "";
  let outputDeviceId = "";
  let inputGain = 1.0;
  let outputVolume = 1.0;
  let vadSensitivity = 50;
  let noiseSuppression = true;
  let inputDevices: AudioDevice[] = [];
  let outputDevices: AudioDevice[] = [];
  let removeDeviceListener: (() => void) | null = null;

  // Screen share viewing state
  let watchingScreenUserId: string | null = null;
  let watchingScreenUsername: string = "";
  let screenVideoElement: HTMLVideoElement;
  let screenAudioElement: HTMLAudioElement | null = null;

  // Camera state
  let isCameraSharing = false;
  let watchingCameraUserId: string | null = null;
  let watchingCameraUsername: string = "";
  let cameraVideoElement: HTMLVideoElement;

  // Message editing state
  let editingMessageId: string | null = null;
  let editMessageContent = "";

  // Reply state
  let replyingTo: Message | null = null;

  // Version info
  let backendVersion = "";
  let serverName = "";
  let tauriVersion = "";

  // User roles map (user_id -> role)
  let userRolesMap: Record<string, string> = {};

  // Mobile sidebar state
  let sidebarOpen = false;

  // Server management state (Tauri only)
  let showAddServerDialog = false;
  let needsServerAuth = false; // Tauri: server added but not authenticated
  let tauriAuthIsLogin = true; // Toggle login/register in inline auth

  // Typing indicator state
  let typingUsers: Map<
    string,
    { username: string; timeout: ReturnType<typeof setTimeout> }
  > = new Map();
  let lastTypingSent = 0;
  const TYPING_DEBOUNCE_MS = 3000;
  const TYPING_DISPLAY_MS = 5000;

  function updateSpeaking(userId: string, speaking: boolean) {
    if (speaking) {
      speakingUsers.add(userId);
    } else {
      speakingUsers.delete(userId);
    }
    speakingUsers = new Set(speakingUsers);
  }

  function syncVoiceState() {
    currentVoiceChannel = voiceManager.currentChannel;
    isMuted = voiceManager.isMutedState;
    isDeafened = voiceManager.isDeafenedState;
    isScreenSharing = voiceManager.isScreenSharingState;
    isCameraSharing = voiceManager.isCameraSharingState;
    voiceInputMode = voiceManager.currentInputMode;
    pttActive = voiceManager.isPTTActive;
  }

  function updateMessageReaction(
    msgId: string,
    emoji: string,
    userId: string,
    add: boolean,
  ) {
    messages = messages.map((m) => {
      if (m.id !== msgId) return m;
      let reactions = m.reactions ? [...m.reactions] : [];
      const existing = reactions.find((r) => r.emoji === emoji);
      if (add) {
        if (existing) {
          existing.count += 1;
          if (userId === $user?.id) existing.reacted = true;
        } else {
          reactions.push({
            emoji,
            count: 1,
            reacted: userId === $user?.id,
          });
        }
      } else if (existing) {
        existing.count -= 1;
        if (userId === $user?.id) existing.reacted = false;
        if (existing.count <= 0) {
          reactions = reactions.filter((r) => r.emoji !== emoji);
        }
      }
      return {
        ...m,
        reactions: reactions.length > 0 ? reactions : undefined,
      };
    });
  }

  function setupWsHandlers() {
    wsManager.onMessage((data) => {
      if (
        data.type === "message" &&
        data.data.channel_id === selectedChannelId
      ) {
        messages = [...messages, data.data];
        // Clear typing indicator for this user
        const authorId = data.data.author_id;
        if (authorId && typingUsers.has(authorId)) {
          clearTimeout(typingUsers.get(authorId)!.timeout);
          typingUsers.delete(authorId);
          typingUsers = new Map(typingUsers);
        }
        requestAnimationFrame(() => messageList?.scrollToBottomIfNear());
      }

      // Channel CRUD events
      if (data.type === "channel_created") {
        channels = [...channels, data.data];
      }
      if (data.type === "channel_updated") {
        channels = channels.map((c) => (c.id === data.data.id ? data.data : c));
        if (selectedChannelId === data.data.id) {
          selectedChannelName = data.data.name;
        }
      }
      if (data.type === "channel_deleted") {
        channels = channels.filter((c) => c.id !== data.data.id);
        if (selectedChannelId === data.data.id) {
          const firstText = channels.find((c) => c.channel_type === "text");
          if (firstText) {
            selectChannel(firstText.id, firstText.name);
          } else {
            selectedChannelId = "";
            selectedChannelName = "";
            messages = [];
          }
        }
      }

      // Online presence events
      if (data.type === "user_online") {
        if (!onlineUsers.find((u) => u.user_id === data.data.user_id)) {
          onlineUsers = [...onlineUsers, data.data];
        }
      }
      if (data.type === "user_offline") {
        onlineUsers = onlineUsers.filter(
          (u) => u.user_id !== data.data.user_id,
        );
      }

      // Message edit/delete events
      if (
        data.type === "message_edited" &&
        data.data.channel_id === selectedChannelId
      ) {
        messages = messages.map((m) => (m.id === data.data.id ? data.data : m));
      }
      if (
        data.type === "message_deleted" &&
        data.data.channel_id === selectedChannelId
      ) {
        messages = messages.filter((m) => m.id !== data.data.id);
      }

      // Reaction events
      if (data.type === "reaction_added" && selectedChannelId) {
        updateMessageReaction(
          data.data.message_id,
          data.data.emoji,
          data.data.user_id,
          true,
        );
      }
      if (data.type === "reaction_removed" && selectedChannelId) {
        updateMessageReaction(
          data.data.message_id,
          data.data.emoji,
          data.data.user_id,
          false,
        );
      }

      // Link preview events
      if (
        data.type === "link_preview_ready" &&
        data.data.channel_id === selectedChannelId
      ) {
        const msgId = data.data.message_id;
        const previews = data.data.link_previews;
        messages = messages.map((m) => {
          if (m.id !== msgId) return m;
          return { ...m, link_previews: previews };
        });
      }

      // Voice state events (global broadcast)
      if (data.type === "voice_user_joined") {
        voiceStates = [
          ...voiceStates.filter((vs) => vs.user_id !== data.data.user_id),
          data.data,
        ];
      }

      // Consume new producers as they're created (replaces fragile timeout)
      if (
        data.type === "new_producer" &&
        data.data.channel_id === currentVoiceChannel &&
        data.data.user_id !== $user?.id &&
        data.data.label !== "screen" &&
        data.data.label !== "camera"
      ) {
        voiceManager.consumeProducer(
          data.data.producer_id,
          data.data.user_id,
          data.data.label,
        );
      }
      if (data.type === "voice_user_left") {
        voiceStates = voiceStates.filter(
          (vs) =>
            !(
              vs.user_id === data.data.user_id &&
              vs.channel_id === data.data.channel_id
            ),
        );
        // Clean up audio elements for the departed user
        voiceManager.removeUserAudio(data.data.user_id);
      }
      if (data.type === "voice_state_updated") {
        voiceStates = voiceStates.map((vs) =>
          vs.user_id === data.data.user_id &&
          vs.channel_id === data.data.channel_id
            ? data.data
            : vs,
        );
      }
      if (data.type === "voice_speaking") {
        updateSpeaking(data.data.user_id, data.data.is_speaking);
      }

      // Screen share events
      if (data.type === "screen_share_updated") {
        voiceStates = voiceStates.map((vs) =>
          vs.user_id === data.data.user_id &&
          vs.channel_id === data.data.channel_id
            ? { ...vs, is_screen_sharing: data.data.is_screen_sharing }
            : vs,
        );

        // If the user we're watching stopped sharing, go back to chat
        if (
          !data.data.is_screen_sharing &&
          watchingScreenUserId === data.data.user_id
        ) {
          stopWatching();
        }
      }

      // Camera events
      if (data.type === "camera_updated") {
        voiceStates = voiceStates.map((vs) =>
          vs.user_id === data.data.user_id &&
          vs.channel_id === data.data.channel_id
            ? { ...vs, is_camera_sharing: data.data.is_camera_sharing }
            : vs,
        );

        if (
          !data.data.is_camera_sharing &&
          watchingCameraUserId === data.data.user_id
        ) {
          stopWatchingCamera();
        }
      }

      // Moderation events
      if (data.type === "user_kicked") {
        if (data.data.user_id === $user?.id) {
          alert("You have been kicked from the server.");
          AuthService.logout();
          goto("/auth");
          return;
        }
      }
      if (data.type === "user_banned") {
        if (data.data.user_id === $user?.id) {
          alert("You have been banned from the server.");
          AuthService.logout();
          goto("/auth");
          return;
        }
      }
      if (data.type === "user_role_changed") {
        if (data.data.user_id === $user?.id && $user) {
          user.set({ ...$user, role: data.data.new_role });
        }
        // Update roles map for online user badges
        userRolesMap = {
          ...userRolesMap,
          [data.data.user_id]: data.data.new_role,
        };
      }

      // Username change events
      if (data.type === "user_renamed") {
        const { user_id, new_username } = data.data;
        // Update online users list
        onlineUsers = onlineUsers.map((u) =>
          u.user_id === user_id ? { ...u, username: new_username } : u,
        );
        // Update voice states
        voiceStates = voiceStates.map((vs) =>
          vs.user_id === user_id ? { ...vs, username: new_username } : vs,
        );
      }

      // Typing indicator events
      if (
        data.type === "typing" &&
        data.data.channel_id === selectedChannelId
      ) {
        const userId = data.data.user_id;
        if (userId === $user?.id) return;

        const existing = typingUsers.get(userId);
        if (existing) clearTimeout(existing.timeout);

        const timeout = setTimeout(() => {
          typingUsers.delete(userId);
          typingUsers = new Map(typingUsers);
        }, TYPING_DISPLAY_MS);

        typingUsers.set(userId, { username: data.data.username, timeout });
        typingUsers = new Map(typingUsers);
      }
    });
  }

  function setupVoiceHandlers() {
    voiceManager.onVoiceStatesChange((states) => {
      if (states.length > 0) {
        const channelId = states[0].channel_id;
        voiceStates = [
          ...voiceStates.filter((vs) => vs.channel_id !== channelId),
          ...states,
        ];
      }
    });

    voiceManager.onSpeakingChange(updateSpeaking);
    voiceManager.onStateChange(syncVoiceState);

    voiceManager.onScreenTrack((track, userId) => {
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

    voiceManager.onCameraTrack((track, userId) => {
      if (track.kind === "video" && cameraVideoElement) {
        cameraVideoElement.srcObject = new MediaStream([track]);
        cameraVideoElement
          .play()
          .catch((e) => console.warn("Camera video autoplay prevented:", e));
      }
    });
  }

  /** Connect to the active server: auth check, WS connect, load data. */
  async function connectToServer() {
    // Reset state for new server connection
    channels = [];
    messages = [];
    voiceStates = [];
    speakingUsers = new Set();
    onlineUsers = [];
    selectedChannelId = "";
    selectedChannelName = "";
    backendVersion = "";
    serverName = "";
    needsServerAuth = false;
    typingUsers.forEach((u) => clearTimeout(u.timeout));
    typingUsers = new Map();

    // In Tauri mode, we need an active server before we can do anything
    if (isTauri && !$activeServer) {
      showAddServerDialog = true;
      return;
    }

    await AuthService.init();

    if (!$user) {
      if (isTauri) {
        // Show inline login/register for this server
        needsServerAuth = true;
        return;
      }
      goto("/auth");
      return;
    }

    try {
      const init = await API.getInit();
      channels = init.channels;
      onlineUsers = init.online_users;
      voiceStates = init.voice_states;
      backendVersion = init.version;
      serverName = init.server_name;
      if (init.users) {
        userRolesMap = Object.fromEntries(
          init.users.map((u) => [u.id, u.role]),
        );
      }

      wsManager = new WebSocketManager();
      voiceManager.setWebSocketManager(wsManager);
      setupWsHandlers();
      wsManager.onReconnect(() => voiceManager.reconcileProducers());

      await wsManager.connect();

      syncVoiceState();

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
    inputDeviceId = settings.inputDeviceId;
    outputDeviceId = settings.outputDeviceId;
    inputGain = settings.inputGain;
    outputVolume = settings.outputVolume;
    vadSensitivity = settings.vadSensitivity;
    noiseSuppression = settings.noiseSuppression;

    // Apply saved settings to voice manager
    voiceManager.setInputGain(inputGain);
    voiceManager.setOutputVolume(outputVolume);
    voiceManager.setSpeakingThreshold(vadSensitivity);

    // Load per-user volumes
    const perUserVols = loadPerUserVolumes();
    for (const [userId, vol] of Object.entries(perUserVols)) {
      voiceManager.setUserVolume(userId, vol);
    }

    // Enumerate devices
    await refreshDeviceList();

    // Listen for device hot-plug
    removeDeviceListener = onDeviceChange(() => refreshDeviceList());
  }

  async function refreshDeviceList() {
    const devices = await enumerateAudioDevices();
    inputDevices = devices.inputs;
    outputDevices = devices.outputs;
  }

  onMount(async () => {
    setupVoiceHandlers();
    // Initialize PTT settings from localStorage
    const settings = await initPTT();
    voiceInputMode = settings.inputMode;
    pttKey = settings.pttKey;

    if (isTauri) {
      import("@tauri-apps/api/app")
        .then((m) => m.getVersion())
        .then((v) => (tauriVersion = v))
        .catch(() => {});
    }

    // Initialize audio settings
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

  // Server management (Tauri only)
  async function handleSelectServer(server: EchoraServer) {
    if ($activeServer?.id === server.id) return;

    // Disconnect from current server
    wsManager.disconnect();
    if (currentVoiceChannel) {
      await voiceManager.leaveVoiceChannel().catch(() => {});
    }

    // Switch to new server
    setActiveServer(server.id);
    await connectToServer();
  }

  function handleAddServer(url: string, name: string) {
    const server = addServer(url, name);
    showAddServerDialog = false;
    // Set active and connect (will show inline auth since no token yet)
    setActiveServer(server.id);
    connectToServer();
  }

  /** Called when inline auth (login/register) succeeds in Tauri mode. */
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

  // Channel CRUD handlers
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

  // Message edit/delete
  function startEditMessage(message: Message) {
    editingMessageId = message.id;
    editMessageContent = message.content;
  }

  function cancelEditMessage() {
    editingMessageId = null;
    editMessageContent = "";
  }

  async function saveEditMessage() {
    if (!editingMessageId || !editMessageContent.trim()) return;
    tryAction(async () => {
      await API.editMessage(
        selectedChannelId,
        editingMessageId!,
        editMessageContent.trim(),
      );
      editingMessageId = null;
      editMessageContent = "";
    }, "edit message");
  }

  async function deleteMessage(messageId: string) {
    if (!confirm("Delete this message?")) return;
    tryAction(
      () => API.deleteMessage(selectedChannelId, messageId),
      "delete message",
    );
  }

  // Reply functions
  function startReply(message: Message) {
    replyingTo = message;
  }

  function cancelReply() {
    replyingTo = null;
  }

  // Reaction functions
  async function toggleReaction(messageId: string, emoji: string) {
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

  // Voice functions
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
    await switchInputMode(mode, pttKey);
    voiceInputMode = mode;
    syncVoiceState();
  }

  async function handleChangePTTKey(key: string) {
    pttKey = key;
    await changePTTKey(key);
  }

  // Audio settings handlers
  function saveCurrentAudioSettings() {
    saveAudioSettings({
      inputDeviceId,
      outputDeviceId,
      inputGain,
      outputVolume,
      vadSensitivity,
      noiseSuppression,
    });
  }

  function handleInputDeviceChange(deviceId: string) {
    inputDeviceId = deviceId;
    voiceManager.setInputDevice(deviceId);
    saveCurrentAudioSettings();
  }

  function handleOutputDeviceChange(deviceId: string) {
    outputDeviceId = deviceId;
    voiceManager.setOutputDevice(deviceId);
    saveCurrentAudioSettings();
  }

  function handleInputGainChange(gain: number) {
    inputGain = gain;
    voiceManager.setInputGain(gain);
    saveCurrentAudioSettings();
  }

  function handleOutputVolumeChange(volume: number) {
    outputVolume = volume;
    voiceManager.setOutputVolume(volume);
    saveCurrentAudioSettings();
  }

  function handleVadSensitivityChange(sensitivity: number) {
    vadSensitivity = sensitivity;
    voiceManager.setSpeakingThreshold(sensitivity);
    saveCurrentAudioSettings();
  }

  function handleNoiseSuppressionToggle(enabled: boolean) {
    noiseSuppression = enabled;
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
    try {
      if (isScreenSharing) {
        await voiceManager.stopScreenShare();
      } else {
        await voiceManager.startScreenShare();
      }
    } catch (error) {
      // User cancelled screen picker -- not an error
      if (error instanceof Error && error.name === "NotAllowedError") return;
      console.error("Failed to toggle screen share:", error);
    }
  }

  async function watchScreen(userId: string, username: string) {
    watchingScreenUserId = userId;
    watchingScreenUsername = username;

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
    watchingScreenUserId = null;
    watchingScreenUsername = "";
    if (screenVideoElement) {
      screenVideoElement.srcObject = null;
    }
    if (screenAudioElement) {
      screenAudioElement.srcObject = null;
      screenAudioElement.remove();
      screenAudioElement = null;
    }
  }

  async function toggleCamera() {
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
    watchingCameraUserId = userId;
    watchingCameraUsername = username;

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
    watchingCameraUserId = null;
    watchingCameraUsername = "";
    if (cameraVideoElement) {
      cameraVideoElement.srcObject = null;
    }
  }

  // Message/channel selection
  async function selectChannel(channelId: string, channelName: string) {
    selectedChannelId = channelId;
    selectedChannelName = channelName;
    hasMoreMessages = true;
    sidebarOpen = false;
    replyingTo = null;
    wsManager.joinChannel(channelId);
    // Clear typing indicators on channel switch
    typingUsers.forEach((u) => clearTimeout(u.timeout));
    typingUsers = new Map();

    try {
      messages = await API.getMessages(channelId, 50);
      hasMoreMessages = messages.length >= 50;
      requestAnimationFrame(() => messageList?.scrollToBottom());
    } catch (error) {
      console.error("Failed to load messages:", error);
      messages = [];
    }
  }

  async function loadOlderMessages() {
    if (
      loadingMore ||
      !hasMoreMessages ||
      !selectedChannelId ||
      messages.length === 0
    )
      return;

    loadingMore = true;
    const oldestTimestamp = messages[0]?.timestamp;

    try {
      const olderMessages = await API.getMessages(
        selectedChannelId,
        50,
        oldestTimestamp,
      );
      hasMoreMessages = olderMessages.length >= 50;

      if (olderMessages.length > 0) {
        messageList?.preserveScroll(() => {
          messages = [...olderMessages, ...messages];
        });
      }
    } catch (error) {
      console.error("Failed to load older messages:", error);
    } finally {
      loadingMore = false;
    }
  }

  function handleSendMessage(text: string, attachmentIds?: string[]) {
    if (selectedChannelId && $user) {
      try {
        wsManager.sendMessage(
          selectedChannelId,
          text,
          replyingTo?.id,
          attachmentIds,
        );
        replyingTo = null;
      } catch (error) {
        console.error("Failed to send message:", error);
      }
    }
  }

  function handleTyping() {
    const now = Date.now();
    if (selectedChannelId && now - lastTypingSent > TYPING_DEBOUNCE_MS) {
      lastTypingSent = now;
      wsManager.sendTyping(selectedChannelId);
    }
  }

  function logout() {
    AuthService.logout();
    goto("/auth");
  }

  function getTypingText(): string {
    const names = [...typingUsers.values()].map((u) => u.username);
    if (names.length === 1) return `${names[0]} is typing...`;
    if (names.length <= 3) return `${names.join(", ")} are typing...`;
    return "Several people are typing...";
  }

  // Username editing state
  let editingUsername = false;
  let newUsername = "";
  let usernameError = "";

  async function handleUpdateUsername() {
    if (!newUsername.trim()) return;
    usernameError = "";
    try {
      const response = await API.updateUsername(newUsername.trim());
      // Update token and user store
      if (isTauri) {
        const server = $activeServer;
        if (server) {
          updateServer(server.id, {
            token: response.token,
            username: response.user.username,
          });
        }
      } else {
        localStorage.setItem("echora_token", response.token);
      }
      token.set(response.token);
      user.set(response.user);
      editingUsername = false;
      newUsername = "";
    } catch (error: unknown) {
      usernameError =
        error instanceof Error ? error.message : "Failed to update username";
    }
  }

  function startEditUsername() {
    newUsername = $user?.username || "";
    usernameError = "";
    editingUsername = true;
  }

  function cancelEditUsername() {
    editingUsername = false;
    newUsername = "";
    usernameError = "";
  }

  $: activeServerName = serverName || $activeServer?.name || "Echora";
  $: userRole = $user?.role || "member";
  $: isMod =
    userRole === "moderator" || userRole === "admin" || userRole === "owner";
  $: onlineUserRoles = userRolesMap;
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
    <!-- Tauri mode: no servers added yet, just show sidebar + dialog -->
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
    <!-- Tauri mode: server selected but not authenticated -->
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
    <!-- Normal authenticated state (web or Tauri with active session) -->
    <div class="sidebar {sidebarOpen ? 'open' : ''}">
      <div class="channels-area">
        <div class="server-header">
          <span class="server-name">{activeServerName}</span>
          <div class="header-actions">
            {#if isMod}
              <button
                class="header-icon-btn"
                on:click={() => (showAdminPanel = true)}
                title="Admin Panel"
              >
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                  ><path
                    d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.53c.04-.32.07-.64.07-.97s-.03-.66-.07-1l2.11-1.63c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65A.49.49 0 0 0 14 2h-4c-.25 0-.46.18-.5.42l-.37 2.65c-.63.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64L4.57 11c-.04.34-.07.67-.07 1s.03.65.07.97l-2.11 1.66c-.19.15-.25.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1.01c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.25.42.5.42h4c.25 0 .46-.18.5-.42l.37-2.65c.63-.26 1.17-.59 1.69-.98l2.49 1.01c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.66z"
                  /></svg
                >
              </button>
            {/if}
            <button
              class="header-icon-btn logout"
              on:click={logout}
              title="Logout"
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="currentColor"
                ><path
                  d="M5 5h7V3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h7v-2H5V5zm16 7l-4-4v3H9v2h8v3l4-4z"
                /></svg
              >
            </button>
          </div>
        </div>

        <div class="channels-list">
          <ChannelList
            {channels}
            {selectedChannelId}
            {currentVoiceChannel}
            {voiceStates}
            {speakingUsers}
            currentUserId={$user?.id || ""}
            userRole={$user?.role || "member"}
            onSelectChannel={selectChannel}
            onCreateChannel={handleCreateChannel}
            onUpdateChannel={handleUpdateChannel}
            onDeleteChannel={handleDeleteChannel}
            onJoinVoice={joinVoiceChannel}
            onWatchScreen={watchScreen}
            onWatchCamera={watchCamera}
            onUserVolumeChange={handleUserVolumeChange}
            getUserVolume={handleGetUserVolume}
          />

          <OnlineUsers {onlineUsers} userRoles={onlineUserRoles} />
        </div>

        <VoicePanel
          {currentVoiceChannel}
          {isMuted}
          {isDeafened}
          {isScreenSharing}
          {isCameraSharing}
          {voiceInputMode}
          {pttKey}
          {pttActive}
          {inputDeviceId}
          {outputDeviceId}
          {inputGain}
          {outputVolume}
          {vadSensitivity}
          {noiseSuppression}
          {inputDevices}
          {outputDevices}
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

        <div class="user-bar">
          {#if editingUsername}
            <div class="username-edit">
              <input
                type="text"
                class="username-edit-input"
                bind:value={newUsername}
                on:keydown={(e) => {
                  if (e.key === "Enter") handleUpdateUsername();
                  else if (e.key === "Escape") cancelEditUsername();
                }}
                minlength={2}
                maxlength={32}
              />
              <button
                class="username-edit-btn save"
                on:click={handleUpdateUsername}
                title="Save"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                  ><path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z" /></svg
                >
              </button>
              <button
                class="username-edit-btn cancel"
                on:click={cancelEditUsername}
                title="Cancel"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                  ><path
                    d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                  /></svg
                >
              </button>
            </div>
            {#if usernameError}
              <div class="username-error">{usernameError}</div>
            {/if}
          {:else}
            <div class="user-info">
              <span class="user-bar-username">{$user?.username || ""}</span>
              <button
                class="username-edit-trigger"
                on:click={startEditUsername}
                title="Edit username"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
                  ><path
                    d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
                  /></svg
                >
              </button>
            </div>
          {/if}
        </div>

        <div class="version-bar">
          {#if tauriVersion}
            <span class="version-info">app v{tauriVersion}</span>
          {/if}
          <span class="version-info">frontend v{FRONTEND_VERSION}</span>
          <span class="version-info">backend v{backendVersion || "..."}</span>
        </div>
      </div>
    </div>

    <div class="main-content">
      <div class="chat-header">
        <button
          class="hamburger-btn"
          on:click={() => (sidebarOpen = !sidebarOpen)}>|||</button
        >
        <div class="channel-name">
          {selectedChannelName || "Select a channel"}
        </div>
      </div>

      {#if watchingScreenUserId}
        <ScreenShareViewer
          username={watchingScreenUsername}
          onClose={stopWatching}
          bind:videoElement={screenVideoElement}
        />
      {:else if watchingCameraUserId}
        <ScreenShareViewer
          username={watchingCameraUsername}
          type="camera"
          onClose={stopWatchingCamera}
          bind:videoElement={cameraVideoElement}
        />
      {:else}
        <MessageList
          bind:this={messageList}
          {messages}
          currentUserId={$user?.id || ""}
          userRole={$user?.role || "member"}
          {loadingMore}
          {editingMessageId}
          bind:editMessageContent
          onScrollTop={loadOlderMessages}
          onStartEdit={startEditMessage}
          onSaveEdit={saveEditMessage}
          onCancelEdit={cancelEditMessage}
          onDeleteMessage={deleteMessage}
          onReply={startReply}
          onToggleReaction={toggleReaction}
        />

        {#if typingUsers.size > 0}
          <div class="typing-indicator">
            <span class="typing-text">{getTypingText()}</span>
          </div>
        {/if}

        <MessageInput
          channelName={selectedChannelName}
          disabled={!selectedChannelId}
          {replyingTo}
          onSend={handleSendMessage}
          onTyping={handleTyping}
          onCancelReply={cancelReply}
        />
      {/if}
    </div>
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
