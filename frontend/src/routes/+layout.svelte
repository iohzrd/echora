<script lang="ts">
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";

  let { children } = $props();

  const ZOOM_KEY = "echora_zoom";
  const ZOOM_STEP = 0.1;
  const ZOOM_MIN = 0.5;
  const ZOOM_MAX = 2.0;

  function getZoom(): number {
    if (!browser) return 1;
    const saved = localStorage.getItem(ZOOM_KEY);
    return saved ? parseFloat(saved) : 1;
  }

  function setZoom(level: number) {
    const clamped =
      Math.round(Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, level)) * 100) / 100;
    document.documentElement.style.zoom = String(clamped);
    localStorage.setItem(ZOOM_KEY, String(clamped));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!e.ctrlKey && !e.metaKey) return;
    if (e.key === "=" || e.key === "+") {
      e.preventDefault();
      setZoom(getZoom() + ZOOM_STEP);
    } else if (e.key === "-") {
      e.preventDefault();
      setZoom(getZoom() - ZOOM_STEP);
    } else if (e.key === "0") {
      e.preventDefault();
      setZoom(1);
    }
  }

  onMount(() => {
    setZoom(getZoom());
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

{@render children?.()}
