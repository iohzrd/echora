<script lang="ts">
  import type { Channel } from '../api';
  import { user, isAdminRole } from '../auth';
  import { voiceStore } from '../stores/voiceStore';
  import { serverState } from '../stores/serverState';
  import { chatState } from '../stores/chatState';
  import { uiState } from '../stores/uiState';
  import { selectChannel, createChannel, updateChannel, deleteChannel } from '../actions/chat';
  import { joinVoice, watchScreen, watchCamera, getUserVolume } from '../actions/voice';
  import { changeUserVolume } from '../actions/audioSettings';
  import UserVolumeMenu from './UserVolumeMenu.svelte';
  import Avatar from './Avatar.svelte';

  // Local state for channel create/edit forms
  let showCreateChannel: 'text' | 'voice' | null = null;
  let newChannelName = '';
  let editingChannelId: string | null = null;
  let editChannelName = '';

  // Per-user volume menu state
  let volumeMenuUserId: string | null = null;
  let volumeMenuUsername = '';
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
    if (event.key === 'Enter') {
      event.preventDefault();
      onSubmit();
    } else if (event.key === 'Escape') {
      onCancel();
    }
  }

  function handleCreateKeydown(event: KeyboardEvent) {
    formKeydown(
      event,
      () => {
        if (newChannelName.trim() && showCreateChannel) {
          createChannel(newChannelName.trim(), showCreateChannel);
          newChannelName = '';
          showCreateChannel = null;
        }
      },
      () => {
        showCreateChannel = null;
        newChannelName = '';
      },
    );
  }

  function handleEditKeydown(event: KeyboardEvent) {
    formKeydown(
      event,
      () => {
        if (editingChannelId && editChannelName.trim()) {
          updateChannel(editingChannelId, editChannelName.trim());
          editingChannelId = null;
          editChannelName = '';
        }
      },
      () => {
        editingChannelId = null;
        editChannelName = '';
      },
    );
  }

  function startEdit(channel: Channel) {
    editingChannelId = channel.id;
    editChannelName = channel.name;
  }

  function toggleCreateForm(type: 'text' | 'voice') {
    showCreateChannel = showCreateChannel === type ? null : type;
  }

  let isAdmin = $derived(isAdminRole($user?.role));
  let currentUserId = $derived($user?.id ?? '');
  let textChannels = $derived($serverState.channels.filter((c) => c.channel_type === 'text'));
  let voiceChannels = $derived($serverState.channels.filter((c) => c.channel_type === 'voice'));
</script>

<div class="channel-category">
  <span>Text Channels</span>
  {#if isAdmin}
    <button
      class="create-channel-btn"
      on:click={() => toggleCreateForm('text')}
      title="Create Text Channel">+</button
    >
  {/if}
</div>
{#if showCreateChannel === 'text'}
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
{#each textChannels as channel}
  <div
    class="channel-item {$chatState.selectedChannelId === channel.id ? 'selected' : ''}"
    on:click={() => {
      if (editingChannelId !== channel.id)
        selectChannel(channel.id, channel.name);
    }}
    role="button"
    tabindex="0"
    on:keydown={(e) => e.key === 'Enter' && selectChannel(channel.id, channel.name)}
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
            on:click|stopPropagation={() => deleteChannel(channel.id)}
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
      on:click={() => toggleCreateForm('voice')}
      title="Create Voice Channel">+</button
    >
  {/if}
</div>
{#if showCreateChannel === 'voice'}
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
{#each voiceChannels as channel}
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
            on:click|stopPropagation={() => deleteChannel(channel.id)}
            title="Delete">X</button
          >
        </div>
      {/if}
      {#if $voiceStore.currentVoiceChannel !== channel.id}
        <button
          class="voice-btn join"
          on:click={() => joinVoice(channel.id)}
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

    {#if $voiceStore.voiceStates.some((vs) => vs.channel_id === channel.id)}
      <div class="voice-users">
        {#each $voiceStore.voiceStates.filter((vs) => vs.channel_id === channel.id) as voiceState}
          <div
            class="voice-user {$voiceStore.speakingUsers.includes(voiceState.user_id)
              ? 'speaking'
              : ''} {voiceState.is_screen_sharing
              ? 'screen-sharing'
              : ''} {voiceState.is_camera_sharing ? 'camera-sharing' : ''}"
            on:click={() => uiState.update((s) => ({ ...s, profileViewUserId: voiceState.user_id }))}
            on:contextmenu|preventDefault={(e) => {
              if (voiceState.user_id !== currentUserId) {
                openUserVolumeMenu(e, voiceState.user_id, voiceState.username);
              }
            }}
            role="button"
            tabindex="0"
            on:keydown={(e) => e.key === 'Enter' && uiState.update((s) => ({ ...s, profileViewUserId: voiceState.user_id }))}
          >
            <Avatar
              username={voiceState.username}
              avatarUrl={$serverState.userAvatars[voiceState.user_id]}
              size="xs"
            />
            <span class="username">{voiceState.username}</span>
            {#if voiceState.is_muted}
              <span class="mute-indicator">M</span>
            {/if}
            {#if voiceState.is_deafened}
              <span class="deafen-indicator">D</span>
            {/if}
            {#if voiceState.is_screen_sharing}
              <button
                class="screen-indicator"
                on:click|stopPropagation={() => {
                  if (voiceState.user_id !== currentUserId)
                    watchScreen(voiceState.user_id, voiceState.username);
                }}
                title="Watch screen"
              >S</button>
            {/if}
            {#if voiceState.is_camera_sharing}
              <button
                class="camera-indicator"
                on:click|stopPropagation={() => {
                  if (voiceState.user_id !== currentUserId)
                    watchCamera(voiceState.user_id, voiceState.username);
                }}
                title="Watch camera"
              >C</button>
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
    onVolumeChange={changeUserVolume}
    onClose={() => (volumeMenuUserId = null)}
  />
{/if}
