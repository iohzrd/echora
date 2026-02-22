<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { voiceStore } from "../stores/voiceStore.svelte";
  import { chatState } from "../stores/chatState.svelte";
  import { voiceManager } from "../voice";
  import { toggleSidebar, toggleMembersSidebar } from "../actions/ui";
  import { uiState } from "../stores/uiState.svelte";
  import { stopWatching, stopWatchingCamera } from "../actions/voice";
  import MessageList from "./MessageList.svelte";
  import MessageInput from "./MessageInput.svelte";
  import ScreenShareViewer from "./ScreenShareViewer.svelte";

  let screenVideoElement: HTMLVideoElement | undefined = $state();
  let cameraVideoElement: HTMLVideoElement | undefined = $state();
  let screenAudioEl: HTMLAudioElement | null = null;

  onMount(() => {
    voiceManager.onScreenTrack((track) => {
      if (track.kind === "video") {
        if (screenVideoElement) {
          screenVideoElement.srcObject = new MediaStream([track]);
          screenVideoElement
            .play()
            .catch((e) => console.warn("Screen video autoplay prevented:", e));
        }
      } else if (track.kind === "audio") {
        if (screenAudioEl) {
          screenAudioEl.srcObject = null;
          screenAudioEl.remove();
        }
        screenAudioEl = document.createElement("audio");
        screenAudioEl.autoplay = true;
        screenAudioEl.volume = 1.0;
        screenAudioEl.srcObject = new MediaStream([track]);
        (
          document.getElementById("audio-container") ?? document.body
        ).appendChild(screenAudioEl);
        screenAudioEl
          .play()
          .catch((e) => console.warn("Screen audio autoplay prevented:", e));
      }
    });

    voiceManager.onCameraTrack((track) => {
      if (track.kind === "video" && cameraVideoElement) {
        cameraVideoElement.srcObject = new MediaStream([track]);
        cameraVideoElement
          .play()
          .catch((e) => console.warn("Camera video autoplay prevented:", e));
      }
    });
  });

  onDestroy(() => {
    voiceManager.onScreenTrack(() => {});
    voiceManager.onCameraTrack(() => {});
    if (screenAudioEl) {
      screenAudioEl.srcObject = null;
      screenAudioEl.remove();
      screenAudioEl = null;
    }
  });

  function handleStopWatching() {
    stopWatching();
    if (screenVideoElement) screenVideoElement.srcObject = null;
    if (screenAudioEl) {
      screenAudioEl.srcObject = null;
      screenAudioEl.remove();
      screenAudioEl = null;
    }
  }

  function handleStopWatchingCamera() {
    stopWatchingCamera();
    if (cameraVideoElement) cameraVideoElement.srcObject = null;
  }

  // When the WS signals that a remote screen share stopped, tear down our DOM
  $effect(() => {
    if (!voiceStore.watchingScreenUserId && screenVideoElement?.srcObject) {
      handleStopWatching();
    }
  });
  $effect(() => {
    if (!voiceStore.watchingCameraUserId && cameraVideoElement?.srcObject) {
      handleStopWatchingCamera();
    }
  });

  function getTypingText(): string {
    const names = Object.values(chatState.typingUsers).map((u) => u.username);
    if (names.length === 1) return `${names[0]} is typing...`;
    if (names.length <= 3) return `${names.join(", ")} are typing...`;
    return "Several people are typing...";
  }
</script>

<div class="main-content">
  <div class="chat-header">
    <button class="hamburger-btn" onclick={toggleSidebar}>|||</button>
    <div class="channel-name">
      {chatState.selectedChannelName || "Select a channel"}
    </div>
    <button
      class="members-toggle-btn"
      class:active={uiState.membersSidebarOpen}
      onclick={toggleMembersSidebar}
      title="Toggle members list"
    >
      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
        <path
          d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z"
        />
      </svg>
    </button>
  </div>

  {#if voiceStore.watchingScreenUserId}
    <ScreenShareViewer
      username={voiceStore.watchingScreenUsername}
      onClose={handleStopWatching}
      bind:videoElement={screenVideoElement}
    />
  {:else if voiceStore.watchingCameraUserId}
    <ScreenShareViewer
      username={voiceStore.watchingCameraUsername}
      type="camera"
      onClose={handleStopWatchingCamera}
      bind:videoElement={cameraVideoElement}
    />
  {:else}
    <MessageList />

    {#if Object.keys(chatState.typingUsers).length > 0}
      <div class="typing-indicator">
        <span class="typing-text">{getTypingText()}</span>
      </div>
    {/if}

    {#if chatState.rateLimitWarning}
      <div class="rate-limit-warning">
        Slow down! You are sending messages too fast.
      </div>
    {/if}

    {#if chatState.sendError}
      <div class="rate-limit-warning">
        Not connected. Your message was not sent.
      </div>
    {/if}

    <MessageInput />
  {/if}
</div>

<style>
  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    background-color: var(--bg-primary);
    min-height: 0;
    min-width: 0;
  }

  .chat-header {
    height: 48px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border-input);
    background-color: var(--bg-primary);
    flex-shrink: 0;
  }

  .chat-header .channel-name {
    flex: 1;
    font-weight: 600;
    color: var(--text-white);
  }

  .members-toggle-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 6px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background-color 0.15s ease,
      color 0.15s ease;
    flex-shrink: 0;
  }

  .members-toggle-btn:hover {
    background-color: var(--bg-hover);
    color: var(--text-normal);
  }

  .members-toggle-btn.active {
    color: var(--text-white);
  }

  .chat-header .channel-name::before {
    content: "#";
    color: var(--text-muted);
    margin-right: 4px;
  }

  .typing-indicator {
    padding: 4px 16px;
    font-size: 12px;
    color: var(--text-tertiary);
    background-color: var(--bg-primary);
    flex-shrink: 0;
  }

  .typing-text {
    font-style: italic;
  }

  .rate-limit-warning {
    padding: 4px 16px;
    font-size: 12px;
    color: var(--status-error);
    background-color: var(--bg-primary);
    flex-shrink: 0;
  }

  @media (max-width: 480px) {
    .chat-header {
      padding: 0 8px;
    }
  }
</style>
