<script lang="ts">
  import type { AudioDevice } from "../audioSettings";

  export let inputDeviceId: string = "";
  export let outputDeviceId: string = "";
  export let inputGain: number = 1.0;
  export let outputVolume: number = 1.0;
  export let vadSensitivity: number = 50;
  export let noiseSuppression: boolean = true;
  export let inputDevices: AudioDevice[] = [];
  export let outputDevices: AudioDevice[] = [];
  export let showSensitivity: boolean = true;
  export let showOutputDevice: boolean = true;

  export let onInputDeviceChange: (deviceId: string) => void = () => {};
  export let onOutputDeviceChange: (deviceId: string) => void = () => {};
  export let onInputGainChange: (gain: number) => void = () => {};
  export let onOutputVolumeChange: (volume: number) => void = () => {};
  export let onVadSensitivityChange: (sensitivity: number) => void = () => {};
  export let onNoiseSuppressionToggle: (enabled: boolean) => void = () => {};
</script>

<div class="audio-settings">
  <div class="audio-setting-row">
    <label class="audio-setting-label">INPUT</label>
    <select
      class="audio-select"
      value={inputDeviceId}
      on:change={(e) => onInputDeviceChange(e.currentTarget.value)}
    >
      <option value="">Default</option>
      {#each inputDevices as device}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </div>

  {#if showOutputDevice}
    <div class="audio-setting-row">
      <label class="audio-setting-label">OUTPUT</label>
      <select
        class="audio-select"
        value={outputDeviceId}
        on:change={(e) => onOutputDeviceChange(e.currentTarget.value)}
      >
        <option value="">Default</option>
        {#each outputDevices as device}
          <option value={device.deviceId}>{device.label}</option>
        {/each}
      </select>
    </div>
  {/if}

  <div class="audio-setting-row">
    <label class="audio-setting-label">IN VOL</label>
    <input
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round(inputGain * 100)}
      on:input={(e) => onInputGainChange(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(inputGain * 100)}%</span>
  </div>

  <div class="audio-setting-row">
    <label class="audio-setting-label">OUT VOL</label>
    <input
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round(outputVolume * 100)}
      on:input={(e) => onOutputVolumeChange(parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(outputVolume * 100)}%</span>
  </div>

  {#if showSensitivity}
    <div class="audio-setting-row">
      <label class="audio-setting-label">SENS</label>
      <input
        type="range"
        class="audio-slider"
        min="0"
        max="100"
        value={vadSensitivity}
        on:input={(e) => onVadSensitivityChange(parseInt(e.currentTarget.value))}
      />
      <span class="audio-value">{vadSensitivity}</span>
    </div>
  {/if}

  <div class="audio-setting-row">
    <label class="audio-setting-label">NOISE</label>
    <button
      class="mode-btn {noiseSuppression ? 'active' : ''}"
      on:click={() => onNoiseSuppressionToggle(!noiseSuppression)}
    >
      {noiseSuppression ? "ON" : "OFF"}
    </button>
  </div>
</div>
