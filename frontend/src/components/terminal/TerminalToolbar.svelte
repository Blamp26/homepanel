<script lang="ts">
  export let onNew: () => void;
  export let onRefresh: () => void;
  export let onKill: () => void;
  export let onClear: () => void;
  export let canAct = true;
</script>

<div class="toolbar">
  <button type="button" on:click={onNew}>New terminal</button>
  <button type="button" on:click={onRefresh}>Refresh list</button>
  <button type="button" on:click={onKill} disabled={!canAct}>Kill</button>
  <button type="button" on:click={onClear} disabled={!canAct}>Clear</button>
</div>

<style>
  .toolbar {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  button {
    padding: 4px 8px;
    border: 1px solid #bbb;
    background: #f4f4f4;
  }
</style>
