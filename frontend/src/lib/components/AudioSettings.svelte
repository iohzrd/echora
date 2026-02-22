<script lang="ts">
  import { audioSettingsStore } from '../stores/audioSettingsStore';
  import {
    changeInputDevice,
    changeOutputDevice,
    changeInputGain,
    changeOutputVolume,
    changeVadSensitivity,
    toggleNoiseSuppression,
  } from '../actions/audioSettings';

  export let showSensitivity: boolean = true;
</script>

<div class="audio-settings">
  <div class="audio-setting-row">
    <label class="audio-setting-label">INPUT</label>
    <select
      class="audio-select"
      value={$audioSettingsStore.inputDeviceId}
      on:change={(e) => changeInputDevice(e.currentTarget.value)}
    >
      <option value="">Default</option>
      {#each $audioSettingsStore.inputDevices as device}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label">OUTPUT</label>
    <select
      class="audio-select"
      value={$audioSettingsStore.outputDeviceId}
      on:change={(e) => changeOutputDevice(e.currentTarget.value)}
    >
      <option value="">Default</option>
      {#each $audioSettingsStore.outputDevices as device}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label">IN VOL</label>
    <input
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round($audioSettingsStore.inputGain * 100)}
      on:input={(e) => changeInputGain(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round($audioSettingsStore.inputGain * 100)}%</span>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label">OUT VOL</label>
    <input
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round($audioSettingsStore.outputVolume * 100)}
      on:input={(e) => changeOutputVolume(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round($audioSettingsStore.outputVolume * 100)}%</span>
  </div>

  {#if showSensitivity}
    <div class="audio-setting-row">
      <label class="audio-setting-label">SENS</label>
      <input
        type="range"
        class="audio-slider"
        min="0"
        max="100"
        value={$audioSettingsStore.vadSensitivity}
        on:input={(e) => changeVadSensitivity(parseInt(e.currentTarget.value))}
      />
      <span class="audio-value">{$audioSettingsStore.vadSensitivity}</span>
    </div>
  {/if}

  <div class="audio-setting-row">
    <label class="audio-setting-label">NOISE</label>
    <button
      class="mode-btn {$audioSettingsStore.noiseSuppression ? 'active' : ''}"
      on:click={() => toggleNoiseSuppression(!$audioSettingsStore.noiseSuppression)}
    >
      {$audioSettingsStore.noiseSuppression ? 'ON' : 'OFF'}
    </button>
  </div>
</div>
