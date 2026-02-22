<script lang="ts">
  import "../../app.css";
  import { onMount, onDestroy } from "svelte";
  import {
    isTauri,
    activeServer,
    servers,
    addServer,
    removeServer,
    setActiveServer,
    type EchoraServer,
  } from "../../lib/serverManager";
  import { getWs } from "../../lib/ws";
  import { voiceManager } from "../../lib/voice";
  import { voiceStore } from "../../lib/stores/voiceStore.svelte";
  import { serverState } from "../../lib/stores/serverState.svelte";
  import { uiState } from "../../lib/stores/uiState.svelte";
  import { connectToServer, initPTTSettings } from "../../lib/actions/server";
  import { initAudioSettings } from "../../lib/actions/audioSettings";
  import {
    openAddServerDialog,
    closeAddServerDialog,
    closeAdminPanel,
    closePasskeySettings,
    closeProfileModal,
    closeProfileView,
    closeSidebar,
    setNeedsServerAuth,
    setTauriAuthIsLogin,
  } from "../../lib/actions/ui";

  import ServerSidebar from "../../lib/components/ServerSidebar.svelte";
  import AddServerDialog from "../../lib/components/AddServerDialog.svelte";
  import LoginForm from "../../lib/components/LoginForm.svelte";
  import RegisterForm from "../../lib/components/RegisterForm.svelte";
  import AdminPanel from "../../lib/components/AdminPanel.svelte";
  import PasskeySettings from "../../lib/components/PasskeySettings.svelte";
  import ProfileModal from "../../lib/components/ProfileModal.svelte";
  import AppSidebar from "../../lib/components/AppSidebar.svelte";
  import ChatArea from "../../lib/components/ChatArea.svelte";
  import EmojiPicker from "../../lib/components/EmojiPicker.svelte";
  import { emojiPickerState } from "../../lib/stores/emojiPickerState.svelte";

  let { children } = $props();

  let activeServerName = $derived(
    serverState.serverName || $activeServer?.name || "Echora",
  );

  let _removeDeviceListener: (() => void) | null = null;

  onMount(async () => {
    await initPTTSettings();

    if (isTauri) {
      import("@tauri-apps/api/app")
        .then((m) => m.getVersion())
        .then((v) => {
          uiState.tauriVersion = v;
        })
        .catch(() => {});
    }

    _removeDeviceListener = await initAudioSettings();
    await connectToServer();
  });

  onDestroy(() => {
    getWs().disconnect();
    _removeDeviceListener?.();
  });

  async function handleSelectServer(server: EchoraServer) {
    if ($activeServer?.id === server.id) return;
    getWs().disconnect();
    const { currentVoiceChannel } = voiceStore;
    if (currentVoiceChannel) {
      await voiceManager.leaveVoiceChannel().catch(() => {});
    }
    setActiveServer(server.id);
    await connectToServer();
  }

  async function handleAddServer(url: string, name: string) {
    const server = addServer(url, name);
    closeAddServerDialog();
    setActiveServer(server.id);
    await connectToServer();
  }

  async function handleTauriAuthSuccess() {
    setNeedsServerAuth(false);
    setTauriAuthIsLogin(true);
    await connectToServer();
  }

  async function handleRemoveServer(id: string) {
    if (!confirm("Remove this server from your list?")) return;
    const wasActive = $activeServer?.id === id;
    removeServer(id);
    if (wasActive && $servers.length > 0) {
      await connectToServer();
    }
  }
</script>

<div class="layout">
  {#if uiState.sidebarOpen}
    <div
      class="sidebar-overlay"
      onclick={closeSidebar}
      role="presentation"
    ></div>
  {/if}

  {#if isTauri}
    <ServerSidebar
      onSelectServer={handleSelectServer}
      onAddServer={openAddServerDialog}
      onRemoveServer={handleRemoveServer}
    />
  {/if}

  {#if isTauri && !$activeServer}
    <div class="main-content tauri-empty-state">
      <div class="empty-state-message">
        <h2>Welcome to Echora</h2>
        <p>Add a server to get started.</p>
        <button class="submit-btn" onclick={openAddServerDialog}>
          Add Server
        </button>
      </div>
    </div>
  {:else if isTauri && uiState.needsServerAuth}
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
          {#if uiState.tauriAuthIsLogin}
            <LoginForm onSuccess={handleTauriAuthSuccess} />
          {:else}
            <RegisterForm onSuccess={handleTauriAuthSuccess} />
          {/if}
          <div class="auth-toggle">
            {#if uiState.tauriAuthIsLogin}
              <span>Need an account?</span>
              <button
                onclick={() => setTauriAuthIsLogin(false)}
                class="toggle-btn">Register</button
              >
            {:else}
              <span>Already have an account?</span>
              <button
                onclick={() => setTauriAuthIsLogin(true)}
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
    {@render children?.()}
  {/if}
</div>

{#if uiState.showAddServerDialog}
  <AddServerDialog onAdd={handleAddServer} onCancel={closeAddServerDialog} />
{/if}

{#if uiState.showAdminPanel}
  <AdminPanel onClose={closeAdminPanel} />
{/if}

{#if uiState.showPasskeySettings}
  <PasskeySettings onClose={closePasskeySettings} />
{/if}

{#if uiState.showProfileModal}
  <ProfileModal onClose={closeProfileModal} />
{/if}

{#if uiState.profileViewUserId}
  <ProfileModal
    viewUserId={uiState.profileViewUserId}
    onClose={closeProfileView}
  />
{/if}

{#if emojiPickerState.anchorRect && emojiPickerState.onSelect}
  <EmojiPicker
    anchorRect={emojiPickerState.anchorRect}
    onSelect={emojiPickerState.onSelect}
    customEmojis={serverState.customEmojis}
  />
{/if}
