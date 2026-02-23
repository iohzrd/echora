<script lang="ts">
  import { onMount } from "svelte";
  import { API, type SoundboardSound } from "../api";
  import { voiceStore } from "../stores/voiceStore.svelte";
  import { soundboardStore } from "../stores/soundboardStore.svelte";
  import { authState } from "../stores/authState.svelte";
  import { precacheSoundAudio } from "../soundboardAudio";

  let tab: "all" | "favorites" = $state("all");
  let search = $state("");
  let showUpload = $state(false);
  let playingId = $state<string | null>(null);
  let errorMessage = $state("");

  // Upload form state
  let uploadName = $state("");
  let uploadVolume = $state(1.0);
  let uploadFile = $state<File | null>(null);
  let uploading = $state(false);
  let uploadError = $state("");
  let fileInput: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      const [sounds, favorites] = await Promise.all([
        API.getSoundboardSounds(),
        API.getSoundboardFavorites(),
      ]);
      soundboardStore.sounds = sounds;
      soundboardStore.favorites = favorites;
      // Pre-cache audio for all sounds
      for (const sound of sounds) {
        precacheSoundAudio(sound.id);
      }
    } catch {
      // ignore load errors
    }
  });

  let filteredSounds = $derived.by(() => {
    let sounds = soundboardStore.sounds;
    if (tab === "favorites") {
      sounds = sounds.filter((s) => soundboardStore.favorites.includes(s.id));
    }
    if (search.trim()) {
      const q = search.toLowerCase();
      sounds = sounds.filter((s) => s.name.toLowerCase().includes(q));
    }
    return sounds;
  });

  async function handlePlay(sound: SoundboardSound) {
    if (!voiceStore.currentVoiceChannel || playingId) return;
    try {
      playingId = sound.id;
      errorMessage = "";
      await API.playSoundboardSound(sound.id, voiceStore.currentVoiceChannel);
    } catch (e: unknown) {
      errorMessage = e instanceof Error ? e.message : "Failed to play sound";
    } finally {
      setTimeout(() => {
        playingId = null;
      }, 500);
    }
  }

  async function handleToggleFavorite(soundId: string) {
    try {
      const result = await API.toggleSoundboardFavorite(soundId);
      if (result.favorited) {
        soundboardStore.favorites = [...soundboardStore.favorites, soundId];
      } else {
        soundboardStore.favorites = soundboardStore.favorites.filter(
          (id) => id !== soundId,
        );
      }
    } catch {
      // ignore
    }
  }

  async function handleDelete(soundId: string) {
    try {
      await API.deleteSoundboardSound(soundId);
      // WS event will remove it from store
    } catch (e: unknown) {
      errorMessage = e instanceof Error ? e.message : "Failed to delete sound";
    }
  }

  async function handleUpload() {
    if (!uploadFile || !uploadName.trim()) return;
    uploading = true;
    uploadError = "";
    try {
      await API.uploadSoundboardSound(
        uploadName.trim(),
        uploadFile,
        uploadVolume,
      );
      // Reset form
      uploadName = "";
      uploadVolume = 1.0;
      uploadFile = null;
      if (fileInput) fileInput.value = "";
      showUpload = false;
    } catch (e: unknown) {
      uploadError = e instanceof Error ? e.message : "Upload failed";
    } finally {
      uploading = false;
    }
  }

  function formatDuration(ms: number): string {
    const seconds = (ms / 1000).toFixed(1);
    return `${seconds}s`;
  }

  function canManageSound(sound: SoundboardSound): boolean {
    const user = authState.user;
    if (!user) return false;
    if (sound.created_by === user.id) return true;
    const role = user.role;
    return role === "admin" || role === "owner" || role === "moderator";
  }
</script>

<div class="soundboard-panel">
  <div class="soundboard-header">
    <div class="soundboard-tabs">
      <button
        class="soundboard-tab {tab === 'all' ? 'active' : ''}"
        onclick={() => (tab = "all")}
      >
        All
      </button>
      <button
        class="soundboard-tab {tab === 'favorites' ? 'active' : ''}"
        onclick={() => (tab = "favorites")}
      >
        Favorites
      </button>
    </div>
    <button
      class="soundboard-add-btn"
      onclick={() => (showUpload = !showUpload)}
      title="Upload Sound"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <path
          d="M12 4a1 1 0 0 1 1 1v6h6a1 1 0 1 1 0 2h-6v6a1 1 0 1 1-2 0v-6H5a1 1 0 1 1 0-2h6V5a1 1 0 0 1 1-1z"
        />
      </svg>
    </button>
  </div>

  {#if showUpload}
    <div class="soundboard-upload">
      <input
        type="text"
        class="soundboard-input"
        placeholder="Sound name (2-32 chars)"
        bind:value={uploadName}
        maxlength={32}
      />
      <div class="soundboard-upload-row">
        <input
          type="file"
          accept=".mp3,.ogg,.wav"
          onchange={(e) => {
            const f = (e.target as HTMLInputElement).files;
            if (f && f.length > 0) uploadFile = f[0];
          }}
          bind:this={fileInput}
          class="soundboard-file-input"
        />
      </div>
      <div class="soundboard-upload-row">
        <label class="soundboard-volume-label">
          Vol: {Math.round(uploadVolume * 100)}%
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            bind:value={uploadVolume}
            class="soundboard-volume-slider"
          />
        </label>
      </div>
      {#if uploadError}
        <div class="soundboard-error">{uploadError}</div>
      {/if}
      <button
        class="soundboard-upload-btn"
        onclick={handleUpload}
        disabled={uploading || !uploadFile || uploadName.trim().length < 2}
      >
        {uploading ? "Uploading..." : "Upload"}
      </button>
    </div>
  {/if}

  <input
    type="text"
    class="soundboard-search"
    placeholder="Search sounds..."
    bind:value={search}
  />

  {#if errorMessage}
    <div class="soundboard-error">{errorMessage}</div>
  {/if}

  <div class="soundboard-grid">
    {#each filteredSounds as sound (sound.id)}
      <div class="soundboard-sound {playingId === sound.id ? 'playing' : ''}">
        <button
          class="soundboard-play-btn"
          onclick={() => handlePlay(sound)}
          disabled={!voiceStore.currentVoiceChannel || playingId !== null}
          title="{sound.name} ({formatDuration(sound.duration_ms)})"
        >
          <span class="soundboard-sound-name">{sound.name}</span>
        </button>
        <div class="soundboard-sound-actions">
          <button
            class="soundboard-action-btn {soundboardStore.favorites.includes(
              sound.id,
            )
              ? 'favorited'
              : ''}"
            onclick={() => handleToggleFavorite(sound.id)}
            title={soundboardStore.favorites.includes(sound.id)
              ? "Unfavorite"
              : "Favorite"}
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill={soundboardStore.favorites.includes(sound.id)
                ? "currentColor"
                : "none"}
              stroke="currentColor"
              stroke-width="2"
            >
              <path
                d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"
              />
            </svg>
          </button>
          {#if canManageSound(sound)}
            <button
              class="soundboard-action-btn delete"
              onclick={() => handleDelete(sound.id)}
              title="Delete"
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path
                  d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14"
                />
              </svg>
            </button>
          {/if}
        </div>
      </div>
    {:else}
      <div class="soundboard-empty">
        {#if tab === "favorites"}
          No favorites yet
        {:else if search}
          No sounds match "{search}"
        {:else}
          No sounds uploaded yet
        {/if}
      </div>
    {/each}
  </div>
</div>
