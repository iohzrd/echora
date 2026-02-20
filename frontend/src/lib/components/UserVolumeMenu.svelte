<script lang="ts">
  import { onMount } from "svelte";

  export let userId: string;
  export let username: string;
  export let volume: number = 1.0;
  export let x: number = 0;
  export let y: number = 0;
  export let onVolumeChange: (
    userId: string,
    volume: number,
  ) => void = () => {};
  export let onClose: () => void = () => {};

  let menuEl: HTMLDivElement;

  function handleClickOutside(e: MouseEvent) {
    if (menuEl && !menuEl.contains(e.target as Node)) {
      onClose();
    }
  }

  onMount(() => {
    // Delay adding click listener to avoid immediate close from the contextmenu event
    const timer = setTimeout(() => {
      document.addEventListener("click", handleClickOutside);
      document.addEventListener("contextmenu", handleClickOutside);
    }, 50);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("click", handleClickOutside);
      document.removeEventListener("contextmenu", handleClickOutside);
    };
  });
</script>

<div
  class="user-volume-menu"
  style="left: {x}px; top: {y}px"
  bind:this={menuEl}
>
  <div class="user-volume-header">{username}</div>
  <div class="user-volume-slider-row">
    <input
      type="range"
      class="audio-slider"
      min="0"
      max="200"
      value={Math.round(volume * 100)}
      on:input={(e) =>
        onVolumeChange(userId, parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(volume * 100)}%</span>
  </div>
</div>
