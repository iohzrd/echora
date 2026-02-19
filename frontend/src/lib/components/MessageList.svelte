<script lang="ts">
  import type { Message } from "../api";
  import { renderMarkdown } from "../markdown";

  export let messages: Message[] = [];
  export let currentUserId: string = "";
  export let loadingMore: boolean = false;
  export let editingMessageId: string | null = null;
  export let editMessageContent: string = "";
  export let onScrollTop: () => void = () => {};
  export let onStartEdit: (message: Message) => void = () => {};
  export let onSaveEdit: () => void = () => {};
  export let onCancelEdit: () => void = () => {};
  export let onDeleteMessage: (messageId: string) => void = () => {};
  export let onReply: (message: Message) => void = () => {};
  export let onToggleReaction: (messageId: string, emoji: string) => void = () => {};

  let messagesArea: HTMLDivElement;
  let emojiPickerMessageId: string | null = null;

  const COMMON_EMOJI = [
    "\u{1F44D}", "\u{1F44E}", "\u{2764}\u{FE0F}", "\u{1F602}", "\u{1F622}", "\u{1F621}",
    "\u{1F389}", "\u{1F525}", "\u{1F44F}", "\u{1F914}",
    "\u{1F440}", "\u{1F680}", "\u{2705}", "\u{274C}", "\u{1F4AF}", "\u{1F60D}",
    "\u{1F631}", "\u{1F64F}", "\u{1F499}", "\u{1F49A}",
  ];

  export function scrollToBottom() {
    if (messagesArea) {
      messagesArea.scrollTop = messagesArea.scrollHeight;
    }
  }

  export function preserveScroll(callback: () => void) {
    if (messagesArea) {
      const prevScrollHeight = messagesArea.scrollHeight;
      callback();
      requestAnimationFrame(() => {
        if (messagesArea) {
          messagesArea.scrollTop = messagesArea.scrollHeight - prevScrollHeight;
        }
      });
    }
  }

  export function scrollToBottomIfNear() {
    if (messagesArea) {
      const isNearBottom =
        messagesArea.scrollHeight -
          messagesArea.scrollTop -
          messagesArea.clientHeight <
        100;
      if (isNearBottom) {
        requestAnimationFrame(() => {
          if (messagesArea) {
            messagesArea.scrollTop = messagesArea.scrollHeight;
          }
        });
      }
    }
  }

  function handleScroll() {
    if (messagesArea && messagesArea.scrollTop === 0) {
      onScrollTop();
    }
  }

  function handleEditKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSaveEdit();
    } else if (event.key === "Escape") {
      onCancelEdit();
    }
  }

  function formatTimestamp(timestamp: string): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  function getAvatar(author: string): string {
    return author.charAt(0).toUpperCase();
  }

  function truncateContent(content: string, maxLen = 100): string {
    if (content.length <= maxLen) return content;
    return content.substring(0, maxLen) + "...";
  }

  function toggleEmojiPicker(messageId: string) {
    if (emojiPickerMessageId === messageId) {
      emojiPickerMessageId = null;
    } else {
      emojiPickerMessageId = messageId;
    }
  }

  function selectEmoji(messageId: string, emoji: string) {
    onToggleReaction(messageId, emoji);
    emojiPickerMessageId = null;
  }
</script>

<div
  class="messages-area"
  bind:this={messagesArea}
  on:scroll={handleScroll}
>
  {#if loadingMore}
    <div class="loading-more">Loading older messages...</div>
  {/if}
  {#each messages as message}
    <div class="message">
      <div class="message-avatar">
        {getAvatar(message.author)}
      </div>
      <div class="message-content">
        <div class="message-header">
          <span class="message-author">{message.author}</span>
          <span class="message-timestamp"
            >{formatTimestamp(message.timestamp)}</span
          >
          {#if message.edited_at}
            <span class="message-edited">(edited)</span>
          {/if}
        </div>
        {#if message.reply_to}
          <div class="reply-preview">
            <span class="reply-author">{message.reply_to.author}</span>
            <span class="reply-content">{truncateContent(message.reply_to.content)}</span>
          </div>
        {:else if message.reply_to_id}
          <div class="reply-preview reply-deleted">
            <span class="reply-content">(original message deleted)</span>
          </div>
        {/if}
        {#if editingMessageId === message.id}
          <div class="edit-message-form">
            <textarea
              class="edit-message-input"
              bind:value={editMessageContent}
              on:keydown={handleEditKeydown}
            ></textarea>
            <div class="edit-message-actions">
              <button
                class="edit-action-btn cancel"
                on:click={onCancelEdit}>Cancel</button
              >
              <button
                class="edit-action-btn save"
                on:click={onSaveEdit}>Save</button
              >
            </div>
          </div>
        {:else}
          <div class="message-text">{@html renderMarkdown(message.content)}</div>
        {/if}
        {#if message.link_previews && message.link_previews.length > 0}
          {#each message.link_previews as preview}
            <div class="link-preview-card">
              {#if preview.image_url}
                <img class="link-preview-image" src={preview.image_url} alt={preview.title || ''} loading="lazy" />
              {/if}
              <div class="link-preview-text">
                {#if preview.site_name}
                  <div class="link-preview-site">{preview.site_name}</div>
                {/if}
                {#if preview.title}
                  <a class="link-preview-title" href={preview.url} target="_blank" rel="noopener noreferrer">{preview.title}</a>
                {/if}
                {#if preview.description}
                  <div class="link-preview-description">{preview.description}</div>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
        {#if message.reactions && message.reactions.length > 0}
          <div class="reactions-row">
            {#each message.reactions as reaction}
              <button
                class="reaction-btn {reaction.reacted ? 'reacted' : ''}"
                on:click={() => onToggleReaction(message.id, reaction.emoji)}
                title={reaction.emoji}
              >
                {reaction.emoji} {reaction.count}
              </button>
            {/each}
            <button
              class="reaction-btn add-reaction"
              on:click={() => toggleEmojiPicker(message.id)}
              title="Add reaction"
            >+</button>
            {#if emojiPickerMessageId === message.id}
              <div class="emoji-picker">
                {#each COMMON_EMOJI as emoji}
                  <button
                    class="emoji-picker-btn"
                    on:click={() => selectEmoji(message.id, emoji)}
                  >{emoji}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
      {#if editingMessageId !== message.id}
        <div class="message-actions">
          <button
            class="msg-action-btn"
            on:click={() => onReply(message)}
            title="Reply">R</button
          >
          <button
            class="msg-action-btn"
            on:click={() => toggleEmojiPicker(message.id)}
            title="React">+</button
          >
          {#if message.author_id === currentUserId}
            <button
              class="msg-action-btn"
              on:click={() => onStartEdit(message)}
              title="Edit">E</button
            >
            <button
              class="msg-action-btn delete"
              on:click={() => onDeleteMessage(message.id)}
              title="Delete">X</button
            >
          {/if}
        </div>
        {#if emojiPickerMessageId === message.id && !(message.reactions && message.reactions.length > 0)}
          <div class="emoji-picker emoji-picker-floating">
            {#each COMMON_EMOJI as emoji}
              <button
                class="emoji-picker-btn"
                on:click={() => selectEmoji(message.id, emoji)}
              >{emoji}</button>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  {/each}
</div>
