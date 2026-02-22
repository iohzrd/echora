<script lang="ts">
  import { goto } from '$app/navigation';
  import { serverState } from '../../lib/stores/serverState';

  $effect(() => {
    const first = $serverState.channels.find((c) => c.channel_type === 'text');
    if (first) goto(`/channels/${first.id}`, { replaceState: true });
  });
</script>
