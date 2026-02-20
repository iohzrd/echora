<script lang="ts">
  export let username: string;
  export let onClose: () => void = () => {};

  export let videoElement: HTMLVideoElement;
</script>

<div class="screen-share-viewer">
  <div class="screen-share-header">
    <span class="screen-share-title">Watching {username}'s screen</span>
    <button class="screen-share-back-btn" on:click={onClose}
      >Back to chat</button
    >
  </div>
  <div class="screen-share-video-container">
    <video
      class="screen-share-video"
      bind:this={videoElement}
      autoplay
      playsinline
    ></video>
  </div>
</div>
