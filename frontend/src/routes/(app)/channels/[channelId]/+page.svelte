<script lang="ts">
  import { page } from '$app/state';
  import { untrack } from 'svelte';
  import { serverState } from '../../../../lib/stores/serverState.svelte';
  import { chatState } from '../../../../lib/stores/chatState.svelte';
  import { selectChannel } from '../../../../lib/actions/chat';

  // Uses untrack() intentionally to bypass reactivity tracking for store reads --
  // this effect should only re-run when the URL channelId changes, not on every store update.
  $effect(() => {
    const channelId = page.params.channelId;
    if (!channelId) return;
    untrack(() => {
      if (chatState.selectedChannelId === channelId) return;
      const channel = serverState.channels.find((c) => c.id === channelId);
      if (channel) selectChannel(channel.id, channel.name);
    });
  });
</script>
