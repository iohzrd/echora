<script lang="ts">
  import { getInitial } from "../utils";

  let {
    username,
    avatarUrl = undefined,
    size = 'medium',
  }: {
    username: string;
    avatarUrl?: string;
    size?: 'xs' | 'small' | 'medium' | 'large';
  } = $props();

  let imgError = $state(false);

  let showImage = $derived(!!avatarUrl && !imgError);
  let sizeClass = $derived(`avatar-${size}`);

  function handleImgError() {
    imgError = true;
  }

  $effect(() => {
    if (avatarUrl) imgError = false;
  });
</script>

<div class="avatar {sizeClass}" class:has-image={showImage}>
  {#if showImage}
    <img src={avatarUrl} alt="{username}" onerror={handleImgError} />
  {:else}
    {getInitial(username)}
  {/if}
</div>

<style>
  .avatar {
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    color: #fff;
    overflow: hidden;
    flex-shrink: 0;
  }

  .avatar:not(.has-image) {
    background: var(--brand-primary);
  }

  .avatar.has-image {
    background: transparent;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .avatar-xs {
    width: 20px;
    height: 20px;
    font-size: 0.65rem;
  }

  .avatar-small {
    width: 24px;
    height: 24px;
    font-size: 0.7rem;
  }

  .avatar-medium {
    width: 40px;
    height: 40px;
    font-size: 1rem;
  }

  .avatar-large {
    width: 80px;
    height: 80px;
    font-size: 2rem;
  }
</style>
