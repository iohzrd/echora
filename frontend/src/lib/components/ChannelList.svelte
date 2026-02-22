<script lang="ts">
  import type { Channel } from '../api';
  import { user, isAdminRole } from '../auth';
  import { voiceStore } from '../stores/voiceStore';
  import { serverState } from '../stores/serverState';
  import { chatState } from '../stores/chatState';
  import { viewUserProfile } from '../actions/ui';
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
            title="Edit">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>
          </button>
          <button
            class="channel-action-btn delete"
            on:click|stopPropagation={() => deleteChannel(channel.id)}
            title="Delete">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/></svg>
          </button>
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
            title="Edit">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>
          </button>
          <button
            class="channel-action-btn delete"
            on:click|stopPropagation={() => deleteChannel(channel.id)}
            title="Delete">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/></svg>
          </button>
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
            on:click={() => viewUserProfile(voiceState.user_id)}
            on:contextmenu|preventDefault={(e) => {
              if (voiceState.user_id !== currentUserId) {
                openUserVolumeMenu(e, voiceState.user_id, voiceState.username);
              }
            }}
            role="button"
            tabindex="0"
            on:keydown={(e) => e.key === 'Enter' && viewUserProfile(voiceState.user_id)}
          >
            <Avatar
              username={voiceState.username}
              avatarUrl={$serverState.userAvatars[voiceState.user_id]}
              size="xs"
            />
            <span class="username">{voiceState.username}</span>
            {#if voiceState.is_muted}
              <span class="mute-indicator" title="Muted">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M16.5 12A4.5 4.5 0 0 0 12 7.5v2.77l4.43 4.43c.04-.23.07-.46.07-.7zM19 12c0 .94-.2 1.82-.54 2.64l1.51 1.51A9.9 9.9 0 0 0 21 12c0-5.52-3.88-10.12-9-11.29V3a8 8 0 0 1 7 9zm-8.61-9.1L8 5.29 5.27 2.55 4 3.83l3 3V9a3 3 0 0 0 3 3h.29l1.5 1.5A4.5 4.5 0 0 1 7.5 12H5.57A8 8 0 0 0 11 20.71v2.29h2v-2.29A8 8 0 0 0 19.17 15l1.66 1.66 1.27-1.27L4 3.83z"/></svg>
              </span>
            {/if}
            {#if voiceState.is_deafened}
              <span class="deafen-indicator" title="Deafened">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1C8.86 1 6.35 3.25 6.04 6.17L12 12.13V7a2 2 0 1 1 4 0v.13l2.39 2.39A6 6 0 0 0 18 7V7C18 3.69 15.31 1 12 1zm-1 20v-2H9v-2h2v-2H9v-2h1.13L2.39 5.26 1.13 6.53l9 9A4.98 4.98 0 0 0 7 20v2h4zm2-2h4v-2h-4v2z"/></svg>
              </span>
            {/if}
            {#if voiceState.is_screen_sharing}
              <button
                class="screen-indicator"
                on:click|stopPropagation={() => {
                  if (voiceState.user_id !== currentUserId)
                    watchScreen(voiceState.user_id, voiceState.username);
                }}
                title="Watch screen">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>
              </button>
            {/if}
            {#if voiceState.is_camera_sharing}
              <button
                class="camera-indicator"
                on:click|stopPropagation={() => {
                  if (voiceState.user_id !== currentUserId)
                    watchCamera(voiceState.user_id, voiceState.username);
                }}
                title="Watch camera">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z"/></svg>
              </button>
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
