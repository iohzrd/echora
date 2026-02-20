<script lang="ts">
  import type { Channel, VoiceState } from "../api";
  import type { VoiceInputMode } from "../voice";
  import { getInitial } from "../utils";
  import { formatKeyLabel, keyEventToTauriKey } from "../ptt";

  export let channels: Channel[] = [];
  export let selectedChannelId: string = "";
  export let currentVoiceChannel: string | null = null;
  export let voiceStates: VoiceState[] = [];
  export let speakingUsers: Set<string> = new Set();
  export let isMuted: boolean = false;
  export let isDeafened: boolean = false;
  export let isScreenSharing: boolean = false;
  export let currentUserId: string = "";
  export let voiceInputMode: VoiceInputMode = "voice-activity";
  export let pttKey: string = "Space";
  export let pttActive: boolean = false;

  export let onSelectChannel: (id: string, name: string) => void = () => {};
  export let onCreateChannel: (name: string, type: "text" | "voice") => void = () => {};
  export let onUpdateChannel: (id: string, name: string) => void = () => {};
  export let onDeleteChannel: (id: string) => void = () => {};
  export let onJoinVoice: (channelId: string) => void = () => {};
  export let onLeaveVoice: () => void = () => {};
  export let onToggleMute: () => void = () => {};
  export let onToggleDeafen: () => void = () => {};
  export let onToggleScreenShare: () => void = () => {};
  export let onWatchScreen: (userId: string, username: string) => void = () => {};
  export let onSwitchInputMode: (mode: VoiceInputMode) => void = () => {};
  export let onChangePTTKey: (key: string) => void = () => {};

  // Local state for channel create/edit forms
  let showCreateChannel: "text" | "voice" | null = null;
  let newChannelName = "";
  let editingChannelId: string | null = null;
  let editChannelName = "";

  // PTT key recording state
  let recordingKey = false;

  function handleKeyRecord(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    const key = keyEventToTauriKey(e);
    if (key) {
      recordingKey = false;
      onChangePTTKey(key);
    }
  }

  function formKeydown(event: KeyboardEvent, onSubmit: () => void, onCancel: () => void) {
    if (event.key === "Enter") {
      event.preventDefault();
      onSubmit();
    } else if (event.key === "Escape") {
      onCancel();
    }
  }

  function handleCreateKeydown(event: KeyboardEvent) {
    formKeydown(event, () => {
      if (newChannelName.trim() && showCreateChannel) {
        onCreateChannel(newChannelName.trim(), showCreateChannel);
        newChannelName = "";
        showCreateChannel = null;
      }
    }, () => {
      showCreateChannel = null;
      newChannelName = "";
    });
  }

  function handleEditKeydown(event: KeyboardEvent) {
    formKeydown(event, () => {
      if (editingChannelId && editChannelName.trim()) {
        onUpdateChannel(editingChannelId, editChannelName.trim());
        editingChannelId = null;
        editChannelName = "";
      }
    }, () => {
      editingChannelId = null;
      editChannelName = "";
    });
  }

  function startEdit(channel: Channel) {
    editingChannelId = channel.id;
    editChannelName = channel.name;
  }

  function toggleCreateForm(type: "text" | "voice") {
    showCreateChannel = showCreateChannel === type ? null : type;
  }
</script>

<div class="channel-category">
  <span>Text Channels</span>
  <button
    class="create-channel-btn"
    on:click={() => toggleCreateForm("text")}
    title="Create Text Channel">+</button
  >
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
  </div>
{/each}

<div class="channel-category">
  <span>Voice Channels</span>
  <button
    class="create-channel-btn"
    on:click={() => toggleCreateForm("voice")}
    title="Create Voice Channel">+</button
  >
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
      {#if currentVoiceChannel === channel.id}
        <button
          class="voice-btn leave"
          on:click={() => onLeaveVoice()}
          title="Leave Voice Channel"
        >
          Leave
        </button>
      {:else}
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

    {#if currentVoiceChannel === channel.id}
      <div class="voice-controls">
        <button
          class="voice-control-btn {isMuted ? 'active' : ''} {voiceInputMode === 'push-to-talk' && pttActive ? 'ptt-transmitting' : ''}"
          on:click={onToggleMute}
          title={voiceInputMode === 'push-to-talk'
            ? (pttActive ? 'Transmitting' : 'PTT: Hold ' + formatKeyLabel(pttKey))
            : (isMuted ? 'Unmute' : 'Mute')}
        >
          {#if voiceInputMode === 'push-to-talk'}
            {pttActive ? 'TX' : 'PTT'}
          {:else}
            {isMuted ? "MUTED" : "MIC"}
          {/if}
        </button>
        <button
          class="voice-control-btn {isDeafened ? 'active' : ''}"
          on:click={onToggleDeafen}
          title={isDeafened ? "Undeafen" : "Deafen"}
        >
          {isDeafened ? "DEAF" : "AUDIO"}
        </button>
        <button
          class="voice-control-btn screen {isScreenSharing ? 'active' : ''}"
          on:click={onToggleScreenShare}
          title={isScreenSharing ? "Stop Sharing" : "Share Screen"}
        >
          {isScreenSharing ? "SHARING" : "SCREEN"}
        </button>
      </div>

      <div class="voice-settings">
        <div class="voice-mode-toggle">
          <button
            class="mode-btn {voiceInputMode === 'voice-activity' ? 'active' : ''}"
            on:click={() => onSwitchInputMode('voice-activity')}
            title="Voice Activity Detection"
          >
            VAD
          </button>
          <button
            class="mode-btn {voiceInputMode === 'push-to-talk' ? 'active' : ''}"
            on:click={() => onSwitchInputMode('push-to-talk')}
            title="Push to Talk"
          >
            PTT
          </button>
        </div>

        {#if voiceInputMode === 'push-to-talk'}
          <div class="ptt-key-binding">
            {#if recordingKey}
              <button
                class="ptt-key-btn recording"
                on:keydown={handleKeyRecord}
                on:blur={() => (recordingKey = false)}
              >
                Press a key...
              </button>
            {:else}
              <button
                class="ptt-key-btn"
                on:click={() => (recordingKey = true)}
                title="Click to change PTT key"
              >
                {formatKeyLabel(pttKey)}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    {#if voiceStates.some((vs) => vs.channel_id === channel.id)}
      <div class="voice-users">
        {#each voiceStates.filter((vs) => vs.channel_id === channel.id) as voiceState}
          <div
            class="voice-user {speakingUsers.has(voiceState.user_id)
              ? 'speaking'
              : ''} {voiceState.is_screen_sharing ? 'screen-sharing' : ''}"
            on:click={() => {
              if (
                voiceState.is_screen_sharing &&
                voiceState.user_id !== currentUserId
              )
                onWatchScreen(voiceState.user_id, voiceState.username);
            }}
            role={voiceState.is_screen_sharing ? "button" : "listitem"}
            tabindex={voiceState.is_screen_sharing ? 0 : -1}
          >
            <div class="user-avatar">
              {getInitial(voiceState.username)}
            </div>
            <span class="username">{voiceState.username}</span>
            {#if voiceState.is_muted}
              <span class="mute-indicator">M</span>
            {/if}
            {#if voiceState.is_deafened}
              <span class="deafen-indicator">D</span>
            {/if}
            {#if voiceState.is_screen_sharing}
              <span class="screen-indicator">S</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/each}
