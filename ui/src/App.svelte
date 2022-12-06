<script lang="ts">
  import { start_hydrating } from "svelte/internal";
  import { runSearch, paths } from "./lib/wikipedia-speedrun";

  enum PageRole {
    Source,
    Target,
  }

  async function autocomplete(e: Event, role: PageRole) {
    const target = e.currentTarget as HTMLInputElement;
    const term = target.value;
    const results = await runSearch(term);
  }


</script>

<main>
  <h1>Wikipedia Speedrun</h1>
  <div>
    <input
      type="search"
      placeholder="Source Page"
      on:input={(e) => autocomplete(e, PageRole.Source)}
    />
    →
    <input
      type="search"
      placeholder="Target Page"
      on:input={(e) => autocomplete(e, PageRole.Target)}
    />
    <input type="submit" />
  </div>

  {#if $paths.length}
  <h3>Paths</h3>
  {#each $paths as path}

  {/each}

  {/if}
</main>

<style>
</style>
