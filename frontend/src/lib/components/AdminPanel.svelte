<script lang="ts">
  import { onMount } from "svelte";
  import {
    API,
    type UserSummary,
    type Ban,
    type Mute,
    type Invite,
    type ModLogEntry,
  } from "../api";
  import { user } from "../auth";
  import { formatTimestamp } from "../utils";

  let { onClose = () => {} }: { onClose?: () => void } = $props();

  let activeTab: "users" | "moderation" | "invites" | "settings" | "modlog" =
    $state("users");
  let users: UserSummary[] = $state([]);
  let bans: Ban[] = $state([]);
  let mutes: Mute[] = $state([]);
  let invites: Invite[] = $state([]);
  let modLog: ModLogEntry[] = $state([]);
  let settings: Record<string, string> = $state({});

  let loading = $state(false);
  let error = $state("");

  // Form state
  let banUserId = $state("");
  let banReason = $state("");
  let banDuration = $state("");
  let muteUserId = $state("");
  let muteReason = $state("");
  let muteDuration = $state("");
  let kickUserId = $state("");
  let kickReason = $state("");
  let inviteMaxUses = $state("");
  let inviteExpiry = $state("");
  let lastCreatedInvite: Invite | null = $state(null);

  let userRole = $derived($user?.role ?? "member");
  let isAdmin = $derived(userRole === "admin" || userRole === "owner");

  function roleLevel(role: string): number {
    switch (role) {
      case "owner":
        return 3;
      case "admin":
        return 2;
      case "moderator":
        return 1;
      default:
        return 0;
    }
  }

  function canModerate(targetRole: string): boolean {
    return roleLevel(userRole) > roleLevel(targetRole);
  }

  function assignableRoles(): string[] {
    const roles: string[] = [];
    if (roleLevel(userRole) > 0) roles.push("member");
    if (roleLevel(userRole) > 1) roles.push("moderator");
    if (roleLevel(userRole) > 2) roles.push("admin");
    return roles;
  }

  let loadSeq = 0;
  async function loadTab(tab: string) {
    const seq = ++loadSeq;
    loading = true;
    error = "";
    try {
      switch (tab) {
        case "users":
          users = await API.getUsers();
          break;
        case "moderation":
          [bans, mutes] = await Promise.all([API.getBans(), API.getMutes()]);
          if (users.length === 0) users = await API.getUsers();
          break;
        case "invites":
          invites = await API.getInvites();
          break;
        case "settings":
          settings = await API.getSettings();
          break;
        case "modlog":
          modLog = await API.getModLog();
          if (users.length === 0) users = await API.getUsers();
          break;
      }
    } catch (err) {
      if (seq !== loadSeq) return;
      error = err instanceof Error ? err.message : "Failed to load data";
    } finally {
      if (seq === loadSeq) loading = false;
    }
  }

  function switchTab(tab: typeof activeTab) {
    activeTab = tab;
    loadTab(tab);
  }

  function getUsernameById(id: string): string {
    return users.find((u) => u.id === id)?.username || id.slice(0, 8);
  }

  async function handleChangeRole(userId: string, newRole: string) {
    try {
      await API.changeUserRole(userId, newRole);
      await loadTab("users");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to change role";
    }
  }

  async function handleKick() {
    if (!kickUserId) return;
    try {
      await API.kickUser(kickUserId, kickReason || undefined);
      kickUserId = "";
      kickReason = "";
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to kick user";
    }
  }

  async function handleBan() {
    if (!banUserId) return;
    try {
      const duration = banDuration ? parseInt(banDuration) : undefined;
      await API.banUser(banUserId, banReason || undefined, duration);
      banUserId = "";
      banReason = "";
      banDuration = "";
      await loadTab("moderation");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to ban user";
    }
  }

  async function handleUnban(userId: string) {
    try {
      await API.unbanUser(userId);
      await loadTab("moderation");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to unban user";
    }
  }

  async function handleMute() {
    if (!muteUserId) return;
    try {
      const duration = muteDuration ? parseInt(muteDuration) : undefined;
      await API.muteUser(muteUserId, muteReason || undefined, duration);
      muteUserId = "";
      muteReason = "";
      muteDuration = "";
      await loadTab("moderation");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to mute user";
    }
  }

  async function handleUnmute(userId: string) {
    try {
      await API.unmuteUser(userId);
      await loadTab("moderation");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to unmute user";
    }
  }

  async function handleCreateInvite() {
    try {
      const maxUses = inviteMaxUses ? parseInt(inviteMaxUses) : undefined;
      const expiry = inviteExpiry ? parseInt(inviteExpiry) : undefined;
      lastCreatedInvite = await API.createInvite(maxUses, expiry);
      inviteMaxUses = "";
      inviteExpiry = "";
      await loadTab("invites");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to create invite";
    }
  }

  async function handleRevokeInvite(inviteId: string) {
    try {
      await API.revokeInvite(inviteId);
      await loadTab("invites");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to revoke invite";
    }
  }

  async function handleDeleteUser(userId: string, username: string) {
    if (
      !confirm(`Permanently delete user "${username}"? This cannot be undone.`)
    )
      return;
    try {
      await API.deleteUser(userId);
      await loadTab("users");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to delete user";
    }
  }

  async function handleUpdateSetting(key: string, value: string) {
    try {
      await API.updateSetting(key, value);
      await loadTab("settings");
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to update setting";
    }
  }

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
  }

  onMount(() => {
    loadTab(activeTab);
  });
</script>

<div
  class="admin-overlay"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
  role="presentation"
>
  <div class="admin-panel">
    <div class="admin-header">
      <h2>Server Administration</h2>
      <button class="admin-close-btn" onclick={onClose} title="Close">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"
          ><path
            d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
          /></svg
        >
      </button>
    </div>

    <div class="admin-tabs">
      <button
        class="admin-tab {activeTab === 'users' ? 'active' : ''}"
        onclick={() => switchTab("users")}>Users</button
      >
      <button
        class="admin-tab {activeTab === 'moderation' ? 'active' : ''}"
        onclick={() => switchTab("moderation")}>Moderation</button
      >
      <button
        class="admin-tab {activeTab === 'invites' ? 'active' : ''}"
        onclick={() => switchTab("invites")}>Invites</button
      >
      {#if isAdmin}
        <button
          class="admin-tab {activeTab === 'settings' ? 'active' : ''}"
          onclick={() => switchTab("settings")}>Settings</button
        >
      {/if}
      <button
        class="admin-tab {activeTab === 'modlog' ? 'active' : ''}"
        onclick={() => switchTab("modlog")}>Mod Log</button
      >
    </div>

    {#if error}
      <div class="admin-error">{error}</div>
    {/if}

    <div class="admin-content">
      {#if loading}
        <div class="admin-loading">Loading...</div>
      {:else if activeTab === "users"}
        <div class="admin-table-wrap">
          <table class="admin-table">
            <thead>
              <tr>
                <th>Username</th>
                <th>Role</th>
                <th>Joined</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each users as u}
                <tr>
                  <td>{u.username}</td>
                  <td>
                    <span class="role-badge role-{u.role}">{u.role}</span>
                  </td>
                  <td>{formatTimestamp(u.created_at)}</td>
                  <td class="user-actions-cell">
                    {#if isAdmin && canModerate(u.role) && u.id !== $user?.id}
                      <select
                        value={u.role}
                        onchange={(e) =>
                          handleChangeRole(u.id, e.currentTarget.value)}
                        class="role-select"
                      >
                        {#each assignableRoles() as role}
                          <option value={role} selected={u.role === role}
                            >{role}</option
                          >
                        {/each}
                      </select>
                    {/if}
                    {#if userRole === "owner" && u.role !== "owner" && u.id !== $user?.id}
                      <button
                        class="mod-action-btn delete-user"
                        onclick={() => handleDeleteUser(u.id, u.username)}
                        >Delete</button
                      >
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else if activeTab === "moderation"}
        <div class="mod-section">
          <h3>Quick Actions</h3>
          <div class="mod-forms">
            <div class="mod-form">
              <h4>Kick User</h4>
              <select bind:value={kickUserId} class="mod-input">
                <option value="">Select user...</option>
                {#each users.filter((u) => canModerate(u.role) && u.id !== $user?.id) as u}
                  <option value={u.id}>{u.username}</option>
                {/each}
              </select>
              <input
                type="text"
                bind:value={kickReason}
                placeholder="Reason (optional)"
                class="mod-input"
              />
              <button
                class="mod-action-btn kick"
                onclick={handleKick}
                disabled={!kickUserId}>Kick</button
              >
            </div>

            <div class="mod-form">
              <h4>Ban User</h4>
              <select bind:value={banUserId} class="mod-input">
                <option value="">Select user...</option>
                {#each users.filter((u) => canModerate(u.role) && u.id !== $user?.id) as u}
                  <option value={u.id}>{u.username}</option>
                {/each}
              </select>
              <input
                type="text"
                bind:value={banReason}
                placeholder="Reason (optional)"
                class="mod-input"
              />
              <input
                type="number"
                bind:value={banDuration}
                placeholder="Duration in hours (empty = permanent)"
                class="mod-input"
              />
              <button
                class="mod-action-btn ban"
                onclick={handleBan}
                disabled={!banUserId}>Ban</button
              >
            </div>

            <div class="mod-form">
              <h4>Mute User</h4>
              <select bind:value={muteUserId} class="mod-input">
                <option value="">Select user...</option>
                {#each users.filter((u) => canModerate(u.role) && u.id !== $user?.id) as u}
                  <option value={u.id}>{u.username}</option>
                {/each}
              </select>
              <input
                type="text"
                bind:value={muteReason}
                placeholder="Reason (optional)"
                class="mod-input"
              />
              <input
                type="number"
                bind:value={muteDuration}
                placeholder="Duration in hours (empty = permanent)"
                class="mod-input"
              />
              <button
                class="mod-action-btn mute"
                onclick={handleMute}
                disabled={!muteUserId}>Mute</button
              >
            </div>
          </div>
        </div>

        {#if bans.length > 0}
          <div class="mod-section">
            <h3>Active Bans</h3>
            <div class="mod-list">
              {#each bans as ban}
                <div class="mod-list-item">
                  <div class="mod-list-info">
                    <strong>{getUsernameById(ban.user_id)}</strong>
                    {#if ban.reason}<span class="mod-reason"
                        >- {ban.reason}</span
                      >{/if}
                    {#if ban.expires_at}<span class="mod-expiry"
                        >Expires: {formatTimestamp(ban.expires_at)}</span
                      >{:else}<span class="mod-expiry">Permanent</span>{/if}
                  </div>
                  <button
                    class="mod-action-btn unban"
                    onclick={() => handleUnban(ban.user_id)}>Unban</button
                  >
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mutes.length > 0}
          <div class="mod-section">
            <h3>Active Mutes</h3>
            <div class="mod-list">
              {#each mutes as mute}
                <div class="mod-list-item">
                  <div class="mod-list-info">
                    <strong>{getUsernameById(mute.user_id)}</strong>
                    {#if mute.reason}<span class="mod-reason"
                        >- {mute.reason}</span
                      >{/if}
                    {#if mute.expires_at}<span class="mod-expiry"
                        >Expires: {formatTimestamp(mute.expires_at)}</span
                      >{:else}<span class="mod-expiry">Permanent</span>{/if}
                  </div>
                  <button
                    class="mod-action-btn unmute"
                    onclick={() => handleUnmute(mute.user_id)}>Unmute</button
                  >
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else if activeTab === "invites"}
        <div class="mod-section">
          <h3>Create Invite</h3>
          <div class="invite-form">
            <input
              type="number"
              bind:value={inviteMaxUses}
              placeholder="Max uses (empty = unlimited)"
              class="mod-input"
            />
            <input
              type="number"
              bind:value={inviteExpiry}
              placeholder="Expires in hours (empty = never)"
              class="mod-input"
            />
            <button class="mod-action-btn create" onclick={handleCreateInvite}
              >Create Invite</button
            >
          </div>
          {#if lastCreatedInvite}
            <div class="invite-created">
              <span class="invite-code">{lastCreatedInvite.code}</span>
              <button
                class="copy-btn"
                onclick={() =>
                  lastCreatedInvite && copyToClipboard(lastCreatedInvite.code)}
                >Copy</button
              >
            </div>
          {/if}
        </div>

        {#if invites.length > 0}
          <div class="mod-section">
            <h3>All Invites</h3>
            <div class="admin-table-wrap">
              <table class="admin-table">
                <thead>
                  <tr>
                    <th>Code</th>
                    <th>Uses</th>
                    <th>Expires</th>
                    <th>Status</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {#each invites as invite}
                    <tr class={invite.revoked ? "revoked" : ""}>
                      <td><span class="invite-code">{invite.code}</span></td>
                      <td
                        >{invite.uses}{invite.max_uses
                          ? `/${invite.max_uses}`
                          : ""}</td
                      >
                      <td
                        >{invite.expires_at
                          ? formatTimestamp(invite.expires_at)
                          : "Never"}</td
                      >
                      <td>{invite.revoked ? "Revoked" : "Active"}</td>
                      <td>
                        {#if !invite.revoked}
                          <button
                            class="mod-action-btn revoke"
                            onclick={() => handleRevokeInvite(invite.id)}
                            >Revoke</button
                          >
                        {/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        {/if}
      {:else if activeTab === "settings"}
        <div class="mod-section">
          <h3>Registration Mode</h3>
          <div class="setting-row">
            <label>
              <input
                type="radio"
                name="reg_mode"
                value="open"
                checked={settings.registration_mode === "open"}
                onchange={() =>
                  handleUpdateSetting("registration_mode", "open")}
              />
              Open Registration
            </label>
            <label>
              <input
                type="radio"
                name="reg_mode"
                value="invite_only"
                checked={settings.registration_mode === "invite_only"}
                onchange={() =>
                  handleUpdateSetting("registration_mode", "invite_only")}
              />
              Invite Only
            </label>
          </div>
        </div>
      {:else if activeTab === "modlog"}
        <div class="admin-table-wrap">
          <table class="admin-table">
            <thead>
              <tr>
                <th>Action</th>
                <th>Moderator</th>
                <th>Target</th>
                <th>Reason</th>
                <th>Date</th>
              </tr>
            </thead>
            <tbody>
              {#each modLog as entry}
                <tr>
                  <td
                    ><span class="mod-action-label {entry.action}"
                      >{entry.action}</span
                    ></td
                  >
                  <td>{getUsernameById(entry.moderator_id)}</td>
                  <td>{getUsernameById(entry.target_user_id)}</td>
                  <td>{entry.reason || "-"}</td>
                  <td>{formatTimestamp(entry.created_at)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .admin-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .admin-panel {
    background: var(--bg-secondary);
    border-radius: 8px;
    width: 90%;
    max-width: 900px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .admin-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--bg-tertiary);
  }

  .admin-header h2 {
    margin: 0;
    color: var(--text-normal);
    font-size: 18px;
  }

  .admin-close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    cursor: pointer;
    padding: 4px 8px;
  }

  .admin-close-btn:hover {
    color: var(--text-normal);
  }

  .admin-tabs {
    display: flex;
    border-bottom: 1px solid var(--bg-tertiary);
    padding: 0 20px;
  }

  .admin-tab {
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 10px 16px;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    font-size: 14px;
  }

  .admin-tab:hover {
    color: var(--text-normal);
  }

  .admin-tab.active {
    color: var(--text-normal);
    border-bottom-color: var(--brand-primary);
  }

  .admin-error {
    color: var(--status-negative);
    padding: 8px 20px;
    font-size: 13px;
  }

  .admin-content {
    padding: 20px;
    overflow-y: auto;
    flex: 1;
  }

  .admin-loading {
    color: var(--text-muted);
    text-align: center;
    padding: 40px;
  }

  .admin-table-wrap {
    overflow-x: auto;
  }

  .admin-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .admin-table th,
  .admin-table td {
    padding: 8px 12px;
    text-align: left;
    border-bottom: 1px solid var(--bg-tertiary);
  }

  .admin-table th {
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    font-size: 11px;
  }

  .admin-table td {
    color: var(--text-normal);
  }

  .admin-table tr.revoked td {
    opacity: 0.5;
  }

  .role-badge {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .role-owner {
    background: #f0b232;
    color: #000;
  }
  .role-admin {
    background: #5865f2;
    color: #fff;
  }
  .role-moderator,
  .role-mod {
    background: #43b581;
    color: #fff;
  }
  .role-member {
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .role-select {
    background: var(--bg-tertiary);
    color: var(--text-normal);
    border: 1px solid var(--bg-primary);
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
  }

  .mod-section {
    margin-bottom: 24px;
  }

  .mod-section h3 {
    color: var(--text-normal);
    font-size: 15px;
    margin: 0 0 12px;
  }

  .mod-section h4 {
    color: var(--text-muted);
    font-size: 13px;
    margin: 0 0 8px;
  }

  .mod-forms {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 16px;
  }

  .mod-form {
    background: var(--bg-tertiary);
    padding: 12px;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .mod-input {
    background: var(--bg-primary);
    color: var(--text-normal);
    border: 1px solid var(--bg-secondary);
    padding: 6px 10px;
    border-radius: 4px;
    font-size: 13px;
  }

  .mod-input::placeholder {
    color: var(--text-faint);
  }

  .mod-action-btn {
    padding: 6px 12px;
    border: none;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    color: #fff;
  }

  .mod-action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .mod-action-btn.kick {
    background: #faa61a;
  }
  .mod-action-btn.ban {
    background: var(--status-negative);
  }
  .mod-action-btn.mute {
    background: #747f8d;
  }
  .mod-action-btn.unban,
  .mod-action-btn.unmute {
    background: var(--status-positive);
  }
  .mod-action-btn.create {
    background: var(--brand-primary);
  }
  .mod-action-btn.revoke {
    background: var(--status-negative);
  }
  .mod-action-btn.delete-user {
    background: #8b0000;
  }
  .user-actions-cell {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mod-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .mod-list-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--bg-tertiary);
    padding: 8px 12px;
    border-radius: 4px;
  }

  .mod-list-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text-normal);
    font-size: 13px;
  }

  .mod-reason {
    color: var(--text-muted);
  }

  .mod-expiry {
    color: var(--text-faint);
    font-size: 12px;
  }

  .invite-form {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .invite-created {
    margin-top: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-tertiary);
    padding: 8px 12px;
    border-radius: 4px;
  }

  .invite-code {
    font-family: "Consolas", "Monaco", monospace;
    color: var(--brand-primary);
    font-weight: 600;
    font-size: 14px;
  }

  .copy-btn {
    background: var(--bg-primary);
    color: var(--text-muted);
    border: 1px solid var(--bg-secondary);
    padding: 4px 8px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 12px;
  }

  .copy-btn:hover {
    color: var(--text-normal);
  }

  .setting-row {
    display: flex;
    gap: 20px;
  }

  .setting-row label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-normal);
    cursor: pointer;
    font-size: 14px;
  }

  .mod-action-label {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .mod-action-label.kick {
    background: #faa61a33;
    color: #faa61a;
  }
  .mod-action-label.ban {
    background: #f0474733;
    color: var(--status-negative);
  }
  .mod-action-label.unban {
    background: #43b58133;
    color: var(--status-positive);
  }
  .mod-action-label.mute {
    background: #747f8d33;
    color: #747f8d;
  }
  .mod-action-label.unmute {
    background: #43b58133;
    color: var(--status-positive);
  }
  .mod-action-label.role_change {
    background: #5865f233;
    color: var(--brand-primary);
  }
</style>
