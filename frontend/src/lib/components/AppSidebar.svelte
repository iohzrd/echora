<script lang="ts">
  import { API, FRONTEND_VERSION } from '../api';
  import { goto } from '$app/navigation';
  import { activeServer } from '../serverManager';
  import AuthService, { user, isModerator } from '../auth';
  import { serverState } from '../stores/serverState';
  import { uiState } from '../stores/uiState';
  import { openAdminPanel, openPasskeySettings, openProfileModal, toggleSidebar } from '../actions/ui';
  import ChannelList from './ChannelList.svelte';
  import OnlineUsers from './OnlineUsers.svelte';
  import VoicePanel from './VoicePanel.svelte';
  import Avatar from './Avatar.svelte';

  let activeServerName = $derived($serverState.serverName || $activeServer?.name || 'Echora');
  let userAvatarUrl = $derived($user?.avatar_url ? API.getAvatarUrl($user.id) : undefined);
  let isMod = $derived(isModerator($user?.role));

  function logout() {
    AuthService.logout();
    goto('/auth');
  }
</script>

<div class="sidebar {$uiState.sidebarOpen ? 'open' : ''}">
  <div class="channels-area">
    <div class="server-header">
      <span class="server-name">{activeServerName}</span>
      <div class="header-actions">
        {#if isMod}
          <button
            class="header-icon-btn"
            on:click={openAdminPanel}
            title="Admin Panel"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
              ><path
                d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.53c.04-.32.07-.64.07-.97s-.03-.66-.07-1l2.11-1.63c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65A.49.49 0 0 0 14 2h-4c-.25 0-.46.18-.5.42l-.37 2.65c-.63.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64L4.57 11c-.04.34-.07.67-.07 1s.03.65.07.97l-2.11 1.66c-.19.15-.25.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1.01c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.25.42.5.42h4c.25 0 .46-.18.5-.42l.37-2.65c.63-.26 1.17-.59 1.69-.98l2.49 1.01c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.66z"
              /></svg
            >
          </button>
        {/if}
        <button
          class="header-icon-btn"
          on:click={openPasskeySettings}
          title="Manage passkeys"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M12.65 10a6 6 0 1 0-1.3 0L2 19.5V22h6v-2h2v-2h2l1.5-1.5L12.65 10zM15.5 4a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5z"
            /></svg
          >
        </button>
        <button
          class="header-icon-btn logout"
          on:click={logout}
          title="Logout"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M5 5h7V3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h7v-2H5V5zm16 7l-4-4v3H9v2h8v3l4-4z"
            /></svg
          >
        </button>
      </div>
    </div>

    <div class="channels-list">
      <ChannelList />
      <OnlineUsers />
    </div>

    <VoicePanel />

    <div class="user-bar">
      <button
        class="user-bar-profile"
        on:click={openProfileModal}
        title="Edit profile"
      >
        <Avatar
          username={$user?.username || ''}
          avatarUrl={userAvatarUrl}
          size="small"
        />
        <span class="user-bar-username">{$user?.username || ''}</span>
      </button>
    </div>

    <div class="version-bar">
      {#if $uiState.tauriVersion}
        <span class="version-info">app v{$uiState.tauriVersion}</span>
      {/if}
      <span class="version-info">frontend v{FRONTEND_VERSION}</span>
      <span class="version-info"
        >backend v{$serverState.backendVersion || '...'}</span
      >
    </div>
  </div>
</div>
