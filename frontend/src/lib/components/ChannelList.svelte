<script lang="ts">
  import type { Channel, VoiceState } from "../api";
  import UserVolumeMenu from "./UserVolumeMenu.svelte";
  import Avatar from "./Avatar.svelte";

  export let channels: Channel[] = [];
  export let selectedChannelId: string = "";
  export let currentVoiceChannel: string | null = null;
  export let voiceStates: VoiceState[] = [];
  export let speakingUsers: Set<string> = new Set();
  export let currentUserId: string = "";
  export let userRole: string = "member";

  export let onSelectChannel: (id: string, name: string) => void = () => {};
  export let onCreateChannel: (
    name: string,
    type: "text" | "voice",
  ) => void = () => {};
  export let onUpdateChannel: (id: string, name: string) => void = () => {};
  export let onDeleteChannel: (id: string) => void = () => {};
  export let onJoinVoice: (channelId: string) => void = () => {};
  export let onWatchScreen: (
    userId: string,
    username: string,
  ) => void = () => {};
  export let onWatchCamera: (
    userId: string,
    username: string,
  ) => void = () => {};
  export let onUserVolumeChange: (
    userId: string,
    volume: number,
  ) => void = () => {};
  export let getUserVolume: (userId: string) => number = () => 1.0;
  export let userAvatars: Record<string, string | undefined> = {};

  // Local state for channel create/edit forms
  let showCreateChannel: "text" | "voice" | null = null;
  let newChannelName = "";
  let editingChannelId: string | null = null;
  let editChannelName = "";

  // Per-user volume menu state
  let volumeMenuUserId: string | null = null;
  let volumeMenuUsername = "";
  let volumeMenuX = 0;
  let volumeMenuY = 0;

  function openUserVolumeMenu(e: MouseEvent, userId: string, username: string) {
    volumeMenuUserId = userId;
    volumeMenuUsername = username;
    volumeMenuX = e.clientX;
    volumeMenuY = e.clientY;
  }

  function formKeydown(
    event: KeyboardEvent,
    onSubmit: () => void,
    onCancel: () => void,
  ) {
    if (event.key === "Enter") {
      event.preventDefault();
      onSubmit();
    } else if (event.key === "Escape") {
      onCancel();
    }
  }

  function handleCreateKeydown(event: KeyboardEvent) {
    formKeydown(
      event,
      () => {
        if (newChannelName.trim() && showCreateChannel) {
          onCreateChannel(newChannelName.trim(), showCreateChannel);
          newChannelName = "";
          showCreateChannel = null;
        }
      },
      () => {
        showCreateChannel = null;
        newChannelName = "";
      },
    );
  }

  function handleEditKeydown(event: KeyboardEvent) {
    formKeydown(
      event,
      () => {
        if (editingChannelId && editChannelName.trim()) {
          onUpdateChannel(editingChannelId, editChannelName.trim());
          editingChannelId = null;
          editChannelName = "";
        }
      },
      () => {
        editingChannelId = null;
        editChannelName = "";
      },
    );
  }

  function startEdit(channel: Channel) {
    editingChannelId = channel.id;
    editChannelName = channel.name;
  }

  function toggleCreateForm(type: "text" | "voice") {
    showCreateChannel = showCreateChannel === type ? null : type;
  }

  $: isAdmin = userRole === "admin" || userRole === "owner";
</script>

<div class="channel-category">
  <span>Text Channels</span>
  {#if isAdmin}
    <button
      class="create-channel-btn"
      on:click={() => toggleCreateForm("text")}
      title="Create Text Channel">+</button
    >
  {/if}
</div>
{#if showCreateChannel === "text"}
  <div class="create-channel-form">
    <input
      type="text"
      class="create-channel-input"
      placeholder="channel-name"
      bind:value={newChannelName}
      on:keydown={handleCreateKeydown}
      maxlength="50"
    />
  </div>
{/if}
{#each channels.filter((c) => c.channel_type === "text") as channel}
  <div
    class="channel-item {selectedChannelId === channel.id ? 'selected' : ''}"
    on:click={() => {
      if (editingChannelId !== channel.id)
        onSelectChannel(channel.id, channel.name);
    }}
    role="button"
    tabindex="0"
    on:keydown={(e) =>
      e.key === "Enter" && onSelectChannel(channel.id, channel.name)}
  >
    <div class="channel-icon">#</div>
    {#if editingChannelId === channel.id}
      <input
        type="text"
        class="edit-channel-input"
        bind:value={editChannelName}
        on:keydown={handleEditKeydown}
        on:click|stopPropagation
        maxlength="50"
      />
    {:else}
      <span class="channel-name-text">{channel.name}</span>
      {#if isAdmin}
        <div class="channel-actions">
          <button
            class="channel-action-btn"
            on:click|stopPropagation={() => startEdit(channel)}
            title="Edit">E</button
          >
          <button
            class="channel-action-btn delete"
            on:click|stopPropagation={() => onDeleteChannel(channel.id)}
            title="Delete">X</button
          >
        </div>
      {/if}
    {/if}
  </div>
{/each}

<div class="channel-category">
  <span>Voice Channels</span>
  {#if isAdmin}
    <button
      class="create-channel-btn"
      on:click={() => toggleCreateForm("voice")}
      title="Create Voice Channel">+</button
    >
  {/if}
</div>
{#if showCreateChannel === "voice"}
  <div class="create-channel-form">
    <input
      type="text"
      class="create-channel-input"
      placeholder="channel-name"
      bind:value={newChannelName}
      on:keydown={handleCreateKeydown}
      maxlength="50"
    />
  </div>
{/if}
{#each channels.filter((c) => c.channel_type === "voice") as channel}
  <div class="channel-item voice-channel" role="button" tabindex="0">
    <div class="channel-header">
      <div class="channel-icon">#</div>
      <span class="channel-name">{channel.name}</span>
      {#if isAdmin}
        <div class="channel-actions voice-actions">
          <button
            class="channel-action-btn"
            on:click|stopPropagation={() => startEdit(channel)}
            title="Edit">E</button
          >
          <button
            class="channel-action-btn delete"
            on:click|stopPropagation={() => onDeleteChannel(channel.id)}
            title="Delete">X</button
          >
        </div>
      {/if}
      {#if currentVoiceChannel !== channel.id}
        <button
          class="voice-btn join"
          on:click={() => onJoinVoice(channel.id)}
          title="Join Voice Channel"
        >
          Join
        </button>
      {/if}
    </div>

    {#if editingChannelId === channel.id}
      <div class="create-channel-form">
        <input
          type="text"
          class="edit-channel-input"
          bind:value={editChannelName}
          on:keydown={handleEditKeydown}
          on:click|stopPropagation
          maxlength="50"
        />
      </div>
    {/if}

    {#if voiceStates.some((vs) => vs.channel_id === channel.id)}
      <div class="voice-users">
        {#each voiceStates.filter((vs) => vs.channel_id === channel.id) as voiceState}
          <div
            class="voice-user {speakingUsers.has(voiceState.user_id)
              ? 'speaking'
              : ''} {voiceState.is_screen_sharing
              ? 'screen-sharing'
              : ''} {voiceState.is_camera_sharing ? 'camera-sharing' : ''}"
            on:click={() => {
              if (voiceState.user_id !== currentUserId) {
                if (voiceState.is_screen_sharing)
                  onWatchScreen(voiceState.user_id, voiceState.username);
                else if (voiceState.is_camera_sharing)
                  onWatchCamera(voiceState.user_id, voiceState.username);
              }
            }}
            on:contextmenu|preventDefault={(e) => {
              if (voiceState.user_id !== currentUserId) {
                openUserVolumeMenu(e, voiceState.user_id, voiceState.username);
              }
            }}
            role={voiceState.is_screen_sharing || voiceState.is_camera_sharing
              ? "button"
              : "listitem"}
            tabindex={voiceState.is_screen_sharing ||
            voiceState.is_camera_sharing
              ? 0
              : -1}
          >
            <Avatar
              username={voiceState.username}
              avatarUrl={userAvatars[voiceState.user_id]}
              size="xs"
            />
            <span class="username">{voiceState.username}</span>
            {#if voiceState.is_muted}
              <span class="mute-indicator">🔇</span>
            {/if}
            {#if voiceState.is_deafened}
              <span class="deafen-indicator">🔇</span>
            {/if}
            {#if voiceState.is_screen_sharing}
              <span class="screen-indicator">🖥️</span>
            {/if}
            {#if voiceState.is_camera_sharing}
              <span class="camera-indicator">📷</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/each}

{#if volumeMenuUserId}
  <UserVolumeMenu
    userId={volumeMenuUserId}
    username={volumeMenuUsername}
    volume={getUserVolume(volumeMenuUserId)}
    x={volumeMenuX}
    y={volumeMenuY}
    onVolumeChange={onUserVolumeChange}
    onClose={() => (volumeMenuUserId = null)}
  />
{/if}
