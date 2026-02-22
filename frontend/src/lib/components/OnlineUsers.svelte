<script lang="ts">
  import { serverState } from "../stores/serverState.svelte";
  import { uiState } from "../stores/uiState.svelte";
  import { viewUserProfile } from "../actions/ui";
  import Avatar from "./Avatar.svelte";

  const ROLE_INFO: Record<string, { badge: string; cls: string }> = {
    owner: { badge: "OWN", cls: "role-owner" },
    admin: { badge: "ADM", cls: "role-admin" },
    moderator: { badge: "MOD", cls: "role-mod" },
  };

  let usersWithRole = $derived(
    serverState.onlineUsers.map((u) => ({
      ...u,
      role: ROLE_INFO[serverState.userRolesMap[u.user_id]] ?? null,
    })),
  );
</script>

{#if uiState.membersSidebarOpen}
  <div class="members-sidebar">
    <div class="members-header">
      <span>Members -- {usersWithRole.length}</span>
    </div>
    <div class="members-list">
      {#each usersWithRole as u}
        <button class="online-user" onclick={() => viewUserProfile(u.user_id)}>
          <div class="online-dot"></div>
          <Avatar
            username={u.username}
            avatarUrl={serverState.userAvatars[u.user_id]}
            size="xs"
          />
          <span class="online-username">{u.display_name || u.username}</span>
          {#if u.role}
            <span class="role-badge {u.role.cls}">{u.role.badge}</span>
          {/if}
        </button>
      {/each}
    </div>
  </div>
{/if}
