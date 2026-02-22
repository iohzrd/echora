<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  let { username, type = "screen", onClose = () => {}, videoElement = $bindable() }: {
    username: string;
    type?: "screen" | "camera";
    onClose?: () => void;
    videoElement?: HTMLVideoElement;
  } = $props();

  let viewerElement: HTMLElement;
  let isFullscreen = $state(false);

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

<svelte:window onkeydown={onKeyDown} />

<div class="screen-share-viewer" bind:this={viewerElement}>
  <div class="screen-share-header">
    <span class="screen-share-title"
      >Watching {username}'s {type === "camera" ? "camera" : "screen"}</span
    >
    <div class="screen-share-controls">
      <button
        class="screen-share-control-btn"
        onclick={toggleFullscreen}
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
      <button class="screen-share-back-btn" onclick={onClose}
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
      ondblclick={toggleFullscreen}
    ></video>
  </div>
</div>
