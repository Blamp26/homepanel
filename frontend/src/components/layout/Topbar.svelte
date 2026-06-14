<script lang="ts">
  export let title = 'HomePanel';
  export let status = 'connected';
  export let statusTone: 'neutral' | 'good' | 'warn' | 'bad' = 'good';
  export let onLogout: () => void;
</script>

<header class="app-header">
  <div>{title}</div>
  <div>
    <span class={statusTone}>{status}</span>
    <button type="button" on:click={onLogout}>Logout</button>
  </div>
</header>

<style>
  div {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .good {
    color: var(--success);
  }
  .warn {
    color: var(--warning);
  }
  .bad {
    color: var(--danger);
  }
  button {
    padding: 4px 8px;
  }
</style>
