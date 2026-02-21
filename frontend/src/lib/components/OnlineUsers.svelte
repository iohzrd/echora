<script lang="ts">
  import { serverState } from '../stores/serverState';
  import { uiState } from '../stores/uiState';
  import Avatar from './Avatar.svelte';

  function getRoleBadge(userId: string): string {
    const role = $serverState.userRolesMap[userId];
    if (role === 'owner') return 'OWN';
    if (role === 'admin') return 'ADM';
    if (role === 'moderator') return 'MOD';
    return '';
  }

  function getRoleClass(userId: string): string {
    const role = $serverState.userRolesMap[userId];
    if (role === 'owner') return 'role-owner';
    if (role === 'admin') return 'role-admin';
    if (role === 'moderator') return 'role-mod';
    return '';
  }
</script>

{#if $serverState.onlineUsers.length > 0}
  <div class="channel-category">
    <span>Online -- {$serverState.onlineUsers.length}</span>
  </div>
  {#each $serverState.onlineUsers as u}
    <div
      class="online-user"
      on:click={() => uiState.update((s) => ({ ...s, profileViewUserId: u.user_id }))}
      role="button"
      tabindex="0"
      on:keydown={(e) => e.key === 'Enter' && uiState.update((s) => ({ ...s, profileViewUserId: u.user_id }))}
    >
      <div class="online-dot"></div>
      <Avatar
        username={u.username}
        avatarUrl={$serverState.userAvatars[u.user_id]}
        size="xs"
      />
      <span class="online-username">{u.username}</span>
      {#if getRoleBadge(u.user_id)}
        <span class="role-badge {getRoleClass(u.user_id)}"
          >{getRoleBadge(u.user_id)}</span
        >
      {/if}
    </div>
  {/each}
{/if}
