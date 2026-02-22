<script lang="ts">
  import { page } from '$app/stores';
  import { get } from 'svelte/store';
  import { serverState } from '../../../../lib/stores/serverState';
  import { chatState } from '../../../../lib/stores/chatState';
  import { selectChannel } from '../../../../lib/actions/chat';

  // Uses get() intentionally to bypass reactivity tracking -- this effect
  // should only re-run when the URL channelId changes, not when store state changes.
  $effect(() => {
    const channelId = $page.params.channelId;
    if (!channelId) return;
    const current = get(chatState);
    if (current.selectedChannelId === channelId) return;
    const channels = get(serverState).channels;
    const channel = channels.find((c) => c.id === channelId);
    if (channel) selectChannel(channel.id, channel.name);
  });
</script>
