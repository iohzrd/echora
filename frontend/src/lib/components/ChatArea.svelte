<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { voiceStore } from '../stores/voiceStore';
  import { chatState } from '../stores/chatState';
  import { uiState } from '../stores/uiState';
  import { voiceManager } from '../voice';
  import { stopWatching, stopWatchingCamera } from '../actions/voice';
  import MessageList from './MessageList.svelte';
  import MessageInput from './MessageInput.svelte';
  import ScreenShareViewer from './ScreenShareViewer.svelte';

  let screenVideoElement: HTMLVideoElement;
  let cameraVideoElement: HTMLVideoElement;
  let screenAudioEl: HTMLAudioElement | null = null;

  onMount(() => {
    voiceManager.onScreenTrack((track) => {
      if (track.kind === 'video') {
        if (screenVideoElement) {
          screenVideoElement.srcObject = new MediaStream([track]);
          screenVideoElement
            .play()
            .catch((e) => console.warn('Screen video autoplay prevented:', e));
        }
      } else if (track.kind === 'audio') {
        if (screenAudioEl) {
          screenAudioEl.srcObject = null;
          screenAudioEl.remove();
        }
        screenAudioEl = document.createElement('audio');
        screenAudioEl.autoplay = true;
        screenAudioEl.volume = 1.0;
        screenAudioEl.srcObject = new MediaStream([track]);
        document.body.appendChild(screenAudioEl);
        screenAudioEl
          .play()
          .catch((e) => console.warn('Screen audio autoplay prevented:', e));
      }
    });

    voiceManager.onCameraTrack((track) => {
      if (track.kind === 'video' && cameraVideoElement) {
        cameraVideoElement.srcObject = new MediaStream([track]);
        cameraVideoElement
          .play()
          .catch((e) => console.warn('Camera video autoplay prevented:', e));
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
  $: if (!$voiceStore.watchingScreenUserId && screenVideoElement?.srcObject) {
    handleStopWatching();
  }
  $: if (!$voiceStore.watchingCameraUserId && cameraVideoElement?.srcObject) {
    handleStopWatchingCamera();
  }

  function getTypingText(): string {
    const names = [...$chatState.typingUsers.values()].map((u) => u.username);
    if (names.length === 1) return `${names[0]} is typing...`;
    if (names.length <= 3) return `${names.join(', ')} are typing...`;
    return 'Several people are typing...';
  }
</script>

<div class="main-content">
  <div class="chat-header">
    <button
      class="hamburger-btn"
      on:click={() => uiState.update((s) => ({ ...s, sidebarOpen: !s.sidebarOpen }))}
    >|||</button>
    <div class="channel-name">
      {$chatState.selectedChannelName || 'Select a channel'}
    </div>
  </div>

  {#if $voiceStore.watchingScreenUserId}
    <ScreenShareViewer
      username={$voiceStore.watchingScreenUsername}
      onClose={handleStopWatching}
      bind:videoElement={screenVideoElement}
    />
  {:else if $voiceStore.watchingCameraUserId}
    <ScreenShareViewer
      username={$voiceStore.watchingCameraUsername}
      type="camera"
      onClose={handleStopWatchingCamera}
      bind:videoElement={cameraVideoElement}
    />
  {:else}
    <MessageList />

    {#if $chatState.typingUsers.size > 0}
      <div class="typing-indicator">
        <span class="typing-text">{getTypingText()}</span>
      </div>
    {/if}

    {#if $chatState.rateLimitWarning}
      <div class="rate-limit-warning">
        Slow down! You are sending messages too fast.
      </div>
    {/if}

    <MessageInput />
  {/if}
</div>
