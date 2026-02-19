<script lang="ts">
  import type { UserPresence } from "../api";

  export let onlineUsers: UserPresence[] = [];
</script>

{#if onlineUsers.length > 0}
  <div class="channel-category">
    <span>Online -- {onlineUsers.length}</span>
  </div>
  {#each onlineUsers as u}
    <div class="online-user">
      <div class="online-dot"></div>
      <div class="user-avatar small">
        {u.username.charAt(0).toUpperCase()}
      </div>
      <span class="online-username">{u.username}</span>
    </div>
  {/each}
{/if}
