<script lang="ts">
  import { type Message, type CustomEmoji } from "../api";
  import MessageList from "./MessageList.svelte";
  import MessageInput from "./MessageInput.svelte";
  import ScreenShareViewer from "./ScreenShareViewer.svelte";

  export let selectedChannelName: string = "";
  export let watchingScreenUserId: string | null = null;
  export let watchingScreenUsername: string = "";
  export let watchingCameraUserId: string | null = null;
  export let watchingCameraUsername: string = "";
  export let messages: Message[] = [];
  export let currentUserId: string = "";
  export let userRole: string = "member";
  export let loadingMore: boolean = false;
  export let editingMessageId: string | null = null;
  export let editMessageContent: string = "";
  export let typingUsers: Map<string, { username: string; timeout: ReturnType<typeof setTimeout> }> = new Map();
  export let rateLimitWarning: boolean = false;
  export let selectedChannelId: string = "";
  export let replyingTo: Message | null = null;
  export let customEmojis: CustomEmoji[] = [];
  export let userAvatars: Record<string, string | undefined> = {};

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
  export let onToggleReaction: (messageId: string, emoji: string) => void = () => {};
  export let onUserClick: (userId: string) => void = () => {};
  export let onSend: (text: string, attachmentIds?: string[]) => void = () => {};
  export let onTyping: () => void = () => {};
  export let onCancelReply: () => void = () => {};
  export let getTypingText: () => string = () => "";
</script>

<div class="main-content">
  <div class="chat-header">
    <button
      class="hamburger-btn"
      on:click={onToggleSidebar}>|||</button
    >
    <div class="channel-name">
      {selectedChannelName || "Select a channel"}
    </div>
  </div>

  {#if watchingScreenUserId}
    <ScreenShareViewer
      username={watchingScreenUsername}
      onClose={onStopWatching}
      bind:videoElement={screenVideoElement}
    />
  {:else if watchingCameraUserId}
    <ScreenShareViewer
      username={watchingCameraUsername}
      type="camera"
      onClose={onStopWatchingCamera}
      bind:videoElement={cameraVideoElement}
    />
  {:else}
    <MessageList
      bind:this={messageList}
      {messages}
      {currentUserId}
      {userRole}
      {loadingMore}
      {editingMessageId}
      bind:editMessageContent
      onScrollTop={onScrollTop}
      onStartEdit={onStartEdit}
      onSaveEdit={onSaveEdit}
      onCancelEdit={onCancelEdit}
      onDeleteMessage={onDeleteMessage}
      onReply={onReply}
      onToggleReaction={onToggleReaction}
      {customEmojis}
      {userAvatars}
      onUserClick={onUserClick}
    />

    {#if typingUsers.size > 0}
      <div class="typing-indicator">
        <span class="typing-text">{getTypingText()}</span>
      </div>
    {/if}

    {#if rateLimitWarning}
      <div class="rate-limit-warning">
        Slow down! You are sending messages too fast.
      </div>
    {/if}
    <MessageInput
      channelName={selectedChannelName}
      disabled={!selectedChannelId}
      {replyingTo}
      onSend={onSend}
      onTyping={onTyping}
      onCancelReply={onCancelReply}
    />
  {/if}
</div>
