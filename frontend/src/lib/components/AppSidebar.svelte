<script lang="ts">
  import { API, FRONTEND_VERSION } from "../api";
  import type { VoiceInputMode } from "../voice";
  import { activeServer } from "../serverManager";
  import { user } from "../auth";
  import { voiceStore, audioSettingsStore } from "../stores/voiceStore";
  import { serverState } from "../stores/serverState";
  import { chatState } from "../stores/chatState";
  import ChannelList from "./ChannelList.svelte";
  import OnlineUsers from "./OnlineUsers.svelte";
  import VoicePanel from "./VoicePanel.svelte";
  import Avatar from "./Avatar.svelte";

  export let isMod: boolean = false;
  export let sidebarOpen: boolean = false;

  export let onShowAdminPanel: () => void = () => {};
  export let onShowPasskeySettings: () => void = () => {};
  export let onLogout: () => void = () => {};
  export let onShowProfileModal: () => void = () => {};
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
  export let getUserVolume: (userId: string) => number = () => 1;
  export let onUserClick: (userId: string) => void = () => {};
  export let onLeaveVoice: () => void = () => {};
  export let onToggleMute: () => void = () => {};
  export let onToggleDeafen: () => void = () => {};
  export let onToggleScreenShare: () => void = () => {};
  export let onToggleCamera: () => void = () => {};
  export let onSwitchInputMode: (mode: VoiceInputMode) => void = () => {};
  export let onChangePTTKey: (key: string) => void = () => {};
  export let onInputDeviceChange: (deviceId: string) => void = () => {};
  export let onOutputDeviceChange: (deviceId: string) => void = () => {};
  export let onInputGainChange: (gain: number) => void = () => {};
  export let onOutputVolumeChange: (volume: number) => void = () => {};
  export let onVadSensitivityChange: (sensitivity: number) => void = () => {};
  export let onNoiseSuppressionToggle: (enabled: boolean) => void = () => {};

  $: activeServerName =
    $serverState.serverName || $activeServer?.name || "Echora";
  $: userAvatarUrl = $user?.avatar_url ? API.getAvatarUrl($user.id) : undefined;
</script>

<div class="sidebar {sidebarOpen ? 'open' : ''}">
  <div class="channels-area">
    <div class="server-header">
      <span class="server-name">{activeServerName}</span>
      <div class="header-actions">
        {#if isMod}
          <button
            class="header-icon-btn"
            on:click={onShowAdminPanel}
            title="Admin Panel"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
              ><path
                d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.53c.04-.32.07-.64.07-.97s-.03-.66-.07-1l2.11-1.63c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65A.49.49 0 0 0 14 2h-4c-.25 0-.46.18-.5.42l-.37 2.65c-.63.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64L4.57 11c-.04.34-.07.67-.07 1s.03.65.07.97l-2.11 1.66c-.19.15-.25.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1.01c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.25.42.5.42h4c.25 0 .46-.18.5-.42l.37-2.65c.63-.26 1.17-.59 1.69-.98l2.49 1.01c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.66z"
              /></svg
            >
          </button>
        {/if}
        <button
          class="header-icon-btn"
          on:click={onShowPasskeySettings}
          title="Manage passkeys"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M12.65 10a6 6 0 1 0-1.3 0L2 19.5V22h6v-2h2v-2h2l1.5-1.5L12.65 10zM15.5 4a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5z"
            /></svg
          >
        </button>
        <button
          class="header-icon-btn logout"
          on:click={onLogout}
          title="Logout"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"
            ><path
              d="M5 5h7V3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h7v-2H5V5zm16 7l-4-4v3H9v2h8v3l4-4z"
            /></svg
          >
        </button>
      </div>
    </div>

    <div class="channels-list">
      <ChannelList
        channels={$serverState.channels}
        selectedChannelId={$chatState.selectedChannelId}
        currentVoiceChannel={$voiceStore.currentVoiceChannel}
        voiceStates={$voiceStore.voiceStates}
        speakingUsers={$voiceStore.speakingUsers}
        currentUserId={$user?.id || ""}
        userRole={$user?.role || "member"}
        {onSelectChannel}
        {onCreateChannel}
        {onUpdateChannel}
        {onDeleteChannel}
        {onJoinVoice}
        {onWatchScreen}
        {onWatchCamera}
        {onUserVolumeChange}
        {getUserVolume}
        userAvatars={$serverState.userAvatars}
        {onUserClick}
      />

      <OnlineUsers
        onlineUsers={$serverState.onlineUsers}
        userRoles={$serverState.userRolesMap}
        userAvatars={$serverState.userAvatars}
        {onUserClick}
      />
    </div>

    <VoicePanel
      currentVoiceChannel={$voiceStore.currentVoiceChannel}
      isMuted={$voiceStore.isMuted}
      isDeafened={$voiceStore.isDeafened}
      isScreenSharing={$voiceStore.isScreenSharing}
      isCameraSharing={$voiceStore.isCameraSharing}
      voiceInputMode={$voiceStore.voiceInputMode}
      pttKey={$voiceStore.pttKey}
      pttActive={$voiceStore.pttActive}
      inputDeviceId={$audioSettingsStore.inputDeviceId}
      outputDeviceId={$audioSettingsStore.outputDeviceId}
      inputGain={$audioSettingsStore.inputGain}
      outputVolume={$audioSettingsStore.outputVolume}
      vadSensitivity={$audioSettingsStore.vadSensitivity}
      noiseSuppression={$audioSettingsStore.noiseSuppression}
      inputDevices={$audioSettingsStore.inputDevices}
      outputDevices={$audioSettingsStore.outputDevices}
      {onLeaveVoice}
      {onToggleMute}
      {onToggleDeafen}
      {onToggleScreenShare}
      {onToggleCamera}
      {onSwitchInputMode}
      {onChangePTTKey}
      {onInputDeviceChange}
      {onOutputDeviceChange}
      {onInputGainChange}
      {onOutputVolumeChange}
      {onVadSensitivityChange}
      {onNoiseSuppressionToggle}
    />

    <div class="user-bar">
      <button
        class="user-bar-profile"
        on:click={onShowProfileModal}
        title="Edit profile"
      >
        <Avatar
          username={$user?.username || ""}
          avatarUrl={userAvatarUrl}
          size="small"
        />
        <span class="user-bar-username">{$user?.username || ""}</span>
      </button>
    </div>

    <div class="version-bar">
      {#if $serverState.tauriVersion}
        <span class="version-info">app v{$serverState.tauriVersion}</span>
      {/if}
      <span class="version-info">frontend v{FRONTEND_VERSION}</span>
      <span class="version-info"
        >backend v{$serverState.backendVersion || "..."}</span
      >
    </div>
  </div>
</div>
