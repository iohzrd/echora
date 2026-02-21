<script lang="ts">
  import type { UserPresence } from "../api";
  import Avatar from "./Avatar.svelte";

  export let onlineUsers: UserPresence[] = [];
  export let userRoles: Record<string, string> = {};
  export let userAvatars: Record<string, string | undefined> = {};

  function getRoleBadge(userId: string): string {
    const role = userRoles[userId];
    if (role === "owner") return "OWN";
    if (role === "admin") return "ADM";
    if (role === "moderator") return "MOD";
    return "";
  }

  function getRoleClass(userId: string): string {
    const role = userRoles[userId];
    if (role === "owner") return "role-owner";
    if (role === "admin") return "role-admin";
    if (role === "moderator") return "role-mod";
    return "";
  }
</script>

{#if onlineUsers.length > 0}
  <div class="channel-category">
    <span>Online -- {onlineUsers.length}</span>
  </div>
  {#each onlineUsers as u}
    <div class="online-user">
      <div class="online-dot"></div>
      <Avatar
        username={u.username}
        avatarUrl={userAvatars[u.user_id]}
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
