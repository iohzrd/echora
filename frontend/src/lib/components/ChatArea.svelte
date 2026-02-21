<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { voiceStore } from '../stores/voiceStore';
  import { chatState } from '../stores/chatState';
  import { uiState } from '../stores/uiState';
  import { stopWatching, stopWatchingCamera } from '../actions/voice';
  import { registerScrollCallbacks } from '../actions/server';
  import MessageList from './MessageList.svelte';
  import MessageInput from './MessageInput.svelte';
  import ScreenShareViewer from './ScreenShareViewer.svelte';

  export let messageList: MessageList | undefined = undefined;

  let screenVideoElement: HTMLVideoElement;
  let cameraVideoElement: HTMLVideoElement;
  let screenAudioRef = { el: null as HTMLAudioElement | null };

  onMount(() => {
    registerScrollCallbacks(
      () => messageList?.scrollToBottom(),
      () => messageList?.isNearBottom() ?? false,
    );
  });

  // Watch for WS-driven stop signals
  $: if ($uiState.stopWatchingScreen) {
    stopWatching(screenVideoElement ?? null, screenAudioRef);
    uiState.update((s) => ({ ...s, stopWatchingScreen: false }));
  }
  $: if ($uiState.stopWatchingCamera) {
    stopWatchingCamera(cameraVideoElement ?? null);
    uiState.update((s) => ({ ...s, stopWatchingCamera: false }));
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
      onClose={() => stopWatching(screenVideoElement ?? null, screenAudioRef)}
      bind:videoElement={screenVideoElement}
    />
  {:else if $voiceStore.watchingCameraUserId}
    <ScreenShareViewer
      username={$voiceStore.watchingCameraUsername}
      type="camera"
      onClose={() => stopWatchingCamera(cameraVideoElement ?? null)}
      bind:videoElement={cameraVideoElement}
    />
  {:else}
    <MessageList bind:this={messageList} />

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
