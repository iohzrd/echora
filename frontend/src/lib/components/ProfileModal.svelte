<script lang="ts">
  import { API } from "../api";
  import AuthService, { user } from "../auth";
  import Avatar from "./Avatar.svelte";

  export let onClose: () => void = () => {};

  let username = "";
  let displayName = "";
  let saving = false;
  let uploading = false;
  let error = "";
  let success = "";

  let fileInput: HTMLInputElement;

  $: if ($user) {
    username = $user.username;
    displayName = $user.display_name || "";
  }

  async function handleSave() {
    if (!$user) return;
    saving = true;
    error = "";
    success = "";
    try {
      const data: { username?: string; display_name?: string | null } = {};

      if (username !== $user.username) {
        data.username = username;
      }

      const currentDisplayName = $user.display_name || "";
      if (displayName !== currentDisplayName) {
        data.display_name = displayName.trim() || null;
      }

      if (Object.keys(data).length === 0) {
        success = "No changes to save.";
        saving = false;
        return;
      }

      const authResponse = await API.updateProfile(data);
      AuthService.setAuth(authResponse);
      success = "Profile updated.";
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to update profile";
    } finally {
      saving = false;
    }
  }

  async function handleAvatarUpload(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    if (file.size > 2 * 1024 * 1024) {
      error = "Avatar must be under 2MB.";
      return;
    }

    if (!["image/png", "image/jpeg", "image/gif", "image/webp"].includes(file.type)) {
      error = "Avatar must be PNG, JPEG, GIF, or WebP.";
      return;
    }

    uploading = true;
    error = "";
    success = "";
    try {
      const updatedUser = await API.uploadAvatar(file);
      user.set(updatedUser);
      success = "Avatar updated.";
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to upload avatar";
    } finally {
      uploading = false;
      target.value = "";
    }
  }

  async function handleDeleteAvatar() {
    if (!$user?.avatar_url) return;
    uploading = true;
    error = "";
    success = "";
    try {
      const updatedUser = await API.deleteAvatar();
      user.set(updatedUser);
      success = "Avatar removed.";
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to remove avatar";
    } finally {
      uploading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      handleSave();
    } else if (event.key === "Escape") {
      onClose();
    }
  }
</script>

<div class="profile-overlay" on:click|self={onClose} role="presentation">
  <div class="profile-panel">
    <div class="profile-header">
      <h2>Profile Settings</h2>
      <button class="close-btn" on:click={onClose}>X</button>
    </div>

    <div class="profile-content">
      {#if error}
        <div class="error-message">{error}</div>
      {/if}
      {#if success}
        <div class="success-message">{success}</div>
      {/if}

      <div class="avatar-section">
        <button
          class="avatar-upload-btn"
          on:click={() => fileInput.click()}
          disabled={uploading}
          title="Upload avatar"
        >
          <Avatar
            username={$user?.username || ""}
            avatarUrl={$user?.avatar_url ? API.getAvatarUrl($user.id) : undefined}
            size="large"
          />
          <div class="avatar-overlay">
            {#if uploading}
              ...
            {:else}
              Edit
            {/if}
          </div>
        </button>
        <input
          type="file"
          accept="image/png,image/jpeg,image/gif,image/webp"
          bind:this={fileInput}
          on:change={handleAvatarUpload}
          class="hidden-input"
        />
        {#if $user?.avatar_url}
          <button
            class="remove-avatar-btn"
            on:click={handleDeleteAvatar}
            disabled={uploading}
          >
            Remove Avatar
          </button>
        {/if}
      </div>

      <div class="field">
        <label for="profile-username">Username</label>
        <input
          id="profile-username"
          type="text"
          bind:value={username}
          on:keydown={handleKeydown}
          disabled={saving}
          maxlength="32"
        />
      </div>

      <div class="field">
        <label for="profile-displayname">Display Name</label>
        <input
          id="profile-displayname"
          type="text"
          bind:value={displayName}
          on:keydown={handleKeydown}
          disabled={saving}
          maxlength="64"
          placeholder="Optional"
        />
      </div>

      <div class="profile-actions">
        <button class="cancel-btn" on:click={onClose}>Cancel</button>
        <button class="save-btn" on:click={handleSave} disabled={saving}>
          {saving ? "Saving..." : "Save"}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .profile-overlay {
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

  .profile-panel {
    background: var(--bg-primary, #2b2d31);
    border-radius: var(--radius-lg, 8px);
    width: 100%;
    max-width: 440px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .profile-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-color, #3f4147);
  }

  .profile-header h2 {
    margin: 0;
    font-size: 18px;
    color: var(--text-primary, #fff);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted, #949ba4);
    cursor: pointer;
    font-size: 16px;
    padding: 4px 8px;
  }

  .close-btn:hover {
    color: var(--text-primary, #fff);
  }

  .profile-content {
    padding: 20px;
    overflow-y: auto;
  }

  .error-message {
    background: rgba(237, 66, 69, 0.15);
    color: #ed4245;
    padding: 10px 12px;
    border-radius: var(--radius-sm, 4px);
    font-size: 13px;
    margin-bottom: 12px;
  }

  .success-message {
    background: rgba(87, 242, 135, 0.15);
    color: #57f287;
    padding: 10px 12px;
    border-radius: var(--radius-sm, 4px);
    font-size: 13px;
    margin-bottom: 12px;
  }

  .avatar-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-bottom: 20px;
  }

  .avatar-upload-btn {
    position: relative;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    border-radius: 50%;
  }

  .avatar-overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .avatar-upload-btn:hover .avatar-overlay {
    opacity: 1;
  }

  .hidden-input {
    display: none;
  }

  .remove-avatar-btn {
    background: none;
    border: none;
    color: #ed4245;
    cursor: pointer;
    font-size: 12px;
    padding: 4px 8px;
  }

  .remove-avatar-btn:hover {
    text-decoration: underline;
  }

  .remove-avatar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .field {
    margin-bottom: 16px;
  }

  .field label {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-muted, #949ba4);
  }

  .field input {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--border-color, #3f4147);
    border-radius: var(--radius-sm, 4px);
    background: var(--bg-secondary, #1e1f22);
    color: var(--text-primary, #fff);
    font-size: 14px;
    box-sizing: border-box;
  }

  .field input:focus {
    outline: none;
    border-color: var(--brand-primary, #5865f2);
  }

  .field input:disabled {
    opacity: 0.5;
  }

  .profile-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .cancel-btn {
    padding: 8px 16px;
    background: none;
    border: none;
    color: var(--text-muted, #949ba4);
    cursor: pointer;
    font-size: 14px;
    border-radius: var(--radius-sm, 4px);
  }

  .cancel-btn:hover {
    color: var(--text-primary, #fff);
  }

  .save-btn {
    padding: 8px 20px;
    background: var(--brand-primary, #5865f2);
    color: #fff;
    border: none;
    border-radius: var(--radius-sm, 4px);
    cursor: pointer;
    font-size: 14px;
  }

  .save-btn:hover:not(:disabled) {
    background: var(--brand-hover, #4752c4);
  }

  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
