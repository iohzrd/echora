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

  let onlineUserIds = $derived(
    new Set(serverState.onlineUsers.map((u) => u.user_id)),
  );

  let onlineMembers = $derived(
    serverState.onlineUsers.map((u) => ({
      id: u.user_id,
      username: u.username,
      display_name: u.display_name,
      role: ROLE_INFO[serverState.userRolesMap[u.user_id]] ?? null,
    })),
  );

  let offlineMembers = $derived(
    serverState.members
      .filter((m) => !onlineUserIds.has(m.id))
      .map((m) => ({
        id: m.id,
        username: m.username,
        display_name: m.display_name,
        role: ROLE_INFO[m.role] ?? null,
      })),
  );
</script>

{#if uiState.membersSidebarOpen}
  <div class="members-sidebar">
    <div class="members-header">
      <span>Members -- {serverState.members.length}</span>
    </div>
    <div class="members-list">
      <div class="members-section-header">
        Online -- {onlineMembers.length}
      </div>
      {#each onlineMembers as u}
        <button class="member-entry" onclick={() => viewUserProfile(u.id)}>
          <div class="status-dot online"></div>
          <Avatar
            username={u.username}
            avatarUrl={serverState.userAvatars[u.id]}
            size="xs"
          />
          <span class="member-username">{u.display_name || u.username}</span>
          {#if u.role}
            <span class="role-badge {u.role.cls}">{u.role.badge}</span>
          {/if}
        </button>
      {/each}

      <div class="members-section-header">
        Offline -- {offlineMembers.length}
      </div>
      {#each offlineMembers as u}
        <button
          class="member-entry offline"
          onclick={() => viewUserProfile(u.id)}
        >
          <div class="status-dot"></div>
          <Avatar
            username={u.username}
            avatarUrl={serverState.userAvatars[u.id]}
            size="xs"
          />
          <span class="member-username">{u.display_name || u.username}</span>
          {#if u.role}
            <span class="role-badge {u.role.cls}">{u.role.badge}</span>
          {/if}
        </button>
      {/each}
    </div>
  </div>
{/if}
