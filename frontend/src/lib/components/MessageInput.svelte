<script lang="ts">
  import type { Message } from "../api";

  export let channelName: string = "";
  export let disabled: boolean = false;
  export let replyingTo: Message | null = null;
  export let onSend: (text: string) => void = () => {};
  export let onTyping: () => void = () => {};
  export let onCancelReply: () => void = () => {};

  let messageText = "";

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (messageText.trim()) {
        onSend(messageText.trim());
        messageText = "";
      }
      return;
    }
    if (event.key === "Escape" && replyingTo) {
      onCancelReply();
      return;
    }
    onTyping();
  }
</script>

<div class="message-input-area">
  {#if replyingTo}
    <div class="reply-bar">
      <span class="reply-bar-text"
        >Replying to <strong>{replyingTo.author}</strong></span
      >
      <button
        class="reply-bar-cancel"
        on:click={onCancelReply}
        title="Cancel reply">X</button
      >
    </div>
  {/if}
  <textarea
    class="message-input"
    placeholder="Message #{channelName || 'channel'}"
    bind:value={messageText}
    on:keydown={handleKeydown}
    {disabled}
  ></textarea>
</div>
