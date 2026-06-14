<script lang="ts">
  export let active = 'dashboard';
  export let onSelect: (page: string) => void;

  const items = [
    'dashboard',
    'terminals',
    'files',
    'services',
    'logs',
    'game servers',
    'settings',
  ];
</script>

<nav>
  {#each items as item (item)}
    <button
      type="button"
      class:active={active === item}
      on:click={() => onSelect(item)}
    >
      {item}
    </button>
  {/each}
</nav>

<style>
  nav {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  button {
    padding: 4px 8px;
    border: 1px solid #bbb;
    background: #f4f4f4;
  }
  button.active {
    font-weight: 700;
    background: #fff;
  }
</style>
