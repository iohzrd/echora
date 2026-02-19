<script lang="ts">
  import { servers, activeServer, type EchoraServer } from "../serverManager";
  import { getInitial } from "../utils";

  export let onSelectServer: (server: EchoraServer) => void = () => {};
  export let onAddServer: () => void = () => {};
  export let onRemoveServer: (id: string) => void = () => {};

  let contextMenuServerId: string | null = null;
  let contextMenuX = 0;
  let contextMenuY = 0;

  function handleContextMenu(event: MouseEvent, serverId: string) {
    event.preventDefault();
    contextMenuServerId = serverId;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
  }

  function closeContextMenu() {
    contextMenuServerId = null;
  }

  function handleRemove(id: string) {
    closeContextMenu();
    onRemoveServer(id);
  }

  function handleCopyUrl(server: EchoraServer) {
    closeContextMenu();
    navigator.clipboard.writeText(server.url);
  }
</script>

<svelte:window on:click={closeContextMenu} />

<div class="server-sidebar">
  {#each $servers as server (server.id)}
    <button
      class="server-sidebar-icon {$activeServer?.id === server.id ? 'active' : ''}"
      title="{server.name}{server.username ? ` (${server.username})` : ''}"
      on:click={() => onSelectServer(server)}
      on:contextmenu={(e) => handleContextMenu(e, server.id)}
    >
      {getInitial(server.name)}
    </button>
  {/each}

  <div class="server-sidebar-separator"></div>

  <button
    class="server-sidebar-icon add-server"
    title="Add Server"
    on:click={onAddServer}
  >
    +
  </button>
</div>

{#if contextMenuServerId}
  {@const server = $servers.find(s => s.id === contextMenuServerId)}
  {#if server}
    <div
      class="server-context-menu"
      style="left: {contextMenuX}px; top: {contextMenuY}px;"
      role="menu"
    >
      <button
        class="context-menu-item"
        role="menuitem"
        on:click={() => handleCopyUrl(server)}
      >
        Copy URL
      </button>
      <button
        class="context-menu-item danger"
        role="menuitem"
        on:click={() => handleRemove(server.id)}
      >
        Remove Server
      </button>
    </div>
  {/if}
{/if}
