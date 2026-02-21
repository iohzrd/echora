<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  export let username: string;
  export let type: "screen" | "camera" = "screen";
  export let onClose: () => void = () => {};

  export let videoElement: HTMLVideoElement;

  let viewerElement: HTMLElement;
  let isFullscreen = false;

  function toggleFullscreen() {
    if (!viewerElement) return;

    if (!document.fullscreenElement) {
      viewerElement.requestFullscreen().catch((e) => {
        console.warn("Fullscreen request failed:", e);
      });
    } else {
      document.exitFullscreen();
    }
  }

  function onFullscreenChange() {
    isFullscreen = !!document.fullscreenElement;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && !document.fullscreenElement) {
      onClose();
    }
  }

  onMount(() => {
    document.addEventListener("fullscreenchange", onFullscreenChange);
  });

  onDestroy(() => {
    document.removeEventListener("fullscreenchange", onFullscreenChange);
  });
</script>

<svelte:window on:keydown={onKeyDown} />

<div class="screen-share-viewer" bind:this={viewerElement}>
  <div class="screen-share-header">
    <span class="screen-share-title"
      >Watching {username}'s {type === "camera" ? "camera" : "screen"}</span
    >
    <div class="screen-share-controls">
      <button
        class="screen-share-control-btn"
        on:click={toggleFullscreen}
        title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
      >
        {#if isFullscreen}
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polyline points="4 14 10 14 10 20" />
            <polyline points="20 10 14 10 14 4" />
            <line x1="14" y1="10" x2="21" y2="3" />
            <line x1="3" y1="21" x2="10" y2="14" />
          </svg>
        {:else}
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polyline points="15 3 21 3 21 9" />
            <polyline points="9 21 3 21 3 15" />
            <line x1="21" y1="3" x2="14" y2="10" />
            <line x1="3" y1="21" x2="10" y2="14" />
          </svg>
        {/if}
      </button>
      <button class="screen-share-back-btn" on:click={onClose}
        >Back to chat</button
      >
    </div>
  </div>
  <div class="screen-share-video-container">
    <video
      class="screen-share-video"
      bind:this={videoElement}
      autoplay
      playsinline
      on:dblclick={toggleFullscreen}
    ></video>
  </div>
</div>
