<script lang="ts">
  import AuthService, { type LoginRequest } from "../auth";

  export let onSuccess: () => void = () => {};

  let username = "";
  let password = "";
  let loading = false;
  let error = "";

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

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      handleSubmit();
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
        on:keydown={handleKeydown}
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
        on:keydown={handleKeydown}
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
</div>

<style>
  .auth-form {
    background-color: #36393f;
    padding: 32px;
    border-radius: 8px;
    width: 100%;
    max-width: 480px;
    margin: 0 auto;
  }

  h2 {
    color: #ffffff;
    font-size: 24px;
    font-weight: 600;
    margin-bottom: 8px;
    text-align: center;
  }

  .subtitle {
    color: #b9bbbe;
    font-size: 16px;
    margin-bottom: 20px;
    text-align: center;
  }

  .form-group {
    margin-bottom: 20px;
  }

  label {
    display: block;
    color: #b9bbbe;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    margin-bottom: 8px;
    letter-spacing: 0.02em;
  }

  input {
    width: 100%;
    background-color: #202225;
    border: 1px solid #202225;
    border-radius: 3px;
    padding: 10px;
    font-size: 16px;
    color: #dcddde;
    transition: border-color 0.15s ease;
  }

  input:focus {
    outline: none;
    border-color: #5865f2;
  }

  input:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .error-message {
    color: #ed4245;
    font-size: 14px;
    margin-bottom: 20px;
    text-align: center;
  }

  .submit-btn {
    width: 100%;
    background-color: #5865f2;
    color: #ffffff;
    border: none;
    border-radius: 3px;
    padding: 12px;
    font-size: 16px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .submit-btn:hover:not(:disabled) {
    background-color: #4752c4;
  }

  .submit-btn:disabled {
    background-color: #4752c4;
    cursor: not-allowed;
  }
</style>