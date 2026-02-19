<script lang="ts">
  export let floating: boolean = false;
  export let onSelect: (emoji: string) => void = () => {};

  const COMMON_EMOJI = [
    "\u{1F44D}", "\u{1F44E}", "\u{2764}\u{FE0F}", "\u{1F602}", "\u{1F622}", "\u{1F621}",
    "\u{1F389}", "\u{1F525}", "\u{1F44F}", "\u{1F914}",
    "\u{1F440}", "\u{1F680}", "\u{2705}", "\u{274C}", "\u{1F4AF}", "\u{1F60D}",
    "\u{1F631}", "\u{1F64F}", "\u{1F499}", "\u{1F49A}",
  ];
</script>

<div class="emoji-picker {floating ? 'emoji-picker-floating' : ''}">
  {#each COMMON_EMOJI as emoji}
    <button
      class="emoji-picker-btn"
      on:click={() => onSelect(emoji)}
    >{emoji}</button>
  {/each}
</div>
