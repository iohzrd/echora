<script lang="ts">
  import { voiceStore } from '../stores/voiceStore.svelte';
  import { leaveVoice, toggleMute, toggleDeafen, toggleScreenShare, toggleCamera } from '../actions/voice';
  import { switchVoiceInputMode, changePTTKey } from '../actions/server';
  import { formatKeyLabel, keyEventToTauriKey, mouseEventToTauriKey } from '../ptt';
  import AudioSettingsPanel from './AudioSettings.svelte';

  let recordingKey = $state(false);
  let showSettings = $state(false);

  function handleKeyRecord(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    const key = keyEventToTauriKey(e);
    if (key) {
      recordingKey = false;
      changePTTKey(key);
    }
  }

  function handleMouseRecord(e: MouseEvent) {
    if (e.button === 0 || e.button === 2) return;
    e.preventDefault();
    e.stopPropagation();
    const key = mouseEventToTauriKey(e);
    if (key) {
      recordingKey = false;
      changePTTKey(key);
    }
  }
</script>

{#if voiceStore.currentVoiceChannel}
  <div class="voice-panel">
    <div class="voice-panel-header">
      <span class="voice-panel-status">Voice Connected</span>
      <button
        class="voice-panel-leave"
        onclick={leaveVoice}
        title="Disconnect"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M3.7 16.3a1 1 0 0 1 0-1.4L8.6 10H2a1 1 0 1 1 0-2h6.6L3.7 3.1a1 1 0 0 1 1.4-1.4l6.7 6.7a1 1 0 0 1 0 1.4l-6.7 6.7a1 1 0 0 1-1.4-.2z"
          /><path
            d="M16 3a1 1 0 0 1 1 1v16a1 1 0 1 1-2 0V4a1 1 0 0 1 1-1z"
          /></svg
        >
      </button>
    </div>

    <div class="voice-panel-controls">
      <!-- Mic -->
      <button
        class="voice-control-btn {voiceStore.isMuted ? 'active' : ''} {voiceStore.voiceInputMode ===
          'push-to-talk' && voiceStore.pttActive
          ? 'ptt-transmitting'
          : ''}"
        onclick={toggleMute}
        title={voiceStore.voiceInputMode === 'push-to-talk'
          ? voiceStore.pttActive
            ? 'Transmitting'
            : 'PTT: Hold ' + formatKeyLabel(voiceStore.pttKey)
          : voiceStore.isMuted
            ? 'Unmute'
            : 'Mute'}
      >
        {#if voiceStore.isMuted}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M12 1a4 4 0 0 0-4 4v6a4 4 0 0 0 8 0V5a4 4 0 0 0-4-4z"
            /><path
              d="M6 10a1 1 0 1 0-2 0 8 8 0 0 0 7 7.93V21H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-3.07A8 8 0 0 0 20 10a1 1 0 1 0-2 0 6 6 0 0 1-12 0z"
            /><line
              x1="3"
              y1="3"
              x2="21"
              y2="21"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
            /></svg
          >
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M12 1a4 4 0 0 0-4 4v6a4 4 0 0 0 8 0V5a4 4 0 0 0-4-4z"
            /><path
              d="M6 10a1 1 0 1 0-2 0 8 8 0 0 0 7 7.93V21H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-3.07A8 8 0 0 0 20 10a1 1 0 1 0-2 0 6 6 0 0 1-12 0z"
            /></svg
          >
        {/if}
      </button>
      <!-- Headphones / Deafen -->
      <button
        class="voice-control-btn {voiceStore.isDeafened ? 'active' : ''}"
        onclick={toggleDeafen}
        title={voiceStore.isDeafened ? 'Undeafen' : 'Deafen'}
      >
        {#if voiceStore.isDeafened}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M2 14v-2a10 10 0 0 1 20 0v2a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h1.92A8 8 0 0 0 4.08 9H6a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"
            /><line
              x1="3"
              y1="3"
              x2="21"
              y2="21"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
            /></svg
          >
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M2 14v-2a10 10 0 0 1 20 0v2a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h1.92A8 8 0 0 0 4.08 9H6a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"
            /></svg
          >
        {/if}
      </button>
      <!-- Screen Share -->
      <button
        class="voice-control-btn screen {voiceStore.isScreenSharing ? 'active' : ''}"
        onclick={toggleScreenShare}
        title={voiceStore.isScreenSharing ? 'Stop Sharing' : 'Share Screen'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M3 4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h7v2H7a1 1 0 1 0 0 2h10a1 1 0 1 0 0-2h-3v-2h7a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2H3zm0 2h18v10H3V6z"
          /></svg
        >
      </button>
      <!-- Camera -->
      <button
        class="voice-control-btn camera {voiceStore.isCameraSharing ? 'active' : ''}"
        onclick={toggleCamera}
        title={voiceStore.isCameraSharing ? 'Stop Camera' : 'Start Camera'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M2 6a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6zm16 1.38l3.2-1.92A1 1 0 0 1 22.7 6.4v11.2a1 1 0 0 1-1.5.86L18 16.62V7.38z"
          /></svg
        >
      </button>
      <!-- Settings -->
      <button
        class="voice-control-btn settings-toggle"
        onclick={() => (showSettings = !showSettings)}
        title={showSettings ? 'Hide Settings' : 'Show Settings'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M13.82 22h-3.64a1 1 0 0 1-.98-.8l-.39-2.15a7.5 7.5 0 0 1-1.56-.9l-2.08.6a1 1 0 0 1-1.15-.45l-1.82-3.15a1 1 0 0 1 .17-1.25l1.7-1.45a7.6 7.6 0 0 1 0-1.8l-1.7-1.45a1 1 0 0 1-.17-1.25l1.82-3.15a1 1 0 0 1 1.15-.45l2.08.6c.47-.35.99-.65 1.56-.9l.39-2.15a1 1 0 0 1 .98-.8h3.64a1 1 0 0 1 .98.8l.39 2.15c.57.25 1.09.55 1.56.9l2.08-.6a1 1 0 0 1 1.15.45l1.82 3.15a1 1 0 0 1-.17 1.25l-1.7 1.45a7.6 7.6 0 0 1 0 1.8l1.7 1.45a1 1 0 0 1 .17 1.25l-1.82 3.15a1 1 0 0 1-1.15.45l-2.08-.6c-.47.35-.99.65-1.56.9l-.39 2.15a1 1 0 0 1-.98.8zM12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z"
          /></svg
        >
      </button>
    </div>

    {#if showSettings}
      <div class="voice-panel-settings">
        <div class="voice-settings">
          <div class="voice-mode-toggle">
            <button
              class="mode-btn {voiceStore.voiceInputMode === 'voice-activity' ? 'active' : ''}"
              onclick={() => switchVoiceInputMode('voice-activity')}
              title="Voice Activity Detection"
            >
              VAD
            </button>
            <button
              class="mode-btn {voiceStore.voiceInputMode === 'push-to-talk' ? 'active' : ''}"
              onclick={() => switchVoiceInputMode('push-to-talk')}
              title="Push to Talk"
            >
              PTT
            </button>
          </div>

          {#if voiceStore.voiceInputMode === 'push-to-talk'}
            <div class="ptt-key-binding">
              {#if recordingKey}
                <button
                  class="ptt-key-btn recording"
                  onkeydown={handleKeyRecord}
                  onmousedown={handleMouseRecord}
                  oncontextmenu={(e) => e.preventDefault()}
                  onblur={() => (recordingKey = false)}
                >
                  Press a key or mouse button...
                </button>
              {:else}
                <button
                  class="ptt-key-btn"
                  onclick={() => (recordingKey = true)}
                  title="Click to change PTT key"
                >
                  {formatKeyLabel(voiceStore.pttKey)}
                </button>
              {/if}
            </div>
          {/if}
        </div>

        <AudioSettingsPanel
          showSensitivity={voiceStore.voiceInputMode === 'voice-activity'}
        />
      </div>
    {/if}
  </div>
{/if}
