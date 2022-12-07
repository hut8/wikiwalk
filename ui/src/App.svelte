<script lang="ts">
  import TopAppBar, {
    Row,
    Section,
    Title,
    AutoAdjust,
  } from "@smui/top-app-bar";
  import { Text } from "@smui/list";
  import "svelte-material-ui/bare.css";
  import Autocomplete from "@smui-extra/autocomplete";
  import LinearProgress from "@smui/linear-progress";
  import IconButton from "@smui/icon-button";
  import { runSearch, paths, type WPPage } from "./lib/wikipedia-speedrun";
  import { Timer } from "./lib/timer";

  let sourcePage: WPPage;
  let targetPage: WPPage;

  const searchTimer = new Timer(500);

  async function search(term: string) : Promise<WPPage[]> {
    return new Promise((resolve, _reject) => {
      searchTimer.run(async () => {
        console.log("running search for", term);
        const results = await runSearch(term);
        resolve(results);
      });
    });


  }

  async function autocomplete(term: string): Promise<WPPage[]|false> {
    if (term === "") {
      console.debug("blank search term");
      return [];
    }
    console.log("searching for", term);
    return search(term);
  }
</script>

<TopAppBar>
  <Row>
    <Section>
      <IconButton class="material-icons">menu</IconButton>
      <Title>Wikipedia Speedrun</Title>
    </Section>
    <Section align="end" toolbar>
      <IconButton class="material-icons" aria-label="Download"
        >file_download</IconButton
      >
      <IconButton class="material-icons" aria-label="Print this page"
        >print</IconButton
      >
      <IconButton class="material-icons" aria-label="Bookmark this page"
        >bookmark</IconButton
      >
    </Section>
  </Row>
</TopAppBar>

<main>
  <Autocomplete
    search={autocomplete}
    bind:value={sourcePage}
    getOptionLabel={(option) =>
      option ? option.title : ''}
    showMenuWithNoInput={false}
    label="Source page"
  >
    <Text
      slot="loading"
      style="display: flex; width: 100%; justify-content: center; align-items: center;"
    >
      <LinearProgress style="height: 24px" indeterminate />
    </Text>
  </Autocomplete>
</main>

<style>
  main {
    padding-top: 64px;
  }
</style>
