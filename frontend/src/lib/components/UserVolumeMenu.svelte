<script lang="ts">
  import { onMount } from "svelte";

  let {
    userId,
    username,
    volume = 1.0,
    x = 0,
    y = 0,
    onVolumeChange = () => {},
    onClose = () => {},
  }: {
    userId: string;
    username: string;
    volume?: number;
    x?: number;
    y?: number;
    onVolumeChange?: (userId: string, volume: number) => void;
    onClose?: () => void;
  } = $props();

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
      oninput={(e) =>
        onVolumeChange(userId, parseInt(e.currentTarget.value) / 100)}
    />
    <span class="audio-value">{Math.round(volume * 100)}%</span>
  </div>
</div>
