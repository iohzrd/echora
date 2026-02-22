<script lang="ts">
  import { onMount } from "svelte";
  import { PasskeyService, type PasskeyInfo } from "../passkey";

  let { onClose = () => {} }: { onClose?: () => void } = $props();

  let passkeys: PasskeyInfo[] = $state([]);
  let loading = $state(false);
  let error = $state("");
  let newPasskeyName = $state("");
  let registering = $state(false);
  let passkeySupported = $state(false);

  onMount(async () => {
    passkeySupported = PasskeyService.isSupported();
    if (passkeySupported) {
      await loadPasskeys();
    }
  });

  async function loadPasskeys() {
    loading = true;
    error = "";
    try {
      passkeys = await PasskeyService.listPasskeys();
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to load passkeys";
    } finally {
      loading = false;
    }
  }

  async function registerPasskey() {
    if (registering) return;
    registering = true;
    error = "";
    try {
      await PasskeyService.registerPasskey(newPasskeyName.trim() || undefined);
      newPasskeyName = "";
      await loadPasskeys();
    } catch (err) {
      if (err instanceof Error && err.name === "NotAllowedError") {
        // User cancelled the browser prompt
      } else {
        error =
          err instanceof Error ? err.message : "Failed to register passkey";
      }
    } finally {
      registering = false;
    }
  }

  async function deletePasskey(id: string, name: string) {
    if (
      !confirm(
        `Remove passkey "${name}"? You will no longer be able to sign in with it.`,
      )
    )
      return;
    error = "";
    try {
      await PasskeyService.deletePasskey(id);
      await loadPasskeys();
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to delete passkey";
    }
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }
</script>

<div
  class="passkey-overlay"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
  role="presentation"
>
  <div class="passkey-panel">
    <div class="passkey-header">
      <h2>Passkeys</h2>
      <button class="close-btn" onclick={onClose} title="Close">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
          /></svg
        >
      </button>
    </div>

    <div class="passkey-content">
      {#if !passkeySupported}
        <p class="unsupported-msg">
          Passkeys are not supported in this browser or environment.
        </p>
      {:else}
        {#if error}
          <div class="error-message">{error}</div>
        {/if}

        <div class="add-passkey">
          <input
            type="text"
            class="passkey-name-input"
            bind:value={newPasskeyName}
            placeholder="Passkey name (optional)"
            disabled={registering}
            onkeydown={(e) => {
              if (e.key === "Enter") registerPasskey();
            }}
          />
          <button
            class="add-btn"
            onclick={registerPasskey}
            disabled={registering}
          >
            {registering ? "Registering..." : "Add Passkey"}
          </button>
        </div>

        {#if loading}
          <p class="loading-msg">Loading...</p>
        {:else if passkeys.length === 0}
          <p class="empty-msg">
            No passkeys registered. Add one to enable passwordless login.
          </p>
        {:else}
          <div class="passkey-list">
            {#each passkeys as pk (pk.id)}
              <div class="passkey-item">
                <div class="passkey-info">
                  <span class="passkey-name">{pk.name}</span>
                  <span class="passkey-meta">
                    Added {formatDate(pk.created_at)}{pk.last_used_at
                      ? ` -- Last used ${formatDate(pk.last_used_at)}`
                      : " -- Never used"}
                  </span>
                </div>
                <button
                  class="delete-btn"
                  onclick={() => deletePasskey(pk.id, pk.name)}
                  title="Remove passkey"
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                  >
                    <path
                      d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                    />
                  </svg>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .passkey-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .passkey-panel {
    background: var(--bg-primary, #2b2d31);
    border-radius: var(--radius-lg, 8px);
    width: 100%;
    max-width: 480px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .passkey-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-color, #3f4147);
  }

  .passkey-header h2 {
    margin: 0;
    font-size: 18px;
    color: var(--text-primary, #fff);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted, #949ba4);
    cursor: pointer;
    font-size: 16px;
    padding: 4px 8px;
  }

  .close-btn:hover {
    color: var(--text-primary, #fff);
  }

  .passkey-content {
    padding: 16px 20px;
    overflow-y: auto;
  }

  .error-message {
    background: rgba(237, 66, 69, 0.15);
    color: #ed4245;
    padding: 10px 12px;
    border-radius: var(--radius-sm, 4px);
    font-size: 13px;
    margin-bottom: 12px;
  }

  .add-passkey {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }

  .passkey-name-input {
    flex: 1;
    padding: 8px 12px;
    border: 1px solid var(--border-color, #3f4147);
    border-radius: var(--radius-sm, 4px);
    background: var(--bg-secondary, #1e1f22);
    color: var(--text-primary, #fff);
    font-size: 14px;
  }

  .passkey-name-input:focus {
    outline: none;
    border-color: var(--brand-primary, #5865f2);
  }

  .add-btn {
    padding: 8px 16px;
    background: var(--brand-primary, #5865f2);
    color: #fff;
    border: none;
    border-radius: var(--radius-sm, 4px);
    cursor: pointer;
    font-size: 13px;
    white-space: nowrap;
  }

  .add-btn:hover:not(:disabled) {
    background: var(--brand-hover, #4752c4);
  }

  .add-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .loading-msg,
  .empty-msg,
  .unsupported-msg {
    color: var(--text-muted, #949ba4);
    font-size: 14px;
    text-align: center;
    padding: 20px 0;
  }

  .passkey-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .passkey-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    background: var(--bg-secondary, #1e1f22);
    border-radius: var(--radius-sm, 4px);
  }

  .passkey-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .passkey-name {
    color: var(--text-primary, #fff);
    font-size: 14px;
    font-weight: 500;
  }

  .passkey-meta {
    color: var(--text-muted, #949ba4);
    font-size: 12px;
  }

  .delete-btn {
    background: none;
    border: none;
    color: var(--text-muted, #949ba4);
    cursor: pointer;
    padding: 6px;
    border-radius: var(--radius-sm, 4px);
    flex-shrink: 0;
  }

  .delete-btn:hover {
    color: #ed4245;
    background: rgba(237, 66, 69, 0.1);
  }
</style>
