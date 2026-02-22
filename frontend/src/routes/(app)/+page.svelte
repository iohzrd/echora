<script lang="ts">
  import { goto } from "$app/navigation";
  import { serverState } from "../../lib/stores/serverState";

  let navigated = $state(false);

  $effect(() => {
    const first = $serverState.channels.find((c) => c.channel_type === "text");
    if (first && !navigated) {
      navigated = true;
      goto(`/channels/${first.id}`, { replaceState: true });
    }
  });
</script>
