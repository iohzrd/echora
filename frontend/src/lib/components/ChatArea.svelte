<script lang="ts">
  import { type Message } from "../api";
  import { user } from "../auth";
  import { voiceStore } from "../stores/voiceStore";
  import { serverState } from "../stores/serverState";
  import { chatState } from "../stores/chatState";
  import MessageList from "./MessageList.svelte";
  import MessageInput from "./MessageInput.svelte";
  import ScreenShareViewer from "./ScreenShareViewer.svelte";

  export let messageList: MessageList | undefined = undefined;
  export let screenVideoElement: HTMLVideoElement = undefined!;
  export let cameraVideoElement: HTMLVideoElement = undefined!;

  export let onToggleSidebar: () => void = () => {};
  export let onStopWatching: () => void = () => {};
  export let onStopWatchingCamera: () => void = () => {};
  export let onScrollTop: () => void = () => {};
  export let onStartEdit: (message: Message) => void = () => {};
  export let onSaveEdit: () => void = () => {};
  export let onCancelEdit: () => void = () => {};
  export let onDeleteMessage: (id: string) => void = () => {};
  export let onReply: (message: Message) => void = () => {};
  export let onToggleReaction: (
    messageId: string,
    emoji: string,
  ) => void = () => {};
  export let onUserClick: (userId: string) => void = () => {};
  export let onSend: (
    text: string,
    attachmentIds?: string[],
  ) => void = () => {};
  export let onTyping: () => void = () => {};
  export let onCancelReply: () => void = () => {};

  // Local mirror for two-way bind with MessageList
  let editMessageContent = "";
  $: editMessageContent = $chatState.editMessageContent;
  $: if (editMessageContent !== $chatState.editMessageContent) {
    chatState.update((s) => ({ ...s, editMessageContent }));
  }

  function getTypingText(): string {
    const names = [...$chatState.typingUsers.values()].map((u) => u.username);
    if (names.length === 1) return `${names[0]} is typing...`;
    if (names.length <= 3) return `${names.join(", ")} are typing...`;
    return "Several people are typing...";
  }
</script>

<div class="main-content">
  <div class="chat-header">
    <button class="hamburger-btn" on:click={onToggleSidebar}>|||</button>
    <div class="channel-name">
      {$chatState.selectedChannelName || "Select a channel"}
    </div>
  </div>

  {#if $voiceStore.watchingScreenUserId}
    <ScreenShareViewer
      username={$voiceStore.watchingScreenUsername}
      onClose={onStopWatching}
      bind:videoElement={screenVideoElement}
    />
  {:else if $voiceStore.watchingCameraUserId}
    <ScreenShareViewer
      username={$voiceStore.watchingCameraUsername}
      type="camera"
      onClose={onStopWatchingCamera}
      bind:videoElement={cameraVideoElement}
    />
  {:else}
    <MessageList
      bind:this={messageList}
      messages={$chatState.messages}
      currentUserId={$user?.id || ""}
      userRole={$user?.role || "member"}
      loadingMore={$chatState.loadingMore}
      editingMessageId={$chatState.editingMessageId}
      bind:editMessageContent
      {onScrollTop}
      {onStartEdit}
      {onSaveEdit}
      {onCancelEdit}
      {onDeleteMessage}
      {onReply}
      {onToggleReaction}
      customEmojis={$serverState.customEmojis}
      userAvatars={$serverState.userAvatars}
      {onUserClick}
    />

    {#if $chatState.typingUsers.size > 0}
      <div class="typing-indicator">
        <span class="typing-text">{getTypingText()}</span>
      </div>
    {/if}

    {#if $chatState.rateLimitWarning}
      <div class="rate-limit-warning">
        Slow down! You are sending messages too fast.
      </div>
    {/if}

    <MessageInput
      channelName={$chatState.selectedChannelName}
      disabled={!$chatState.selectedChannelId}
      replyingTo={$chatState.replyingTo}
      {onSend}
      {onTyping}
      {onCancelReply}
    />
  {/if}
</div>
