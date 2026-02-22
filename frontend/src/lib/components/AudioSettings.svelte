<script lang="ts">
  import { audioSettingsStore } from '../stores/audioSettingsStore.svelte';
  import {
    changeInputDevice,
    changeOutputDevice,
    changeInputGain,
    changeOutputVolume,
    changeVadSensitivity,
    toggleNoiseSuppression,
  } from '../actions/audioSettings';

  let { showSensitivity = true }: { showSensitivity?: boolean } = $props();
</script>

<div class="audio-settings">
  <div class="audio-setting-row">
    <label class="audio-setting-label" for="audio-input-select">INPUT</label>
    <select
      id="audio-input-select"
      class="audio-select"
      value={audioSettingsStore.inputDeviceId}
      onchange={(e) => changeInputDevice(e.currentTarget.value)}
    >
      <option value="">Default</option>
      {#each audioSettingsStore.inputDevices as device}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label" for="audio-output-select">OUTPUT</label>
    <select
      id="audio-output-select"
      class="audio-select"
      value={audioSettingsStore.outputDeviceId}
      onchange={(e) => changeOutputDevice(e.currentTarget.value)}
    >
      <option value="">Default</option>
      {#each audioSettingsStore.outputDevices as device}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label" for="audio-input-gain">IN VOL</label>
    <input
      id="audio-input-gain"
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round(audioSettingsStore.inputGain * 100)}
      oninput={(e) => changeInputGain(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(audioSettingsStore.inputGain * 100)}%</span>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label" for="audio-output-volume">OUT VOL</label>
    <input
      id="audio-output-volume"
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round(audioSettingsStore.outputVolume * 100)}
      oninput={(e) => changeOutputVolume(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(audioSettingsStore.outputVolume * 100)}%</span>
  </div>

  {#if showSensitivity}
    <div class="audio-setting-row">
      <label class="audio-setting-label" for="audio-vad-sensitivity">SENS</label>
      <input
        id="audio-vad-sensitivity"
        type="range"
        class="audio-slider"
        min="0"
        max="100"
        value={audioSettingsStore.vadSensitivity}
        oninput={(e) => changeVadSensitivity(parseInt(e.currentTarget.value))}
      />
      <span class="audio-value">{audioSettingsStore.vadSensitivity}</span>
    </div>
  {/if}

  <div class="audio-setting-row">
    <span class="audio-setting-label">NOISE</span>
    <button
      class="mode-btn {audioSettingsStore.noiseSuppression ? 'active' : ''}"
      onclick={() => toggleNoiseSuppression(!audioSettingsStore.noiseSuppression)}
    >
      {audioSettingsStore.noiseSuppression ? 'ON' : 'OFF'}
    </button>
  </div>
</div>
