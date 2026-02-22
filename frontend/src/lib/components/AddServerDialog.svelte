<script lang="ts">
  import { validateServerUrl } from "../serverManager";

  let { onAdd = () => {}, onCancel = () => {} }: {
    onAdd?: (url: string, name: string) => void;
    onCancel?: () => void;
  } = $props();

  let serverUrl = $state("");
  let serverName = $state("");
  let error = $state("");
  let validating = $state(false);

  async function handleSubmit() {
    if (!serverUrl.trim()) {
      error = "Server URL is required";
      return;
    }

    // Ensure protocol prefix
    let url = serverUrl.trim();
    if (!url.startsWith("http://") && !url.startsWith("https://")) {
      url = `https://${url}`;
    }

    validating = true;
    error = "";

    const result = await validateServerUrl(url);

    if (!result.valid) {
      error = result.error || "Could not connect to server";
      validating = false;
      return;
    }

    const resolvedUrl = result.resolvedUrl || url;
    const name =
      serverName.trim() || result.name || new URL(resolvedUrl).hostname;
    validating = false;
    onAdd(resolvedUrl, name);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      handleSubmit();
    } else if (event.key === "Escape") {
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="dialog-overlay" onclick={onCancel} role="presentation">
  <div
    class="dialog-content"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-label="Add Server"
    tabindex="-1"
  >
    <h2>Add Server</h2>
    <p class="dialog-subtitle">
      Enter the URL of an Echora instance to connect to it.
    </p>

    <div class="form-group">
      <label for="server-url">Server URL</label>
      <input
        id="server-url"
        type="text"
        placeholder="https://echora.example.com"
        bind:value={serverUrl}
        onkeydown={handleKeydown}
        disabled={validating}
      />
    </div>

    <div class="form-group">
      <label for="server-name">Display Name (optional)</label>
      <input
        id="server-name"
        type="text"
        placeholder="My Server"
        bind:value={serverName}
        onkeydown={handleKeydown}
        disabled={validating}
        maxlength="32"
      />
    </div>

    {#if error}
      <div class="dialog-error">{error}</div>
    {/if}

    <div class="dialog-actions">
      <button
        class="dialog-btn cancel"
        onclick={onCancel}
        disabled={validating}
      >
        Cancel
      </button>
      <button
        class="dialog-btn submit"
        onclick={handleSubmit}
        disabled={validating}
      >
        {validating ? "Connecting..." : "Add Server"}
      </button>
    </div>
  </div>
</div>
