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
  import { getChannelProducers } from "../lib/mediasoup";
  import AuthService, { user } from "../lib/auth";
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";

  import ChannelList from "../lib/components/ChannelList.svelte";
  import OnlineUsers from "../lib/components/OnlineUsers.svelte";
  import MessageList from "../lib/components/MessageList.svelte";
  import MessageInput from "../lib/components/MessageInput.svelte";
  import ScreenShareViewer from "../lib/components/ScreenShareViewer.svelte";

  let selectedChannelId = "";
  let selectedChannelName = "";

  let channels: Channel[] = [];
  let messages: Message[] = [];
  let voiceStates: VoiceState[] = [];
  let speakingUsers: Set<string> = new Set();
  let onlineUsers: UserPresence[] = [];
  let wsManager = new WebSocketManager();
  let hasMoreMessages = true;
  let loadingMore = false;
  let messageList: MessageList;

  // Local reactive state mirrors
  let currentVoiceChannel: string | null = null;
  let isMuted = false;
  let isDeafened = false;
  let isScreenSharing = false;

  // Screen share viewing state
  let watchingScreenUserId: string | null = null;
  let watchingScreenUsername: string = "";
  let screenVideoElement: HTMLVideoElement;
  let screenAudioElement: HTMLAudioElement | null = null;

  // Message editing state
  let editingMessageId: string | null = null;
  let editMessageContent = "";

  // Reply state
  let replyingTo: Message | null = null;

  // Version info
  let backendVersion = "";

  // Mobile sidebar state
  let sidebarOpen = false;

  // Typing indicator state
  let typingUsers: Map<
    string,
    { username: string; timeout: ReturnType<typeof setTimeout> }
  > = new Map();
  let lastTypingSent = 0;
  const TYPING_DEBOUNCE_MS = 3000;
  const TYPING_DISPLAY_MS = 5000;

  onMount(async () => {
    await AuthService.init();

    if (!$user) {
      goto("/auth");
      return;
    }

    try {
      channels = await API.getChannels();

      await wsManager.connect();
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
          channels = channels.map((c) =>
            c.id === data.data.id ? data.data : c,
          );
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
          messages = messages.map((m) =>
            m.id === data.data.id ? data.data : m,
          );
        }
        if (
          data.type === "message_deleted" &&
          data.data.channel_id === selectedChannelId
        ) {
          messages = messages.filter((m) => m.id !== data.data.id);
        }

        // Reaction events
        if (
          data.type === "reaction_added" &&
          selectedChannelId
        ) {
          const msgId = data.data.message_id;
          messages = messages.map((m) => {
            if (m.id !== msgId) return m;
            const reactions = m.reactions ? [...m.reactions] : [];
            const existing = reactions.find((r) => r.emoji === data.data.emoji);
            if (existing) {
              existing.count += 1;
              if (data.data.user_id === $user?.id) existing.reacted = true;
            } else {
              reactions.push({
                emoji: data.data.emoji,
                count: 1,
                reacted: data.data.user_id === $user?.id,
              });
            }
            return { ...m, reactions };
          });
        }
        if (
          data.type === "reaction_removed" &&
          selectedChannelId
        ) {
          const msgId = data.data.message_id;
          messages = messages.map((m) => {
            if (m.id !== msgId) return m;
            let reactions = m.reactions ? [...m.reactions] : [];
            const existing = reactions.find((r) => r.emoji === data.data.emoji);
            if (existing) {
              existing.count -= 1;
              if (data.data.user_id === $user?.id) existing.reacted = false;
              if (existing.count <= 0) {
                reactions = reactions.filter((r) => r.emoji !== data.data.emoji);
              }
            }
            return { ...m, reactions: reactions.length > 0 ? reactions : undefined };
          });
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
          data.data.label !== "screen"
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
          if (data.data.is_speaking) {
            speakingUsers.add(data.data.user_id);
          } else {
            speakingUsers.delete(data.data.user_id);
          }
          speakingUsers = new Set(speakingUsers);
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

      // Setup voice manager
      voiceManager.onVoiceStatesChange((states) => {
        if (states.length > 0) {
          const channelId = states[0].channel_id;
          voiceStates = [
            ...voiceStates.filter((vs) => vs.channel_id !== channelId),
            ...states,
          ];
        }
      });

      voiceManager.onSpeakingChange((userId, isSpeaking) => {
        if (isSpeaking) {
          speakingUsers.add(userId);
        } else {
          speakingUsers.delete(userId);
        }
        speakingUsers = new Set(speakingUsers);
      });

      voiceManager.onStateChange(() => {
        currentVoiceChannel = voiceManager.currentChannel;
        isMuted = voiceManager.isMutedState;
        isDeafened = voiceManager.isDeafenedState;
        isScreenSharing = voiceManager.isScreenSharingState;
      });

      voiceManager.onScreenTrack((track, userId) => {
        if (track.kind === "video") {
          if (screenVideoElement) {
            screenVideoElement.srcObject = new MediaStream([track]);
            screenVideoElement
              .play()
              .catch((e) =>
                console.warn("Screen video autoplay prevented:", e),
              );
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

      currentVoiceChannel = voiceManager.currentChannel;
      isMuted = voiceManager.isMutedState;
      isDeafened = voiceManager.isDeafenedState;
      isScreenSharing = voiceManager.isScreenSharingState;

      // Fetch online users and voice states
      onlineUsers = await API.getOnlineUsers();
      voiceStates = await API.getAllVoiceStates();
      backendVersion = await API.getBackendVersion();

      // Select first text channel after WebSocket is connected
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
  });

  onDestroy(() => {
    wsManager.disconnect();
  });

  // Channel CRUD handlers
  async function handleCreateChannel(name: string, type: "text" | "voice") {
    try {
      await API.createChannel(name, type);
    } catch (error) {
      console.error("Failed to create channel:", error);
    }
  }

  async function handleUpdateChannel(channelId: string, name: string) {
    try {
      await API.updateChannel(channelId, name);
    } catch (error) {
      console.error("Failed to update channel:", error);
    }
  }

  async function handleDeleteChannel(channelId: string) {
    if (!confirm("Delete this channel and all its messages?")) return;
    try {
      await API.deleteChannel(channelId);
    } catch (error) {
      console.error("Failed to delete channel:", error);
    }
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
    try {
      await API.editMessage(
        selectedChannelId,
        editingMessageId,
        editMessageContent.trim(),
      );
      editingMessageId = null;
      editMessageContent = "";
    } catch (error) {
      console.error("Failed to edit message:", error);
    }
  }

  async function deleteMessage(messageId: string) {
    if (!confirm("Delete this message?")) return;
    try {
      await API.deleteMessage(selectedChannelId, messageId);
    } catch (error) {
      console.error("Failed to delete message:", error);
    }
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

  // Voice functions
  async function joinVoiceChannel(channelId: string) {
    try {
      await voiceManager.joinVoiceChannel(channelId);
    } catch (error) {
      console.error("Failed to join voice channel:", error);
    }
  }

  async function leaveVoiceChannel() {
    try {
      await voiceManager.leaveVoiceChannel();
    } catch (error) {
      console.error("Failed to leave voice channel:", error);
    }
  }

  async function toggleMute() {
    try {
      await voiceManager.toggleMute();
    } catch (error) {
      console.error("Failed to toggle mute:", error);
    }
  }

  async function toggleDeafen() {
    try {
      await voiceManager.toggleDeafen();
    } catch (error) {
      console.error("Failed to toggle deafen:", error);
    }
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

  function handleSendMessage(text: string) {
    if (selectedChannelId && $user) {
      try {
        wsManager.sendMessage(selectedChannelId, text, replyingTo?.id);
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
</script>

<div class="discord-layout">
  {#if sidebarOpen}
    <div
      class="sidebar-overlay"
      on:click={() => (sidebarOpen = false)}
      role="presentation"
    ></div>
  {/if}
  <div class="sidebar {sidebarOpen ? 'open' : ''}">
    <div class="server-list">
      <div class="server-icon home" title="Echora">E</div>
    </div>

    <div class="channels-area">
      <div class="server-header">
        <span>Echora</span>
        <button class="logout-btn" on:click={logout} title="Logout">
          Logout
        </button>
      </div>

      <div class="channels-list">
        <ChannelList
          {channels}
          {selectedChannelId}
          {currentVoiceChannel}
          {voiceStates}
          {speakingUsers}
          {isMuted}
          {isDeafened}
          {isScreenSharing}
          currentUserId={$user?.id || ""}
          onSelectChannel={selectChannel}
          onCreateChannel={handleCreateChannel}
          onUpdateChannel={handleUpdateChannel}
          onDeleteChannel={handleDeleteChannel}
          onJoinVoice={joinVoiceChannel}
          onLeaveVoice={leaveVoiceChannel}
          onToggleMute={toggleMute}
          onToggleDeafen={toggleDeafen}
          onToggleScreenShare={toggleScreenShare}
          onWatchScreen={watchScreen}
        />

        <OnlineUsers {onlineUsers} />
      </div>
      <div class="version-bar">
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
    {:else}
      <MessageList
        bind:this={messageList}
        {messages}
        currentUserId={$user?.id || ""}
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
</div>
