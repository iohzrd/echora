<script lang="ts">
  import { goto } from "$app/navigation";
  import LoginForm from "../../lib/components/LoginForm.svelte";
  import RegisterForm from "../../lib/components/RegisterForm.svelte";
  import { user } from "../../lib/auth";

  let isLogin = $state(true);

  $effect(() => {
    if ($user) {
      goto("/");
    }
  });

  function handleAuthSuccess() {
    goto("/");
  }

  function toggleMode() {
    isLogin = !isLogin;
  }
</script>

<svelte:head>
  <title>{isLogin ? "Login" : "Register"} - Echora</title>
</svelte:head>

<div class="auth-page">
  <div class="auth-container">
    <div class="auth-content">
      {#if isLogin}
        <LoginForm onSuccess={handleAuthSuccess} />
      {:else}
        <RegisterForm onSuccess={handleAuthSuccess} />
      {/if}

      <div class="auth-toggle">
        {#if isLogin}
          <span>Need an account?</span>
          <button onclick={toggleMode} class="toggle-btn">Register</button>
        {:else}
          <span>Already have an account?</span>
          <button onclick={toggleMode} class="toggle-btn">Login</button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .auth-page {
    min-height: 100vh;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .auth-container {
    width: 100%;
    max-width: 480px;
  }

  .auth-content {
    background-color: var(--bg-primary);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.24);
  }

  .auth-toggle {
    padding: 16px 32px;
    text-align: center;
    color: var(--text-muted);
    font-size: 14px;
  }

  .toggle-btn {
    background: none;
    border: none;
    color: var(--brand-primary);
    cursor: pointer;
    text-decoration: none;
    margin-left: 4px;
  }

  .toggle-btn:hover {
    text-decoration: underline;
  }
</style>
