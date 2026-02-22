<script lang="ts">
  import { API } from '../api';
  import { formatFileSize } from '../utils';
  import { chatState } from '../stores/chatState';
  import { sendMessage, sendTyping, cancelReply } from '../actions/chat';

  interface PendingFile {
    file: File;
    id?: string;
    progress: number;
    error?: string;
    uploading: boolean;
  }

  let messageText = '';
  let pendingFiles: PendingFile[] = [];
  let dragOver = false;
  let fileInput: HTMLInputElement;

  const MAX_FILE_SIZE = 25 * 1024 * 1024;
  const MAX_FILES = 5;

  async function uploadFile(pending: PendingFile) {
    pending.uploading = true;
    pending.progress = 0;
    pendingFiles = pendingFiles;

    try {
      const attachment = await API.uploadAttachment(pending.file);
      pending.id = attachment.id;
      pending.progress = 100;
      pending.uploading = false;
    } catch (e) {
      pending.error = e instanceof Error ? e.message : 'Upload failed';
      pending.uploading = false;
    }
    pendingFiles = pendingFiles;
  }

  function addFiles(files: FileList | File[]) {
    const fileArray = Array.from(files);
    for (const file of fileArray) {
      if (pendingFiles.length >= MAX_FILES) break;
      if (file.size > MAX_FILE_SIZE) {
        pendingFiles = [...pendingFiles, {
          file,
          progress: 0,
          error: `File too large (max ${formatFileSize(MAX_FILE_SIZE)})`,
          uploading: false,
        }];
        continue;
      }
      const pending: PendingFile = { file, progress: 0, uploading: false };
      pendingFiles = [...pendingFiles, pending];
      uploadFile(pending);
    }
  }

  function removeFile(index: number) {
    pendingFiles = pendingFiles.filter((_, i) => i !== index);
  }

  function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files) {
      addFiles(input.files);
      input.value = '';
    }
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragOver = false;
    if (event.dataTransfer?.files) {
      addFiles(event.dataTransfer.files);
    }
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    dragOver = true;
  }

  function handleDragLeave() {
    dragOver = false;
  }

  function handlePaste(event: ClipboardEvent) {
    const items = event.clipboardData?.items;
    if (!items) return;
    const files: File[] = [];
    for (const item of items) {
      if (item.kind === 'file') {
        const file = item.getAsFile();
        if (file) files.push(file);
      }
    }
    if (files.length > 0) {
      addFiles(files);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      doSend();
      return;
    }
    if (event.key === 'Escape' && $chatState.replyingTo) {
      cancelReply();
      return;
    }
    sendTyping();
  }

  function doSend() {
    const hasText = messageText.trim().length > 0;
    const uploadedIds = pendingFiles
      .filter((f) => f.id && !f.error)
      .map((f) => f.id!);
    const hasAttachments = uploadedIds.length > 0;
    const stillUploading = pendingFiles.some((f) => f.uploading);

    if (stillUploading) return;
    if (!hasText && !hasAttachments) return;

    sendMessage(messageText.trim(), hasAttachments ? uploadedIds : undefined);
    messageText = '';
    pendingFiles = [];
  }

  $: anyUploading = pendingFiles.some((f) => f.uploading);
</script>

<div
  class="message-input-area"
  class:drag-over={dragOver}
  on:drop={handleDrop}
  on:dragover={handleDragOver}
  on:dragleave={handleDragLeave}
  role="region"
>
  {#if $chatState.replyingTo}
    <div class="reply-bar">
      <span class="reply-bar-text"
        >Replying to <strong>{$chatState.replyingTo.author}</strong></span
      >
      <button
        class="reply-bar-cancel"
        on:click={cancelReply}
        title="Cancel reply">X</button
      >
    </div>
  {/if}
  {#if pendingFiles.length > 0}
    <div class="pending-files">
      {#each pendingFiles as pf, i}
        <div class="pending-file" class:error={!!pf.error}>
          <span class="pending-file-name" title={pf.file.name}>
            {pf.file.name}
          </span>
          <span class="pending-file-size">{formatFileSize(pf.file.size)}</span>
          {#if pf.uploading}
            <span class="pending-file-status">uploading...</span>
          {:else if pf.error}
            <span class="pending-file-status error-text">{pf.error}</span>
          {:else if pf.id}
            <span class="pending-file-status ready">ready</span>
          {/if}
          <button
            class="pending-file-remove"
            on:click={() => removeFile(i)}
            title="Remove"
          >X</button>
        </div>
      {/each}
    </div>
  {/if}
  <div class="input-row">
    <button
      class="attach-btn"
      on:click={() => fileInput.click()}
      title="Attach file"
      disabled={!$chatState.selectedChannelId || pendingFiles.length >= MAX_FILES}
    >+</button>
    <textarea
      class="message-input"
      placeholder="Message #{$chatState.selectedChannelName || 'channel'}"
      bind:value={messageText}
      on:keydown={handleKeydown}
      on:paste={handlePaste}
      disabled={!$chatState.selectedChannelId || anyUploading}
    ></textarea>
  </div>
  <input
    bind:this={fileInput}
    type="file"
    multiple
    class="hidden-file-input"
    on:change={handleFileSelect}
  />
  {#if dragOver}
    <div class="drop-overlay">Drop files here</div>
  {/if}
</div>

<style>
  .message-input-area {
    position: relative;
  }
  .drag-over {
    outline: 2px dashed var(--accent, #5865f2);
    outline-offset: -2px;
  }
  .hidden-file-input {
    display: none;
  }
  .input-row {
    display: flex;
    align-items: flex-end;
    gap: 6px;
  }
  .attach-btn {
    background: var(--bg-tertiary, #2b2d31);
    border: 1px solid var(--border, #3f4147);
    color: var(--text-secondary, #b5bac1);
    border-radius: 4px;
    width: 34px;
    height: 34px;
    font-size: 18px;
    cursor: pointer;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .attach-btn:hover:not(:disabled) {
    background: var(--bg-modifier-hover, #393b40);
    color: var(--text-primary, #f2f3f5);
  }
  .attach-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .pending-files {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border, #3f4147);
  }
  .pending-file {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-tertiary, #2b2d31);
    border: 1px solid var(--border, #3f4147);
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 12px;
    max-width: 280px;
  }
  .pending-file.error {
    border-color: var(--error, #f23f43);
  }
  .pending-file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 140px;
    color: var(--text-primary, #f2f3f5);
  }
  .pending-file-size {
    color: var(--text-muted, #6d6f78);
    flex-shrink: 0;
  }
  .pending-file-status {
    color: var(--text-muted, #6d6f78);
    flex-shrink: 0;
  }
  .pending-file-status.ready {
    color: var(--success, #23a55a);
  }
  .pending-file-status.error-text {
    color: var(--error, #f23f43);
  }
  .pending-file-remove {
    background: none;
    border: none;
    color: var(--text-muted, #6d6f78);
    cursor: pointer;
    font-size: 11px;
    padding: 0 2px;
    flex-shrink: 0;
  }
  .pending-file-remove:hover {
    color: var(--error, #f23f43);
  }
  .drop-overlay {
    position: absolute;
    inset: 0;
    background: rgba(88, 101, 242, 0.15);
    border: 2px dashed var(--accent, #5865f2);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent, #5865f2);
    font-size: 16px;
    font-weight: 600;
    pointer-events: none;
    z-index: 10;
  }
</style>
