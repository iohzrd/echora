<script lang="ts">
  import type { VoiceInputMode } from "../voice";
  import type { AudioDevice } from "../audioSettings";
  import { formatKeyLabel, keyEventToTauriKey } from "../ptt";
  import AudioSettingsPanel from "./AudioSettings.svelte";

  export let currentVoiceChannel: string | null = null;
  export let isMuted: boolean = false;
  export let isDeafened: boolean = false;
  export let isScreenSharing: boolean = false;
  export let isCameraSharing: boolean = false;
  export let voiceInputMode: VoiceInputMode = "voice-activity";
  export let pttKey: string = "Space";
  export let pttActive: boolean = false;

  // Audio settings
  export let inputDeviceId: string = "";
  export let outputDeviceId: string = "";
  export let inputGain: number = 1.0;
  export let outputVolume: number = 1.0;
  export let vadSensitivity: number = 50;
  export let noiseSuppression: boolean = true;
  export let inputDevices: AudioDevice[] = [];
  export let outputDevices: AudioDevice[] = [];
  export let showOutputDevice: boolean = true;

  export let onLeaveVoice: () => void = () => {};
  export let onToggleMute: () => void = () => {};
  export let onToggleDeafen: () => void = () => {};
  export let onToggleScreenShare: () => void = () => {};
  export let onToggleCamera: () => void = () => {};
  export let onSwitchInputMode: (mode: VoiceInputMode) => void = () => {};
  export let onChangePTTKey: (key: string) => void = () => {};
  export let onInputDeviceChange: (deviceId: string) => void = () => {};
  export let onOutputDeviceChange: (deviceId: string) => void = () => {};
  export let onInputGainChange: (gain: number) => void = () => {};
  export let onOutputVolumeChange: (volume: number) => void = () => {};
  export let onVadSensitivityChange: (sensitivity: number) => void = () => {};
  export let onNoiseSuppressionToggle: (enabled: boolean) => void = () => {};

  let recordingKey = false;
  let showSettings = false;

  function handleKeyRecord(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    const key = keyEventToTauriKey(e);
    if (key) {
      recordingKey = false;
      onChangePTTKey(key);
    }
  }
</script>

{#if currentVoiceChannel}
  <div class="voice-panel">
    <div class="voice-panel-header">
      <span class="voice-panel-status">Voice Connected</span>
      <button class="voice-panel-leave" on:click={onLeaveVoice} title="Disconnect">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M3.7 16.3a1 1 0 0 1 0-1.4L8.6 10H2a1 1 0 1 1 0-2h6.6L3.7 3.1a1 1 0 0 1 1.4-1.4l6.7 6.7a1 1 0 0 1 0 1.4l-6.7 6.7a1 1 0 0 1-1.4-.2z"/><path d="M16 3a1 1 0 0 1 1 1v16a1 1 0 1 1-2 0V4a1 1 0 0 1 1-1z"/></svg>
      </button>
    </div>

    <div class="voice-panel-controls">
      <!-- Mic -->
      <button
        class="voice-control-btn {isMuted ? 'active' : ''} {voiceInputMode === 'push-to-talk' && pttActive ? 'ptt-transmitting' : ''}"
        on:click={onToggleMute}
        title={voiceInputMode === 'push-to-talk'
          ? (pttActive ? 'Transmitting' : 'PTT: Hold ' + formatKeyLabel(pttKey))
          : (isMuted ? 'Unmute' : 'Mute')}
      >
        {#if isMuted}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1a4 4 0 0 0-4 4v6a4 4 0 0 0 8 0V5a4 4 0 0 0-4-4z"/><path d="M6 10a1 1 0 1 0-2 0 8 8 0 0 0 7 7.93V21H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-3.07A8 8 0 0 0 20 10a1 1 0 1 0-2 0 6 6 0 0 1-12 0z"/><line x1="3" y1="3" x2="21" y2="21" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1a4 4 0 0 0-4 4v6a4 4 0 0 0 8 0V5a4 4 0 0 0-4-4z"/><path d="M6 10a1 1 0 1 0-2 0 8 8 0 0 0 7 7.93V21H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-3.07A8 8 0 0 0 20 10a1 1 0 1 0-2 0 6 6 0 0 1-12 0z"/></svg>
        {/if}
      </button>
      <!-- Headphones / Deafen -->
      <button
        class="voice-control-btn {isDeafened ? 'active' : ''}"
        on:click={onToggleDeafen}
        title={isDeafened ? "Undeafen" : "Deafen"}
      >
        {#if isDeafened}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M2 14v-2a10 10 0 0 1 20 0v2a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h1.92A8 8 0 0 0 4.08 9H6a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"/><line x1="3" y1="3" x2="21" y2="21" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M2 14v-2a10 10 0 0 1 20 0v2a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h1.92A8 8 0 0 0 4.08 9H6a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"/></svg>
        {/if}
      </button>
      <!-- Screen Share -->
      <button
        class="voice-control-btn screen {isScreenSharing ? 'active' : ''}"
        on:click={onToggleScreenShare}
        title={isScreenSharing ? "Stop Sharing" : "Share Screen"}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M3 4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h7v2H7a1 1 0 1 0 0 2h10a1 1 0 1 0 0-2h-3v-2h7a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2H3zm0 2h18v10H3V6z"/></svg>
      </button>
      <!-- Camera -->
      <button
        class="voice-control-btn camera {isCameraSharing ? 'active' : ''}"
        on:click={onToggleCamera}
        title={isCameraSharing ? "Stop Camera" : "Start Camera"}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M2 6a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6zm16 1.38l3.2-1.92A1 1 0 0 1 22.7 6.4v11.2a1 1 0 0 1-1.5.86L18 16.62V7.38z"/></svg>
      </button>
      <!-- Settings -->
      <button
        class="voice-control-btn settings-toggle"
        on:click={() => (showSettings = !showSettings)}
        title={showSettings ? "Hide Settings" : "Show Settings"}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M13.82 22h-3.64a1 1 0 0 1-.98-.8l-.39-2.15a7.5 7.5 0 0 1-1.56-.9l-2.08.6a1 1 0 0 1-1.15-.45l-1.82-3.15a1 1 0 0 1 .17-1.25l1.7-1.45a7.6 7.6 0 0 1 0-1.8l-1.7-1.45a1 1 0 0 1-.17-1.25l1.82-3.15a1 1 0 0 1 1.15-.45l2.08.6c.47-.35.99-.65 1.56-.9l.39-2.15a1 1 0 0 1 .98-.8h3.64a1 1 0 0 1 .98.8l.39 2.15c.57.25 1.09.55 1.56.9l2.08-.6a1 1 0 0 1 1.15.45l1.82 3.15a1 1 0 0 1-.17 1.25l-1.7 1.45a7.6 7.6 0 0 1 0 1.8l1.7 1.45a1 1 0 0 1 .17 1.25l-1.82 3.15a1 1 0 0 1-1.15.45l-2.08-.6c-.47.35-.99.65-1.56.9l-.39 2.15a1 1 0 0 1-.98.8zM12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z"/></svg>
      </button>
    </div>

    {#if showSettings}
      <div class="voice-panel-settings">
        <div class="voice-settings">
          <div class="voice-mode-toggle">
            <button
              class="mode-btn {voiceInputMode === 'voice-activity' ? 'active' : ''}"
              on:click={() => onSwitchInputMode('voice-activity')}
              title="Voice Activity Detection"
            >
              VAD
            </button>
            <button
              class="mode-btn {voiceInputMode === 'push-to-talk' ? 'active' : ''}"
              on:click={() => onSwitchInputMode('push-to-talk')}
              title="Push to Talk"
            >
              PTT
            </button>
          </div>

          {#if voiceInputMode === 'push-to-talk'}
            <div class="ptt-key-binding">
              {#if recordingKey}
                <button
                  class="ptt-key-btn recording"
                  on:keydown={handleKeyRecord}
                  on:blur={() => (recordingKey = false)}
                >
                  Press a key...
                </button>
              {:else}
                <button
                  class="ptt-key-btn"
                  on:click={() => (recordingKey = true)}
                  title="Click to change PTT key"
                >
                  {formatKeyLabel(pttKey)}
                </button>
              {/if}
            </div>
          {/if}
        </div>

        <AudioSettingsPanel
          {inputDeviceId}
          {outputDeviceId}
          {inputGain}
          {outputVolume}
          {vadSensitivity}
          {noiseSuppression}
          {inputDevices}
          {outputDevices}
          showSensitivity={voiceInputMode === 'voice-activity'}
          {showOutputDevice}
          {onInputDeviceChange}
          {onOutputDeviceChange}
          {onInputGainChange}
          {onOutputVolumeChange}
          {onVadSensitivityChange}
          {onNoiseSuppressionToggle}
        />
      </div>
    {/if}
  </div>
{/if}
