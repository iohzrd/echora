<script lang="ts">
  import { onMount } from "svelte";
  import AuthService, { type LoginRequest } from "../auth";
  import { PasskeyService } from "../passkey";

  export let onSuccess: () => void = () => {};

  let username = "";
  let password = "";
  let loading = false;
  let error = "";
  let passkeySupported = false;

  onMount(() => {
    passkeySupported = PasskeyService.isSupported();
  });

  async function handleSubmit() {
    if (!username.trim() || !password.trim()) {
      error = "Please fill in all fields";
      return;
    }

    loading = true;
    error = "";

    try {
      const loginData: LoginRequest = { username: username.trim(), password };
      await AuthService.login(loginData);
      onSuccess();
    } catch (err) {
      error = err instanceof Error ? err.message : "Login failed";
    } finally {
      loading = false;
    }
  }

  async function handlePasskeyLogin() {
    if (!username.trim()) {
      error = "Enter your username first, then click Sign in with Passkey";
      return;
    }
    loading = true;
    error = "";
    try {
      await AuthService.loginWithPasskey(username.trim());
      onSuccess();
    } catch (err) {
      if (err instanceof Error && err.name === "NotAllowedError") {
        error = "";
      } else {
        error = err instanceof Error ? err.message : "Passkey login failed";
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="auth-form">
  <h2>Welcome back!</h2>
  <p class="subtitle">We're so excited to see you again!</p>

  <form on:submit|preventDefault={handleSubmit}>
    <div class="form-group">
      <label for="username">Username</label>
      <input
        id="username"
        type="text"
        bind:value={username}
        placeholder="Enter your username"
        disabled={loading}
      />
    </div>

    <div class="form-group">
      <label for="password">Password</label>
      <input
        id="password"
        type="password"
        bind:value={password}
        placeholder="Enter your password"
        disabled={loading}
      />
    </div>

    {#if error}
      <div class="error-message">
        {error}
      </div>
    {/if}

    <button type="submit" disabled={loading} class="submit-btn">
      {loading ? "Logging in..." : "Log In"}
    </button>
  </form>

  {#if passkeySupported}
    <div class="passkey-divider">
      <span>or</span>
    </div>
    <div class="passkey-section">
      <p class="passkey-hint">Enter your username above, then:</p>
      <button
        type="button"
        disabled={loading || !username.trim()}
        class="passkey-btn"
        on:click={handlePasskeyLogin}
      >
        Sign in with Passkey
      </button>
    </div>
  {/if}
</div>

<style>
  .passkey-divider {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 32px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .passkey-divider::before,
  .passkey-divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border-color, #444);
  }

  .passkey-section {
    padding: 0 32px 24px;
  }

  .passkey-btn {
    width: 100%;
    padding: 10px;
    border: 1px solid var(--border-color, #444);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--text-primary, #fff);
    font-size: 14px;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .passkey-btn:hover:not(:disabled) {
    background: var(--bg-hover, rgba(255, 255, 255, 0.06));
  }

  .passkey-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .passkey-hint {
    color: var(--text-muted, #949ba4);
    font-size: 12px;
    margin: 0 0 8px;
    text-align: center;
  }
</style>
