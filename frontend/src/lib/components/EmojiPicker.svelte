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
  let fileInput: HTMLInputElement;
  let pickerEl: HTMLDivElement;
  let openBelow = $state(false);
  let activeCategory = $state(0);
  let searchQuery = $state("");
  let searchInput: HTMLInputElement;

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

<style>
.emoji-picker {
	display: flex;
	flex-direction: column;
	padding: 8px;
	background: var(--bg-secondary);
	border: 1px solid var(--border-input);
	border-radius: var(--radius-lg);
	position: absolute;
	bottom: 100%;
	left: 0;
	z-index: 10;
	box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	margin-bottom: 4px;
	width: 320px;
}

.emoji-picker-floating {
	position: absolute;
	bottom: 100%;
	right: 0;
	left: auto;
}

.emoji-picker-below {
	bottom: auto;
	top: 100%;
	margin-bottom: 0;
	margin-top: 4px;
}

.emoji-picker-tabs {
	display: flex;
	gap: 2px;
	margin-bottom: 6px;
	border-bottom: 1px solid var(--border-input);
	padding-bottom: 4px;
}

.emoji-tab-btn {
	background: none;
	border: none;
	color: var(--text-faint);
	font-size: 12px;
	cursor: pointer;
	padding: 2px 8px;
	border-radius: var(--radius-sm);
}

.emoji-tab-btn:hover {
	color: var(--text-normal);
	background: var(--bg-input);
}

.emoji-tab-btn.active {
	color: var(--text-white);
	background: var(--brand-primary);
}

.emoji-category-bar {
	display: flex;
	gap: 2px;
	margin-bottom: 4px;
	padding-bottom: 4px;
	border-bottom: 1px solid var(--border-input);
	overflow-x: auto;
}

.emoji-category-btn {
	background: none;
	border: none;
	font-size: 16px;
	cursor: pointer;
	padding: 2px 4px;
	border-radius: var(--radius-sm);
	line-height: 1;
	flex-shrink: 0;
	opacity: 0.5;
}

.emoji-category-btn:hover {
	background: var(--bg-input);
	opacity: 0.8;
}

.emoji-category-btn.active {
	opacity: 1;
	background: var(--bg-input);
}

.emoji-search {
	width: 100%;
	padding: 4px 8px;
	margin-bottom: 6px;
	border: 1px solid var(--border-input);
	border-radius: var(--radius-sm);
	background: var(--bg-input);
	color: var(--text-normal);
	font-size: 12px;
	outline: none;
	box-sizing: border-box;
}

.emoji-search:focus {
	border-color: var(--brand-primary);
}

.emoji-grid {
	display: grid;
	grid-template-columns: repeat(8, 1fr);
	gap: 2px;
	max-height: 250px;
	overflow-y: auto;
}

.emoji-picker-btn {
	background: none;
	border: none;
	font-size: 20px;
	cursor: pointer;
	padding: 4px;
	border-radius: var(--radius-md);
	line-height: 1;
}

.emoji-picker-btn:hover {
	background: var(--bg-input);
}

.custom-emoji-btn {
	display: flex;
	align-items: center;
	justify-content: center;
}

.custom-emoji-img-picker {
	width: 24px;
	height: 24px;
	object-fit: contain;
}

.emoji-picker-empty {
	grid-column: 1 / -1;
	text-align: center;
	color: var(--text-faint);
	font-size: 12px;
	padding: 12px 0;
}

.emoji-upload-section {
	border-top: 1px solid var(--border-input);
	margin-top: 6px;
	padding-top: 6px;
}

.emoji-upload-row {
	display: flex;
	gap: 4px;
	align-items: center;
}

.emoji-upload-name {
	width: 80px;
	padding: 2px 6px;
	border: 1px solid var(--border-input);
	border-radius: var(--radius-sm);
	background: var(--bg-input);
	color: var(--text-normal);
	font-size: 11px;
}

.emoji-upload-file {
	font-size: 11px;
	color: var(--text-faint);
	max-width: 100px;
}

.emoji-upload-btn {
	padding: 2px 8px;
	border: 1px solid var(--border-input);
	border-radius: var(--radius-sm);
	background: var(--brand-primary);
	color: var(--text-white);
	font-size: 11px;
	cursor: pointer;
}

.emoji-upload-btn:disabled {
	opacity: 0.5;
	cursor: not-allowed;
}

.emoji-upload-error {
	color: var(--status-error);
	font-size: 11px;
	margin-top: 4px;
}
</style>
