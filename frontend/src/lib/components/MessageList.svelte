<script lang="ts">
  import { API, type CustomEmoji } from '../api';
  import { renderMarkdown } from '../markdown';
  import { formatTimestamp, truncateContent, formatFileSize } from '../utils';
  import { getApiBase } from '../config';
  import { isTauri } from '../serverManager';
  import { user } from '../auth';
  import { chatState } from '../stores/chatState';
  import { serverState } from '../stores/serverState';
  import { uiState } from '../stores/uiState';
  import {
    loadOlderMessages,
    startEditMessage,
    saveEditMessage,
    cancelEditMessage,
    deleteMessage,
    startReply,
    toggleReaction,
    updateEditMessageContent,
  } from '../actions/chat';
  import EmojiPicker from './EmojiPicker.svelte';
  import Avatar from './Avatar.svelte';

  function resolveUrl(url: string): string {
    if (isTauri && url.startsWith('/')) {
      const apiBase = getApiBase();
      const origin = apiBase.replace(/\/api$/, '');
      return origin + url;
    }
    return url;
  }

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

  let prevMessageCount = 0;
  $: if ($chatState.messages.length > prevMessageCount) {
    prevMessageCount = $chatState.messages.length;
    if (isNearBottom()) {
      requestAnimationFrame(() => {
        if (messagesArea) messagesArea.scrollTop = messagesArea.scrollHeight;
      });
    }
  }

  function handleScroll() {
    if (messagesArea && messagesArea.scrollTop === 0) {
      loadOlderMessages(() => preserveScroll(() => {}));
    }
  }

  function handleEditKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      saveEditMessage();
    } else if (event.key === 'Escape') {
      cancelEditMessage();
    }
  }

  function toggleEmojiPicker(messageId: string) {
    emojiPickerMessageId = emojiPickerMessageId === messageId ? null : messageId;
  }

  function selectEmoji(messageId: string, emoji: string) {
    toggleReaction(messageId, emoji);
    emojiPickerMessageId = null;
  }

  function isImageType(contentType: string): boolean {
    return contentType.startsWith('image/');
  }

  function isVideoType(contentType: string): boolean {
    return contentType.startsWith('video/');
  }

  function isAudioType(contentType: string): boolean {
    return contentType.startsWith('audio/');
  }

  function getAttachmentUrl(attachmentId: string, filename: string): string {
    return resolveUrl(API.getAttachmentUrl(attachmentId, filename));
  }

  function isCustomEmoji(emoji: string): boolean {
    return /^:[a-zA-Z0-9_-]+:$/.test(emoji);
  }

  function getCustomEmojiData(emoji: string): CustomEmoji | undefined {
    const name = emoji.slice(1, -1);
    return $serverState.customEmojis.find((e) => e.name === name);
  }

  function getCustomEmojiImageUrl(emoji: string): string | null {
    const data = getCustomEmojiData(emoji);
    if (!data) return null;
    return resolveUrl(API.getCustomEmojiUrl(data.id));
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
  let lightboxAlt: string = '';

  function openLightbox(src: string, alt: string) {
    lightboxSrc = src;
    lightboxAlt = alt;
  }

  function closeLightbox() {
    lightboxSrc = null;
    lightboxAlt = '';
  }

  function handleLightboxKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeLightbox();
    }
  }

  $: currentUserId = $user?.id || '';
  $: userRole = $user?.role || 'member';
</script>

<svelte:window on:keydown={handleLightboxKeydown} />

<div class="messages-area" bind:this={messagesArea} on:scroll={handleScroll}>
  {#if $chatState.loadingMore}
    <div class="loading-more">Loading older messages...</div>
  {/if}
  {#each $chatState.messages as message}
    <div class="message">
      <div class="message-avatar-wrapper">
        <Avatar
          username={message.author}
          avatarUrl={$serverState.userAvatars[message.author_id]}
          size="medium"
        />
      </div>
      <div class="message-content">
        <div class="message-header">
          <button
            class="message-author"
            on:click={() => uiState.update((s) => ({ ...s, profileViewUserId: message.author_id }))}
          >{message.author}</button>
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
        {#if $chatState.editingMessageId === message.id}
          <div class="edit-message-form">
            <textarea
              class="edit-message-input"
              value={$chatState.editMessageContent}
              on:input={(e) => updateEditMessageContent(e.currentTarget.value)}
              on:keydown={handleEditKeydown}
            ></textarea>
            <div class="edit-message-actions">
              <button class="edit-action-btn cancel" on:click={cancelEditMessage}
                >Cancel</button
              >
              <button class="edit-action-btn save" on:click={saveEditMessage}
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
                {expandedMessages.has(message.id) ? 'Show less' : 'Show more'}
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
                      resolveUrl(preview.image_url || ''),
                      preview.title || '',
                    )}
                >
                  <img
                    class="link-preview-image"
                    src={resolveUrl(preview.image_url)}
                    alt={preview.title || ''}
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
                on:click={() => toggleReaction(message.id, reaction.emoji)}
                title={reaction.emoji}
              >
                {#if isCustomEmoji(reaction.emoji)}
                  {@const imgUrl = getCustomEmojiImageUrl(reaction.emoji)}
                  {#if imgUrl}
                    <img src={imgUrl} alt={reaction.emoji} class="custom-emoji-reaction" />
                  {:else}
                    {reaction.emoji}
                  {/if}
                {:else}
                  {reaction.emoji}
                {/if}
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
                customEmojis={$serverState.customEmojis}
              />
            {/if}
          </div>
        {/if}
      </div>
      {#if $chatState.editingMessageId !== message.id}
        <div class="message-actions">
          <button
            class="msg-action-btn"
            on:click={() => startReply(message)}
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
              on:click={() => startEditMessage(message)}
              title="Edit">E</button
            >
          {/if}
          {#if message.author_id === currentUserId || userRole === 'moderator' || userRole === 'admin' || userRole === 'owner'}
            <button
              class="msg-action-btn delete"
              on:click={() => deleteMessage(message.id)}
              title="Delete">X</button
            >
          {/if}
        </div>
        {#if emojiPickerMessageId === message.id && !(message.reactions && message.reactions.length > 0)}
          <EmojiPicker
            floating
            onSelect={(emoji) => selectEmoji(message.id, emoji)}
            customEmojis={$serverState.customEmojis}
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
