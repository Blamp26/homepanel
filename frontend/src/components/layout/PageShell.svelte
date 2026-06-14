<script lang="ts">
  export let title = '';
  export let subtitle = '';
</script>

{#if title}
  <div>
    <h1>{title}</h1>
    {#if subtitle}
      <div>{subtitle}</div>
    {/if}
  </div>
{/if}

<slot />
