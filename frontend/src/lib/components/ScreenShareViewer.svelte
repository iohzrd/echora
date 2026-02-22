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

<style>
.screen-share-viewer {
	display: flex;
	flex-direction: column;
	flex: 1;
	background-color: var(--bg-tertiary);
	overflow: hidden;
}

.screen-share-header {
	display: flex;
	align-items: center;
	justify-content: space-between;
	padding: 8px 16px;
	background-color: var(--bg-secondary);
	border-bottom: 1px solid var(--border-primary);
}

.screen-share-title {
	color: var(--text-normal);
	font-size: 14px;
	font-weight: 600;
}

.screen-share-controls {
	display: flex;
	align-items: center;
	gap: 8px;
}

.screen-share-control-btn {
	background: var(--bg-input);
	border: none;
	color: var(--text-muted);
	cursor: pointer;
	padding: 6px;
	border-radius: var(--radius-md);
	display: flex;
	align-items: center;
	justify-content: center;
	transition: background-color 0.2s, color 0.2s;
}

.screen-share-control-btn:hover {
	background: var(--bg-hover-strong);
	color: var(--text-normal);
}

.screen-share-back-btn {
	background: var(--bg-input);
	border: none;
	color: var(--text-normal);
	cursor: pointer;
	padding: 6px 12px;
	border-radius: var(--radius-md);
	font-size: 13px;
	transition: background-color 0.2s;
}

.screen-share-back-btn:hover {
	background: var(--bg-hover-strong);
}

.screen-share-viewer:fullscreen {
	background-color: #000;
}

.screen-share-viewer:fullscreen .screen-share-header {
	position: absolute;
	top: 0;
	left: 0;
	right: 0;
	z-index: 10;
	background-color: rgba(0, 0, 0, 0.7);
	border-bottom: none;
	opacity: 0;
	transition: opacity 0.3s;
}

.screen-share-viewer:fullscreen:hover .screen-share-header {
	opacity: 1;
}

.screen-share-viewer:fullscreen .screen-share-video-container {
	padding: 0;
}

.screen-share-viewer:fullscreen .screen-share-video {
	max-width: 100%;
	max-height: 100%;
	border-radius: 0;
}

.screen-share-video-container {
	flex: 1;
	display: flex;
	align-items: center;
	justify-content: center;
	padding: 16px;
	overflow: hidden;
}

.screen-share-video {
	max-width: 100%;
	max-height: 100%;
	object-fit: contain;
	border-radius: var(--radius-lg);
	background-color: #000;
}
</style>
