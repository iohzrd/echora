<script lang="ts">
  import { onMount } from "svelte";
  import { API, type CustomEmoji } from "$lib/api";
  import { serverState } from "$lib/stores/serverState";
  import { EMOJI_CATEGORIES, type EmojiEntry } from "$lib/emoji-data";

  let { floating = false, onSelect = () => {}, customEmojis = [] }: {
    floating?: boolean;
    onSelect?: (emoji: string) => void;
    customEmojis?: CustomEmoji[];
  } = $props();

  let tab: "standard" | "custom" = $state("standard");
  let uploadName = $state("");
  let uploadFile: File | null = $state(null);
  let uploading = $state(false);
  let uploadError = $state("");
  let fileInput: HTMLInputElement = $state()!;
  let pickerEl: HTMLDivElement = $state()!;
  let openBelow = $state(false);
  let activeCategory = $state(0);
  let searchQuery = $state("");
  let searchInput: HTMLInputElement = $state()!;

  onMount(() => {
    if (pickerEl) {
      const rect = pickerEl.getBoundingClientRect();
      if (rect.top < 0) {
        openBelow = true;
      }
    }
    if (searchInput) {
      searchInput.focus();
    }
  });

  let searchResults = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return null;
    const results: EmojiEntry[] = [];
    for (const cat of EMOJI_CATEGORIES) {
      for (const e of cat.emojis) {
        if (
          e.description.toLowerCase().includes(q) ||
          e.keywords.some((k) => k.toLowerCase().includes(q))
        ) {
          results.push(e);
          if (results.length >= 50) return results;
        }
      }
    }
    return results;
  });

  function selectCustomEmoji(emoji: CustomEmoji) {
    onSelect(`:${emoji.name}:`);
  }

  function handleFileSelect(e: Event) {
    const target = e.target as HTMLInputElement;
    if (target.files && target.files.length > 0) {
      uploadFile = target.files[0];
      if (!uploadName) {
        uploadName = uploadFile.name
          .replace(/\.[^.]+$/, "")
          .replace(/[^a-zA-Z0-9_-]/g, "_");
      }
    }
  }

  async function handleUpload() {
    if (!uploadFile || !uploadName.trim()) return;
    uploading = true;
    uploadError = "";
    try {
      const emoji = await API.uploadCustomEmoji(uploadName.trim(), uploadFile);
      serverState.update((s) => ({ ...s, customEmojis: [...s.customEmojis, emoji] }));
      uploadName = "";
      uploadFile = null;
      if (fileInput) fileInput.value = "";
    } catch (e: any) {
      uploadError = e.message || "Upload failed";
    } finally {
      uploading = false;
    }
  }
</script>

<div
  class="emoji-picker {floating ? 'emoji-picker-floating' : ''} {openBelow
    ? 'emoji-picker-below'
    : ''}"
  bind:this={pickerEl}
>
  <div class="emoji-picker-tabs">
    <button
      class="emoji-tab-btn {tab === 'standard' ? 'active' : ''}"
      onclick={() => (tab = "standard")}>Standard</button
    >
    <button
      class="emoji-tab-btn {tab === 'custom' ? 'active' : ''}"
      onclick={() => (tab = "custom")}>Custom</button
    >
  </div>

  {#if tab === "standard"}
    <div class="emoji-category-bar">
      {#each EMOJI_CATEGORIES as cat, i}
        <button
          class="emoji-category-btn {activeCategory === i && !searchQuery.trim()
            ? 'active'
            : ''}"
          onclick={() => {
            activeCategory = i;
            searchQuery = "";
          }}
          title={cat.name}>{cat.icon}</button
        >
      {/each}
    </div>
    <input
      type="text"
      class="emoji-search"
      placeholder="Search emoji..."
      bind:value={searchQuery}
      bind:this={searchInput}
    />
    <div class="emoji-grid">
      {#if searchResults !== null}
        {#each searchResults as entry}
          <button
            class="emoji-picker-btn"
            onclick={() => onSelect(entry.emoji)}
            title={entry.description}>{entry.emoji}</button
          >
        {/each}
        {#if searchResults.length === 0}
          <div class="emoji-picker-empty">No matches</div>
        {/if}
      {:else}
        {#each EMOJI_CATEGORIES[activeCategory].emojis as entry}
          <button
            class="emoji-picker-btn"
            onclick={() => onSelect(entry.emoji)}
            title={entry.description}>{entry.emoji}</button
          >
        {/each}
      {/if}
    </div>
  {:else}
    <div class="emoji-grid">
      {#each customEmojis as emoji}
        <button
          class="emoji-picker-btn custom-emoji-btn"
          onclick={() => selectCustomEmoji(emoji)}
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
          onchange={handleFileSelect}
          bind:this={fileInput}
          class="emoji-upload-file"
        />
        <button
          class="emoji-upload-btn"
          onclick={handleUpload}
          disabled={uploading || !uploadFile || !uploadName.trim()}
          >Upload</button
        >
      </div>
      {#if uploadError}
        <div class="emoji-upload-error">{uploadError}</div>
      {/if}
    </div>
  {/if}
</div>
