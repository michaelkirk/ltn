<script context="module" lang="ts">
  export type AppFocus = "cnt" | "global";
  const options: Record<AppFocus, string> = {
    global: "🌍 Global Projects",
    cnt: "🏴󠁧󠁢󠁳󠁣󠁴󠁿 Scotland Projects",
  };
</script>

<script lang="ts">
  export var selectedFocus: AppFocus;
</script>

<div class="focus-selector">
  <select bind:value={selectedFocus}>
    {#each Object.entries(options) as [value, label]}
      <option {value}>{label}</option>
    {/each}
  </select>
</div>

<style>
  .focus-selector {
    /* PICO WORKAROUND */
    padding-right: 4px;
    /* PICO WORKAROUND */
    display: inline-block;
  }

  .focus-selector select {
    /* PICO WORKAROUND */
    padding: 4px 40px 4px 12px;
  }
</style>
