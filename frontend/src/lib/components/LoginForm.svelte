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
</div>
