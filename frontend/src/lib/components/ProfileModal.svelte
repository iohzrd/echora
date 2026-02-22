<script lang="ts">
  import { onMount } from "svelte";
  import { API, type PublicProfile } from "../api";
  import AuthService, { user } from "../auth";
  import Avatar from "./Avatar.svelte";

  let { onClose = () => {}, viewUserId = undefined }: {
    onClose?: () => void;
    viewUserId?: string;
  } = $props();

  // Self-edit state
  let displayName = $state("");
  let saving = $state(false);
  let uploading = $state(false);
  let error = $state("");
  let success = $state("");
  let fileInput: HTMLInputElement = $state()!;

  // Password change state
  let showPasswordSection = $state(false);
  let currentPassword = $state("");
  let newPassword = $state("");
  let confirmPassword = $state("");
  let passwordSaving = $state(false);
  let passwordError = $state("");
  let passwordSuccess = $state("");

  // View-other state
  let profile: PublicProfile | null = $state(null);
  let loading = $state(false);

  let isViewMode = $derived(!!viewUserId && viewUserId !== $user?.id);

  $effect(() => {
    if (!isViewMode && $user) {
      displayName = $user.display_name || "";
    }
  });

  onMount(async () => {
    if (isViewMode && viewUserId) {
      loading = true;
      try {
        profile = await API.getUserProfile(viewUserId);
      } catch (err) {
        error =
          err instanceof Error ? err.message : "Failed to load profile";
      } finally {
        loading = false;
      }
    }
  });

  async function handleSave() {
    if (!$user) return;
    saving = true;
    error = "";
    success = "";
    try {
      const currentDisplayName = $user.display_name || "";
      if (displayName === currentDisplayName) {
        success = "No changes to save.";
        saving = false;
        return;
      }

      const authResponse = await API.updateProfile({ display_name: displayName.trim() || null });
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

    if (
      !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(
        file.type,
      )
    ) {
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

  async function handleChangePassword() {
    passwordError = "";
    passwordSuccess = "";
    if (!currentPassword || !newPassword || !confirmPassword) {
      passwordError = "All fields are required.";
      return;
    }
    if (newPassword !== confirmPassword) {
      passwordError = "New passwords do not match.";
      return;
    }
    if (newPassword.length < 8) {
      passwordError = "Password must be at least 8 characters.";
      return;
    }
    passwordSaving = true;
    try {
      await API.changePassword(currentPassword, newPassword);
      passwordSuccess = "Password changed.";
      currentPassword = "";
      newPassword = "";
      confirmPassword = "";
      showPasswordSection = false;
    } catch (err) {
      passwordError = err instanceof Error ? err.message : "Failed to change password";
    } finally {
      passwordSaving = false;
    }
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  }

  function formatRole(role: string): string {
    return role.charAt(0).toUpperCase() + role.slice(1);
  }
</script>

<div
  class="profile-overlay"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
  onkeydown={(e) => e.key === "Escape" && onClose()}
  role="presentation"
>
  <div class="profile-panel">
    <div class="profile-header">
      <h2>
        {#if isViewMode}
          {profile?.display_name || profile?.username || "Profile"}
        {:else}
          Profile Settings
        {/if}
      </h2>
      <button class="close-btn" onclick={onClose}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>
      </button>
    </div>

    <div class="profile-content">
      {#if error}
        <div class="error-message">{error}</div>
      {/if}
      {#if success}
        <div class="success-message">{success}</div>
      {/if}

      {#if isViewMode}
        <!-- Read-only view of another user's profile -->
        {#if loading}
          <div class="loading">Loading profile...</div>
        {:else if profile}
          <div class="avatar-section">
            <Avatar
              username={profile.username}
              avatarUrl={profile.avatar_url
                ? API.getAvatarUrl(profile.id)
                : undefined}
              size="large"
            />
          </div>

          {#if profile.display_name}
            <div class="field">
              <label>Display Name</label>
              <div class="field-value">{profile.display_name}</div>
            </div>
          {/if}

          <div class="field">
            <label>Username</label>
            <div class="field-value">{profile.username}</div>
          </div>

          <div class="field">
            <label>Role</label>
            <div class="field-value role-badge {profile.role}">
              {formatRole(profile.role)}
            </div>
          </div>

          <div class="field">
            <label>Member Since</label>
            <div class="field-value">{formatDate(profile.created_at)}</div>
          </div>
        {/if}
      {:else}
        <!-- Editable self-profile -->
        <div class="avatar-section">
          <button
            class="avatar-upload-btn"
            onclick={() => fileInput.click()}
            disabled={uploading}
            title="Upload avatar"
          >
            <Avatar
              username={$user?.username || ""}
              avatarUrl={$user?.avatar_url
                ? API.getAvatarUrl($user.id)
                : undefined}
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
            onchange={handleAvatarUpload}
            class="hidden-input"
          />
          {#if $user?.avatar_url}
            <button
              class="remove-avatar-btn"
              onclick={handleDeleteAvatar}
              disabled={uploading}
            >
              Remove Avatar
            </button>
          {/if}
        </div>

        <div class="field">
          <label>Username</label>
          <div class="field-value">{$user?.username || ''}</div>
        </div>

        <div class="field">
          <label for="profile-displayname">Display Name</label>
          <input
            id="profile-displayname"
            type="text"
            bind:value={displayName}
            onkeydown={handleKeydown}
            disabled={saving}
            maxlength="64"
            placeholder="Optional"
          />
        </div>

        <div class="profile-actions">
          <button class="cancel-btn" onclick={onClose}>Cancel</button>
          <button class="save-btn" onclick={handleSave} disabled={saving}>
            {saving ? "Saving..." : "Save"}
          </button>
        </div>

        <div class="password-section">
          <button
            class="password-toggle-btn"
            onclick={() => {
              showPasswordSection = !showPasswordSection;
              passwordError = "";
              passwordSuccess = "";
            }}
          >
            {showPasswordSection ? "Cancel Password Change" : "Change Password"}
          </button>

          {#if showPasswordSection}
            {#if passwordError}
              <div class="error-message">{passwordError}</div>
            {/if}
            {#if passwordSuccess}
              <div class="success-message">{passwordSuccess}</div>
            {/if}
            <div class="field">
              <label for="current-password">Current Password</label>
              <input
                id="current-password"
                type="password"
                bind:value={currentPassword}
                disabled={passwordSaving}
                autocomplete="current-password"
              />
            </div>
            <div class="field">
              <label for="new-password">New Password</label>
              <input
                id="new-password"
                type="password"
                bind:value={newPassword}
                disabled={passwordSaving}
                autocomplete="new-password"
              />
            </div>
            <div class="field">
              <label for="confirm-password">Confirm New Password</label>
              <input
                id="confirm-password"
                type="password"
                bind:value={confirmPassword}
                disabled={passwordSaving}
                autocomplete="new-password"
              />
            </div>
            <div class="profile-actions">
              <button
                class="save-btn"
                onclick={handleChangePassword}
                disabled={passwordSaving}
              >
                {passwordSaving ? "Saving..." : "Update Password"}
              </button>
            </div>
          {/if}
        </div>
      {/if}
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

  .loading {
    text-align: center;
    color: var(--text-muted, #949ba4);
    padding: 20px;
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

  .field-value {
    font-size: 14px;
    color: var(--text-primary, #fff);
    padding: 8px 0;
  }

  .role-badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 12px;
    font-weight: 600;
  }

  .role-badge.owner {
    background: rgba(237, 66, 69, 0.2);
    color: #ed4245;
  }

  .role-badge.admin {
    background: rgba(235, 69, 158, 0.2);
    color: #eb459e;
  }

  .role-badge.moderator {
    background: rgba(88, 101, 242, 0.2);
    color: #5865f2;
  }

  .role-badge.member {
    background: rgba(148, 155, 164, 0.2);
    color: #949ba4;
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

  .password-section {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--border-color, #3f4147);
  }

  .password-toggle-btn {
    background: none;
    border: none;
    color: var(--text-muted, #949ba4);
    cursor: pointer;
    font-size: 13px;
    padding: 0;
    text-decoration: underline;
  }

  .password-toggle-btn:hover {
    color: var(--text-primary, #fff);
  }
</style>
