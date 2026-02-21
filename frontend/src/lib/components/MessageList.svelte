<script lang="ts">
  import { API, type Message } from "../api";
  import { renderMarkdown } from "../markdown";
  import { formatTimestamp, getInitial, truncateContent } from "../utils";
  import { getApiBase } from "../config";
  import { isTauri } from "../serverManager";
  import EmojiPicker from "./EmojiPicker.svelte";

  function resolveUrl(url: string): string {
    if (isTauri && url.startsWith("/")) {
      const apiBase = getApiBase();
      // Strip /api suffix to get the server origin
      const origin = apiBase.replace(/\/api$/, "");
      return origin + url;
    }
    return url;
  }

  export let messages: Message[] = [];
  export let currentUserId: string = "";
  export let userRole: string = "member";
  export let loadingMore: boolean = false;
  export let editingMessageId: string | null = null;
  export let editMessageContent: string = "";
  export let onScrollTop: () => void = () => {};
  export let onStartEdit: (message: Message) => void = () => {};
  export let onSaveEdit: () => void = () => {};
  export let onCancelEdit: () => void = () => {};
  export let onDeleteMessage: (messageId: string) => void = () => {};
  export let onReply: (message: Message) => void = () => {};
  export let onToggleReaction: (
    messageId: string,
    emoji: string,
  ) => void = () => {};

  let messagesArea: HTMLDivElement;
  let emojiPickerMessageId: string | null = null;

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

  export function isNearBottom(): boolean {
    if (!messagesArea) return true;
    return (
      messagesArea.scrollHeight -
        messagesArea.scrollTop -
        messagesArea.clientHeight <
      100
    );
  }

  export function scrollToBottomIfNear() {
    if (messagesArea && isNearBottom()) {
      requestAnimationFrame(() => {
        if (messagesArea) {
          messagesArea.scrollTop = messagesArea.scrollHeight;
        }
      });
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

  function isImageType(contentType: string): boolean {
    return contentType.startsWith("image/");
  }

  function isVideoType(contentType: string): boolean {
    return contentType.startsWith("video/");
  }

  function isAudioType(contentType: string): boolean {
    return contentType.startsWith("audio/");
  }

  function getAttachmentUrl(attachmentId: string, filename: string): string {
    return resolveUrl(API.getAttachmentUrl(attachmentId, filename));
  }

  function formatFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  const COLLAPSE_HEIGHT = 300;
  let expandedMessages = new Set<string>();
  let overflowingMessages = new Set<string>();

  function toggleExpand(messageId: string) {
    if (expandedMessages.has(messageId)) {
      expandedMessages.delete(messageId);
    } else {
      expandedMessages.add(messageId);
    }
    expandedMessages = expandedMessages;
  }

  function checkOverflow(node: HTMLElement, messageId: string) {
    function update() {
      const overflows = node.scrollHeight > COLLAPSE_HEIGHT;
      if (overflows && !overflowingMessages.has(messageId)) {
        overflowingMessages.add(messageId);
        overflowingMessages = overflowingMessages;
      } else if (!overflows && overflowingMessages.has(messageId)) {
        overflowingMessages.delete(messageId);
        overflowingMessages = overflowingMessages;
      }
    }
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return {
      destroy() {
        observer.disconnect();
      },
    };
  }

  let lightboxSrc: string | null = null;
  let lightboxAlt: string = "";

  function openLightbox(src: string, alt: string) {
    lightboxSrc = src;
    lightboxAlt = alt;
  }

  function closeLightbox() {
    lightboxSrc = null;
    lightboxAlt = "";
  }

  function handleLightboxKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      closeLightbox();
    }
  }
</script>

<svelte:window on:keydown={handleLightboxKeydown} />

<div class="messages-area" bind:this={messagesArea} on:scroll={handleScroll}>
  {#if loadingMore}
    <div class="loading-more">Loading older messages...</div>
  {/if}
  {#each messages as message}
    <div class="message">
      <div class="message-avatar">
        {getInitial(message.author)}
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
            <span class="reply-content"
              >{truncateContent(message.reply_to.content)}</span
            >
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
              <button class="edit-action-btn cancel" on:click={onCancelEdit}
                >Cancel</button
              >
              <button class="edit-action-btn save" on:click={onSaveEdit}
                >Save</button
              >
            </div>
          </div>
        {:else}
          {#if message.content}
            <div
              class="message-text"
              class:collapsed={overflowingMessages.has(message.id) && !expandedMessages.has(message.id)}
              use:checkOverflow={message.id}
            >
              {@html renderMarkdown(message.content)}
            </div>
            {#if overflowingMessages.has(message.id)}
              <button
                class="message-expand-btn"
                on:click={() => toggleExpand(message.id)}
              >
                {expandedMessages.has(message.id) ? "Show less" : "Show more"}
              </button>
            {/if}
          {/if}
        {/if}
        {#if message.attachments && message.attachments.length > 0}
          <div class="attachments">
            {#each message.attachments as attachment}
              {#if isImageType(attachment.content_type)}
                <button
                  class="attachment-image-link"
                  on:click={() =>
                    openLightbox(
                      getAttachmentUrl(attachment.id, attachment.filename),
                      attachment.filename,
                    )}
                >
                  <img
                    class="attachment-image"
                    src={getAttachmentUrl(attachment.id, attachment.filename)}
                    alt={attachment.filename}
                    loading="lazy"
                  />
                </button>
              {:else if isVideoType(attachment.content_type)}
                <video
                  class="attachment-video"
                  controls
                  preload="metadata"
                  src={getAttachmentUrl(attachment.id, attachment.filename)}
                >
                  <track kind="captions" />
                </video>
              {:else if isAudioType(attachment.content_type)}
                <div class="attachment-audio">
                  <span class="attachment-audio-name">{attachment.filename}</span>
                  <audio
                    controls
                    preload="metadata"
                    src={getAttachmentUrl(attachment.id, attachment.filename)}
                  ></audio>
                </div>
              {:else}
                <a
                  class="attachment-file"
                  href={getAttachmentUrl(attachment.id, attachment.filename)}
                  target="_blank"
                  rel="noopener noreferrer"
                  download={attachment.filename}
                >
                  <span class="attachment-file-icon">F</span>
                  <div class="attachment-file-info">
                    <span class="attachment-file-name">{attachment.filename}</span>
                    <span class="attachment-file-size">{formatFileSize(attachment.size)}</span>
                  </div>
                </a>
              {/if}
            {/each}
          </div>
        {/if}
        {#if message.link_previews && message.link_previews.length > 0}
          {#each message.link_previews as preview}
            <div class="link-preview-card">
              {#if preview.image_url}
                <button
                  class="link-preview-image-btn"
                  on:click={() =>
                    openLightbox(
                      resolveUrl(preview.image_url || ""),
                      preview.title || "",
                    )}
                >
                  <img
                    class="link-preview-image"
                    src={resolveUrl(preview.image_url)}
                    alt={preview.title || ""}
                    loading="lazy"
                  />
                </button>
              {/if}
              <div class="link-preview-text">
                {#if preview.site_name}
                  <div class="link-preview-site">{preview.site_name}</div>
                {/if}
                {#if preview.title}
                  <a
                    class="link-preview-title"
                    href={preview.url}
                    target="_blank"
                    rel="noopener noreferrer">{preview.title}</a
                  >
                {/if}
                {#if preview.description}
                  <div class="link-preview-description">
                    {preview.description}
                  </div>
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
                {reaction.emoji}
                {reaction.count}
              </button>
            {/each}
            <button
              class="reaction-btn add-reaction"
              on:click={() => toggleEmojiPicker(message.id)}
              title="Add reaction">+</button
            >
            {#if emojiPickerMessageId === message.id}
              <EmojiPicker
                onSelect={(emoji) => selectEmoji(message.id, emoji)}
              />
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
          {/if}
          {#if message.author_id === currentUserId || userRole === "moderator" || userRole === "admin" || userRole === "owner"}
            <button
              class="msg-action-btn delete"
              on:click={() => onDeleteMessage(message.id)}
              title="Delete">X</button
            >
          {/if}
        </div>
        {#if emojiPickerMessageId === message.id && !(message.reactions && message.reactions.length > 0)}
          <EmojiPicker
            floating
            onSelect={(emoji) => selectEmoji(message.id, emoji)}
          />
        {/if}
      {/if}
    </div>
  {/each}
</div>

{#if lightboxSrc}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="lightbox-overlay"
    on:click={closeLightbox}
    role="dialog"
    aria-label="Image preview"
    tabindex="-1"
  >
    <button class="lightbox-close" on:click|stopPropagation={closeLightbox}>X</button>
    <div class="lightbox-image-container" on:click|stopPropagation role="presentation">
      <img
        class="lightbox-image"
        src={lightboxSrc}
        alt={lightboxAlt}
      />
    </div>
  </div>
{/if}
