<script lang="ts">
  import { serverState } from '../stores/serverState';
  import { viewUserProfile } from '../actions/ui';
  import Avatar from './Avatar.svelte';

  const ROLE_INFO: Record<string, { badge: string; cls: string }> = {
    owner:     { badge: 'OWN', cls: 'role-owner' },
    admin:     { badge: 'ADM', cls: 'role-admin' },
    moderator: { badge: 'MOD', cls: 'role-mod' },
  };

  let usersWithRole = $derived($serverState.onlineUsers.map((u) => ({
    ...u,
    role: ROLE_INFO[$serverState.userRolesMap[u.user_id]] ?? null,
  })));
</script>

{#if usersWithRole.length > 0}
  <div class="channel-category">
    <span>Online -- {usersWithRole.length}</span>
  </div>
  {#each usersWithRole as u}
    <div
      class="online-user"
      on:click={() => viewUserProfile(u.user_id)}
      role="button"
      tabindex="0"
      on:keydown={(e) => e.key === 'Enter' && viewUserProfile(u.user_id)}
    >
      <div class="online-dot"></div>
      <Avatar
        username={u.username}
        avatarUrl={$serverState.userAvatars[u.user_id]}
        size="xs"
      />
      <span class="online-username">{u.username}</span>
      {#if u.role}
        <span class="role-badge {u.role.cls}">{u.role.badge}</span>
      {/if}
    </div>
  {/each}
{/if}
