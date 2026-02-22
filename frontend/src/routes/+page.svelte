<script lang="ts">
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import {
    isTauri,
    activeServer,
    servers,
    addServer,
    removeServer,
    setActiveServer,
    type EchoraServer,
  } from '../lib/serverManager';
  import { getWs } from '../lib/ws';
  import { voiceManager } from '../lib/voice';
  import { voiceStore } from '../lib/stores/voiceStore';
  import { serverState } from '../lib/stores/serverState';
  import { uiState } from '../lib/stores/uiState';
  import { connectToServer, initPTTSettings } from '../lib/actions/server';
  import { initAudioSettings, removeDeviceListener } from '../lib/actions/audioSettings';

  import ServerSidebar from '../lib/components/ServerSidebar.svelte';
  import AddServerDialog from '../lib/components/AddServerDialog.svelte';
  import LoginForm from '../lib/components/LoginForm.svelte';
  import RegisterForm from '../lib/components/RegisterForm.svelte';
  import AdminPanel from '../lib/components/AdminPanel.svelte';
  import PasskeySettings from '../lib/components/PasskeySettings.svelte';
  import ProfileModal from '../lib/components/ProfileModal.svelte';
  import AppSidebar from '../lib/components/AppSidebar.svelte';
  import ChatArea from '../lib/components/ChatArea.svelte';

  $: activeServerName = $serverState.serverName || $activeServer?.name || 'Echora';

  onMount(async () => {
    await initPTTSettings();

    if (isTauri) {
      import('@tauri-apps/api/app')
        .then((m) => m.getVersion())
        .then((v) => serverState.update((s) => ({ ...s, tauriVersion: v })))
        .catch(() => {});
    }

    await initAudioSettings();
    await connectToServer();
  });

  onDestroy(() => {
    getWs().disconnect();
    removeDeviceListener?.();
  });

  async function handleSelectServer(server: EchoraServer) {
    if ($activeServer?.id === server.id) return;
    getWs().disconnect();
    const { currentVoiceChannel } = get(voiceStore);
    if (currentVoiceChannel) {
      await voiceManager.leaveVoiceChannel().catch(() => {});
    }
    setActiveServer(server.id);
    await connectToServer();
  }

  function handleAddServer(url: string, name: string) {
    const server = addServer(url, name);
    uiState.update((s) => ({ ...s, showAddServerDialog: false }));
    setActiveServer(server.id);
    connectToServer();
  }

  function handleTauriAuthSuccess() {
    uiState.update((s) => ({ ...s, needsServerAuth: false, tauriAuthIsLogin: true }));
    connectToServer();
  }

  function handleRemoveServer(id: string) {
    if (!confirm('Remove this server from your list?')) return;
    const wasActive = $activeServer?.id === id;
    removeServer(id);
    if (wasActive && $servers.length > 0) {
      connectToServer();
    }
  }
</script>

<div class="layout">
  {#if $uiState.sidebarOpen}
    <div
      class="sidebar-overlay"
      on:click={() => uiState.update((s) => ({ ...s, sidebarOpen: false }))}
      role="presentation"
    ></div>
  {/if}

  {#if isTauri}
    <ServerSidebar
      onSelectServer={handleSelectServer}
      onAddServer={() => uiState.update((s) => ({ ...s, showAddServerDialog: true }))}
      onRemoveServer={handleRemoveServer}
    />
  {/if}

  {#if isTauri && !$activeServer}
    <div class="main-content tauri-empty-state">
      <div class="empty-state-message">
        <h2>Welcome to Echora</h2>
        <p>Add a server to get started.</p>
        <button
          class="submit-btn"
          on:click={() => uiState.update((s) => ({ ...s, showAddServerDialog: true }))}
        >
          Add Server
        </button>
      </div>
    </div>
  {:else if isTauri && $uiState.needsServerAuth}
    <div class="sidebar">
      <div class="channels-area">
        <div class="server-header">
          <span>{activeServerName}</span>
        </div>
      </div>
    </div>
    <div class="main-content tauri-auth-state">
      <div class="auth-container">
        <div class="auth-content">
          {#if $uiState.tauriAuthIsLogin}
            <LoginForm onSuccess={handleTauriAuthSuccess} />
          {:else}
            <RegisterForm onSuccess={handleTauriAuthSuccess} />
          {/if}
          <div class="auth-toggle">
            {#if $uiState.tauriAuthIsLogin}
              <span>Need an account?</span>
              <button
                on:click={() => uiState.update((s) => ({ ...s, tauriAuthIsLogin: false }))}
                class="toggle-btn">Register</button
              >
            {:else}
              <span>Already have an account?</span>
              <button
                on:click={() => uiState.update((s) => ({ ...s, tauriAuthIsLogin: true }))}
                class="toggle-btn">Login</button
              >
            {/if}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <AppSidebar />
    <ChatArea />
  {/if}
</div>

{#if $uiState.showAddServerDialog}
  <AddServerDialog
    onAdd={handleAddServer}
    onCancel={() => uiState.update((s) => ({ ...s, showAddServerDialog: false }))}
  />
{/if}

{#if $uiState.showAdminPanel}
  <AdminPanel onClose={() => uiState.update((s) => ({ ...s, showAdminPanel: false }))} />
{/if}

{#if $uiState.showPasskeySettings}
  <PasskeySettings onClose={() => uiState.update((s) => ({ ...s, showPasskeySettings: false }))} />
{/if}

{#if $uiState.showProfileModal}
  <ProfileModal onClose={() => uiState.update((s) => ({ ...s, showProfileModal: false }))} />
{/if}

{#if $uiState.profileViewUserId}
  <ProfileModal
    viewUserId={$uiState.profileViewUserId}
    onClose={() => uiState.update((s) => ({ ...s, profileViewUserId: null }))}
  />
{/if}
