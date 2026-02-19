<script lang="ts">
  import type { UserPresence } from "../api";
  import { getInitial } from "../utils";

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
        {getInitial(u.username)}
      </div>
      <span class="online-username">{u.username}</span>
    </div>
  {/each}
{/if}
