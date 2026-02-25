<script lang="ts">
  import { servers, activeServer, type EchoCellServer } from "../serverManager";
  import { getInitial } from "../utils";

  let {
    onSelectServer = () => {},
    onAddServer = () => {},
    onRemoveServer = () => {},
  }: {
    onSelectServer?: (server: EchoCellServer) => void;
    onAddServer?: () => void;
    onRemoveServer?: (id: string) => void;
  } = $props();

  let contextMenuServerId: string | null = $state(null);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);

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

  function handleCopyUrl(server: EchoCellServer) {
    closeContextMenu();
    navigator.clipboard.writeText(server.url);
  }
</script>

<svelte:window onclick={closeContextMenu} />

<div class="server-sidebar">
  {#each $servers as server (server.id)}
    <button
      class="server-sidebar-icon {$activeServer?.id === server.id
        ? 'active'
        : ''}"
      title="{server.name}{server.username ? ` (${server.username})` : ''}"
      onclick={() => onSelectServer(server)}
      oncontextmenu={(e) => handleContextMenu(e, server.id)}
    >
      {getInitial(server.name)}
    </button>
  {/each}

  <div class="server-sidebar-separator"></div>

  <button
    class="server-sidebar-icon add-server"
    title="Add Server"
    onclick={onAddServer}
  >
    +
  </button>
</div>

{#if contextMenuServerId}
  {@const server = $servers.find((s) => s.id === contextMenuServerId)}
  {#if server}
    <div
      class="server-context-menu"
      style="left: {contextMenuX}px; top: {contextMenuY}px;"
      role="menu"
    >
      <button
        class="context-menu-item"
        role="menuitem"
        onclick={() => handleCopyUrl(server)}
      >
        Copy URL
      </button>
      <button
        class="context-menu-item danger"
        role="menuitem"
        onclick={() => handleRemove(server.id)}
      >
        Remove Server
      </button>
    </div>
  {/if}
{/if}

<style>
  .server-sidebar {
    width: 72px;
    background-color: var(--bg-tertiary);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 0;
    gap: 8px;
    flex-shrink: 0;
    overflow-y: auto;
  }

  .server-sidebar-icon {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background-color: var(--bg-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-white);
    font-weight: bold;
    font-size: 18px;
    cursor: pointer;
    transition:
      border-radius 0.2s ease,
      background-color 0.2s ease;
    border: none;
    flex-shrink: 0;
  }

  .server-sidebar-icon:hover {
    border-radius: 16px;
    background-color: var(--brand-primary);
  }

  .server-sidebar-icon.active {
    border-radius: 16px;
    background-color: var(--brand-primary);
  }

  .server-sidebar-icon.add-server {
    background-color: transparent;
    border: 2px dashed var(--text-faint);
    color: var(--text-faint);
    font-size: 24px;
  }

  .server-sidebar-icon.add-server:hover {
    border-color: var(--status-positive);
    color: var(--status-positive);
    background-color: transparent;
  }

  .server-sidebar-separator {
    width: 32px;
    height: 2px;
    background-color: var(--border-primary);
    border-radius: 1px;
    flex-shrink: 0;
  }

  .server-context-menu {
    position: fixed;
    background-color: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: 4px;
    z-index: 200;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    min-width: 160px;
  }

  .context-menu-item {
    display: block;
    width: 100%;
    background: none;
    border: none;
    color: var(--text-normal);
    padding: 8px 12px;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }

  .context-menu-item:hover {
    background-color: var(--brand-primary);
    color: var(--text-white);
  }

  .context-menu-item.danger:hover {
    background-color: var(--status-negative);
  }
</style>
