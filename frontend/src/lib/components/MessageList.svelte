<script lang="ts">
  import { API, type CustomEmoji } from "../api";
  import { renderMarkdown } from "../markdown";
  import {
    formatTimestamp,
    truncateContent,
    formatFileSize,
    resolveUrl,
  } from "../utils";
  import { user, canDeleteMessage } from "../auth";
  import { chatState } from "../stores/chatState";
  import { serverState } from "../stores/serverState";
  import { viewUserProfile } from "../actions/ui";
  import {
    loadOlderMessages,
    startEditMessage,
    saveEditMessage,
    cancelEditMessage,
    deleteMessage,
    startReply,
    toggleReaction,
    updateEditMessageContent,
  } from "../actions/chat";
  import Avatar from "./Avatar.svelte";
  import {
    emojiPickerState,
    openEmojiPicker,
    closeEmojiPicker,
  } from "../stores/emojiPickerState";

  let messagesArea: HTMLDivElement;

  function scrollToBottom() {
    if (messagesArea) {
      messagesArea.scrollTop = messagesArea.scrollHeight;
    }
  }

  function captureScrollPos(): () => void {
    const prevScrollHeight = messagesArea?.scrollHeight ?? 0;
    return () => {
      requestAnimationFrame(() => {
        if (messagesArea) {
          messagesArea.scrollTop = messagesArea.scrollHeight - prevScrollHeight;
        }
      });
    };
  }

  function isNearBottom(): boolean {
    if (!messagesArea) return true;
    return (
      messagesArea.scrollHeight -
        messagesArea.scrollTop -
        messagesArea.clientHeight <
      100
    );
  }

  function scrollToBottomIfNear() {
    if (messagesArea && isNearBottom()) {
      requestAnimationFrame(() => {
        if (messagesArea) {
          messagesArea.scrollTop = messagesArea.scrollHeight;
        }
      });
    }
  }

  let prevChannelId = $state("");
  let prevMessageCount = $state(0);
  let needsScrollToBottom = $state(false);
  let scrollRafId: number | undefined;
  $effect(() => {
    const channelId = $chatState.selectedChannelId;
    const count = $chatState.messages.length;
    if (channelId !== prevChannelId) {
      // Channel switched — clear stale overflow tracking and scroll once messages load
      prevChannelId = channelId;
      prevMessageCount = count;
      needsScrollToBottom = true;
      expandedMessages = {};
      overflowingMessages = {};
      if (count > 0) {
        if (scrollRafId !== undefined) cancelAnimationFrame(scrollRafId);
        scrollRafId = requestAnimationFrame(() => {
          if (messagesArea) messagesArea.scrollTop = messagesArea.scrollHeight;
          needsScrollToBottom = false;
          scrollRafId = undefined;
        });
      }
    } else if (count > prevMessageCount) {
      prevMessageCount = count;
      if (needsScrollToBottom) {
        // Messages just loaded for this channel — scroll to bottom
        needsScrollToBottom = false;
        if (scrollRafId !== undefined) cancelAnimationFrame(scrollRafId);
        scrollRafId = requestAnimationFrame(() => {
          if (messagesArea) messagesArea.scrollTop = messagesArea.scrollHeight;
          scrollRafId = undefined;
        });
      } else if (isNearBottom()) {
        // New message in same channel — only scroll if already near the bottom
        if (scrollRafId !== undefined) cancelAnimationFrame(scrollRafId);
        scrollRafId = requestAnimationFrame(() => {
          if (messagesArea) messagesArea.scrollTop = messagesArea.scrollHeight;
          scrollRafId = undefined;
        });
      }
    }
    return () => {
      if (scrollRafId !== undefined) {
        cancelAnimationFrame(scrollRafId);
        scrollRafId = undefined;
      }
    };
  });

  function handleScroll() {
    if (messagesArea && messagesArea.scrollTop < 10) {
      loadOlderMessages(captureScrollPos);
    }
  }

  function handleEditKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      saveEditMessage();
    } else if (event.key === "Escape") {
      cancelEditMessage();
    }
  }

  // Tracks which message's picker was just closed by a pointerdown on the trigger
  // button, so the subsequent click doesn't re-open it.
  let suppressOpenForMessageId: string | null = null;

  function handleEmojiButtonPointerDown(messageId: string) {
    if ($emojiPickerState.messageId === messageId) {
      suppressOpenForMessageId = messageId;
    }
  }

  function toggleEmojiPicker(messageId: string, event: MouseEvent) {
    if (suppressOpenForMessageId === messageId) {
      suppressOpenForMessageId = null;
      return;
    }
    suppressOpenForMessageId = null;
    openEmojiPicker(messageId, event.currentTarget as HTMLElement, (emoji) => {
      toggleReaction(messageId, emoji);
    });
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
  let expandedMessages: Record<string, boolean> = $state({});
  let overflowingMessages: Record<string, boolean> = $state({});

  function toggleExpand(messageId: string) {
    expandedMessages[messageId] = !expandedMessages[messageId];
  }

  function checkOverflow(node: HTMLElement, messageId: string) {
    // Measure on next frame so the DOM has settled
    const rafId = requestAnimationFrame(() => {
      if (node.scrollHeight > COLLAPSE_HEIGHT) {
        overflowingMessages[messageId] = true;
      }
    });
    return {
      destroy() {
        cancelAnimationFrame(rafId);
        delete overflowingMessages[messageId];
        delete expandedMessages[messageId];
      },
    };
  }

  let lightboxSrc: string | null = $state(null);
  let lightboxAlt: string = $state("");

  function openLightbox(src: string, alt: string) {
    lightboxSrc = src;
    lightboxAlt = alt;
  }

  function closeLightbox() {
    lightboxSrc = null;
    lightboxAlt = "";
  }

  function handleLightboxKeydown(event: KeyboardEvent) {
    if (lightboxSrc && event.key === "Escape") {
      closeLightbox();
    }
  }

  let currentUserId = $derived($user?.id ?? "");
</script>

<svelte:window onkeydown={handleLightboxKeydown} />

<div class="messages-area" bind:this={messagesArea} onscroll={handleScroll}>
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
            onclick={() => viewUserProfile(message.author_id)}
            >{message.author}</button
          >
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
              oninput={(e) => updateEditMessageContent(e.currentTarget.value)}
              onkeydown={handleEditKeydown}
            ></textarea>
            <div class="edit-message-actions">
              <button class="edit-action-btn cancel" onclick={cancelEditMessage}
                >Cancel</button
              >
              <button class="edit-action-btn save" onclick={saveEditMessage}
                >Save</button
              >
            </div>
          </div>
        {:else if message.content}
          <div
            class="message-text"
            class:collapsed={overflowingMessages[message.id] &&
              !expandedMessages[message.id]}
            use:checkOverflow={message.id}
          >
            {@html renderMarkdown(message.content)}
          </div>
          {#if overflowingMessages[message.id]}
            <button
              class="message-expand-btn"
              onclick={() => toggleExpand(message.id)}
            >
              {expandedMessages[message.id] ? "Show less" : "Show more"}
            </button>
          {/if}
        {/if}
        {#if message.attachments && message.attachments.length > 0}
          <div class="attachments">
            {#each message.attachments as attachment}
              {#if isImageType(attachment.content_type)}
                <button
                  class="attachment-image-link"
                  onclick={() =>
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
                  <span class="attachment-audio-name"
                    >{attachment.filename}</span
                  >
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
                    <span class="attachment-file-name"
                      >{attachment.filename}</span
                    >
                    <span class="attachment-file-size"
                      >{formatFileSize(attachment.size)}</span
                    >
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
                  onclick={() =>
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
                onclick={() => toggleReaction(message.id, reaction.emoji)}
                title={reaction.users.join(", ")}
              >
                {#if isCustomEmoji(reaction.emoji)}
                  {@const imgUrl = getCustomEmojiImageUrl(reaction.emoji)}
                  {#if imgUrl}
                    <img
                      src={imgUrl}
                      alt={reaction.emoji}
                      class="custom-emoji-reaction"
                    />
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
              onpointerdown={() => handleEmojiButtonPointerDown(message.id)}
              onclick={(e) => toggleEmojiPicker(message.id, e)}
              title="Add reaction"
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="currentColor"
                ><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" /></svg
              >
            </button>
          </div>
        {/if}
      </div>
      {#if $chatState.editingMessageId !== message.id}
        <div class="message-actions">
          <button
            class="msg-action-btn"
            onclick={() => startReply(message)}
            title="Reply"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
              ><path
                d="M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z"
              /></svg
            >
          </button>
          <button
            class="msg-action-btn"
            onpointerdown={() => handleEmojiButtonPointerDown(message.id)}
            onclick={(e) => toggleEmojiPicker(message.id, e)}
            title="React"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
              ><path
                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 13.5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm3-5H11v-1h2v1zm1 5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zM17 9H7V7h10v2z"
              /></svg
            >
          </button>
          {#if message.author_id === currentUserId}
            <button
              class="msg-action-btn"
              onclick={() => startEditMessage(message)}
              title="Edit"
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="currentColor"
                ><path
                  d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
                /></svg
              >
            </button>
          {/if}
          {#if canDeleteMessage(message.author_id, currentUserId, $user?.role)}
            <button
              class="msg-action-btn delete"
              onclick={() => deleteMessage(message.id)}
              title="Delete"
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="currentColor"
                ><path
                  d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                /></svg
              >
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
</div>

{#if lightboxSrc}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="lightbox-overlay"
    onclick={closeLightbox}
    role="dialog"
    aria-label="Image preview"
    tabindex="-1"
  >
    <button
      class="lightbox-close"
      onclick={(e) => {
        e.stopPropagation();
        closeLightbox();
      }}
    >
      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"
        ><path
          d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
        /></svg
      >
    </button>
    <div
      class="lightbox-image-container"
      onclick={(e) => e.stopPropagation()}
      role="presentation"
    >
      <img class="lightbox-image" src={lightboxSrc} alt={lightboxAlt} />
    </div>
  </div>
{/if}
