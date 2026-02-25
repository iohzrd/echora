<script lang="ts">
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { isTauri } from "../lib/serverManager";

  let { children } = $props();

  const ZOOM_KEY = "echocell_zoom";
  const ZOOM_STEP = 0.1;
  const ZOOM_MIN = 0.5;
  const ZOOM_MAX = 2.0;

  onMount(() => {
    if (!isTauri) return;

    let currentZoom = 1;
    let webview: Awaited<
      ReturnType<
        (typeof import("@tauri-apps/api/webview"))["getCurrentWebview"]
      >
    >;

    async function init() {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      webview = getCurrentWebview();

      const saved = localStorage.getItem(ZOOM_KEY);
      if (saved) {
        const level = parseFloat(saved);
        if (isFinite(level) && level >= ZOOM_MIN && level <= ZOOM_MAX) {
          currentZoom = level;
          await webview.setZoom(currentZoom).catch(() => {});
        }
      }
    }

    // Mirror the native zoom hotkeys so we can persist the level.
    // zoomHotkeysEnabled handles the actual rendering; we just track the value.
    function handleKeydown(e: KeyboardEvent) {
      if (!webview || !(e.ctrlKey || e.metaKey)) return;
      let next: number | null = null;
      if (e.key === "=" || e.key === "+") {
        next = Math.min(
          ZOOM_MAX,
          Math.round((currentZoom + ZOOM_STEP) * 10) / 10,
        );
      } else if (e.key === "-") {
        next = Math.max(
          ZOOM_MIN,
          Math.round((currentZoom - ZOOM_STEP) * 10) / 10,
        );
      } else if (e.key === "0") {
        next = 1;
      }
      if (next !== null) {
        currentZoom = next;
        localStorage.setItem(ZOOM_KEY, String(currentZoom));
      }
    }

    document.addEventListener("keydown", handleKeydown);
    init();

    return () => {
      document.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

{@render children?.()}
