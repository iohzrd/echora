<script lang="ts">
  import AuthService, { type RegisterRequest } from "../auth";

  export let onSuccess: () => void = () => {};
  export let inviteRequired: boolean = false;

  let username = "";
  let email = "";
  let password = "";
  let inviteCode = "";
  let loading = false;
  let error = "";

  async function handleSubmit() {
    if (!username.trim() || !email.trim() || !password.trim()) {
      error = "Please fill in all fields";
      return;
    }

    loading = true;
    error = "";

    try {
      const registerData: RegisterRequest = {
        username: username.trim(),
        email: email.trim(),
        password,
        ...(inviteCode.trim() ? { invite_code: inviteCode.trim() } : {}),
      };
      await AuthService.register(registerData);
      onSuccess();
    } catch (err) {
      error = err instanceof Error ? err.message : "Registration failed";
    } finally {
      loading = false;
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
        minlength={2}
        maxlength={32}
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

    <div class="form-group">
      <label for="invite_code"
        >Invite Code{inviteRequired ? "" : " (optional)"}</label
      >
      <input
        id="invite_code"
        type="text"
        bind:value={inviteCode}
        placeholder={inviteRequired ? "Required" : "Enter invite code"}
        disabled={loading}
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
