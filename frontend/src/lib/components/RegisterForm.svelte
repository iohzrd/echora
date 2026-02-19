<script lang="ts">
  import AuthService, { type RegisterRequest } from "../auth";

  export let onSuccess: () => void = () => {};

  let username = "";
  let email = "";
  let password = "";
  let loading = false;
  let error = "";

  async function handleSubmit() {
    if (!username.trim() || !email.trim() || !password.trim()) {
      error = "Please fill in all fields";
      return;
    }

    const trimmedUsername = username.trim();
    if (trimmedUsername.length > 32) {
      error = "Username must be 32 characters or fewer";
      return;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(trimmedUsername)) {
      error = "Username can only contain letters, numbers, underscores, and hyphens";
      return;
    }

    const trimmedEmail = email.trim().toLowerCase();
    if (!trimmedEmail.includes("@") || !trimmedEmail.includes(".")) {
      error = "Please enter a valid email address";
      return;
    }

    if (password.length < 8) {
      error = "Password must be at least 8 characters";
      return;
    }
    if (password.length > 128) {
      error = "Password must be 128 characters or fewer";
      return;
    }

    loading = true;
    error = "";

    try {
      const registerData: RegisterRequest = {
        username: username.trim(),
        email: email.trim(),
        password,
      };
      await AuthService.register(registerData);
      onSuccess();
    } catch (err) {
      error = err instanceof Error ? err.message : "Registration failed";
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
  <h2>Create an account</h2>

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
      <label for="email">Email</label>
      <input
        id="email"
        type="email"
        bind:value={email}
        placeholder="Enter your email"
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
      {loading ? "Creating Account..." : "Create Account"}
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