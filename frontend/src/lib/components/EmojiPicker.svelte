<script lang="ts">
  import { onMount } from 'svelte';
  import { API, type CustomEmoji } from '$lib/api';

  export let floating: boolean = false;
  export let onSelect: (emoji: string) => void = () => {};
  export let customEmojis: CustomEmoji[] = [];

  let tab: 'standard' | 'custom' = 'standard';
  let uploadName = '';
  let uploadFile: File | null = null;
  let uploading = false;
  let uploadError = '';
  let fileInput: HTMLInputElement;

  const COMMON_EMOJI = [
    "\u{1F44D}",
    "\u{1F44E}",
    "\u{2764}\u{FE0F}",
    "\u{1F602}",
    "\u{1F622}",
    "\u{1F621}",
    "\u{1F389}",
    "\u{1F525}",
    "\u{1F44F}",
    "\u{1F914}",
    "\u{1F440}",
    "\u{1F680}",
    "\u{2705}",
    "\u{274C}",
    "\u{1F4AF}",
    "\u{1F60D}",
    "\u{1F631}",
    "\u{1F64F}",
    "\u{1F499}",
    "\u{1F49A}",
  ];

  function selectCustomEmoji(emoji: CustomEmoji) {
    onSelect(`:${emoji.name}:`);
  }

  function handleFileSelect(e: Event) {
    const target = e.target as HTMLInputElement;
    if (target.files && target.files.length > 0) {
      uploadFile = target.files[0];
      if (!uploadName) {
        uploadName = uploadFile.name.replace(/\.[^.]+$/, '').replace(/[^a-zA-Z0-9_-]/g, '_');
      }
    }
  }

  async function handleUpload() {
    if (!uploadFile || !uploadName.trim()) return;
    uploading = true;
    uploadError = '';
    try {
      const emoji = await API.uploadCustomEmoji(uploadName.trim(), uploadFile);
      customEmojis = [...customEmojis, emoji];
      uploadName = '';
      uploadFile = null;
      if (fileInput) fileInput.value = '';
    } catch (e: any) {
      uploadError = e.message || 'Upload failed';
    } finally {
      uploading = false;
    }
  }
</script>

<div class="emoji-picker {floating ? 'emoji-picker-floating' : ''}">
  <div class="emoji-picker-tabs">
    <button
      class="emoji-tab-btn {tab === 'standard' ? 'active' : ''}"
      on:click={() => (tab = 'standard')}>Standard</button
    >
    <button
      class="emoji-tab-btn {tab === 'custom' ? 'active' : ''}"
      on:click={() => (tab = 'custom')}>Custom</button
    >
  </div>

  {#if tab === 'standard'}
    <div class="emoji-grid">
      {#each COMMON_EMOJI as emoji}
        <button class="emoji-picker-btn" on:click={() => onSelect(emoji)}>{emoji}</button>
      {/each}
    </div>
  {:else}
    <div class="emoji-grid">
      {#each customEmojis as emoji}
        <button
          class="emoji-picker-btn custom-emoji-btn"
          on:click={() => selectCustomEmoji(emoji)}
          title=":{emoji.name}:"
        >
          <img
            src={API.getCustomEmojiUrl(emoji.id)}
            alt=":{emoji.name}:"
            class="custom-emoji-img-picker"
          />
        </button>
      {/each}
      {#if customEmojis.length === 0}
        <div class="emoji-picker-empty">No custom emojis yet</div>
      {/if}
    </div>
    <div class="emoji-upload-section">
      <div class="emoji-upload-row">
        <input
          type="text"
          bind:value={uploadName}
          placeholder="name"
          class="emoji-upload-name"
          maxlength="32"
        />
        <input
          type="file"
          accept="image/png,image/gif,image/webp,image/jpeg"
          on:change={handleFileSelect}
          bind:this={fileInput}
          class="emoji-upload-file"
        />
        <button
          class="emoji-upload-btn"
          on:click={handleUpload}
          disabled={uploading || !uploadFile || !uploadName.trim()}>Upload</button
        >
      </div>
      {#if uploadError}
        <div class="emoji-upload-error">{uploadError}</div>
      {/if}
    </div>
  {/if}
</div>
