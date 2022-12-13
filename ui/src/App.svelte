<script lang="ts">
  import TopAppBar, {
    Row,
    Section,
    Title,
    AutoAdjust,
  } from "@smui/top-app-bar";
  import Card, { Content } from "@smui/card";
  import Button, { Label as ButtonLabel } from "@smui/button";
  import List, {
    Item,
    Graphic,
    Meta,
    Text,
    PrimaryText,
    SecondaryText,
  } from "@smui/list";
  import "svelte-material-ui/bare.css";
  import Autocomplete from "@smui-extra/autocomplete";
  import LinearProgress from "@smui/linear-progress";
  import Snackbar, { Actions, Label as SnackbarLabel } from "@smui/snackbar";
  import IconButton from "@smui/icon-button";
  import {
    runSearch,
    findPaths,
    pageStore,
    type WPPage,
    type PagePaths,
  } from "./lib/wikipedia-speedrun";
  import { Timer } from "./lib/timer";

  // let sourcePage: WPPage|undefined = undefined;
  // let targetPage: WPPage|undefined = undefined;
  let sourcePageID: number | undefined = undefined;
  let targetPageID: number | undefined = undefined;
  let pathData: PagePaths;
  let snackbar: Snackbar;

  const searchTimer = new Timer(500);

  async function search(term: string): Promise<WPPage[]> {
    return new Promise((resolve, _reject) => {
      searchTimer.run(async () => {
        console.log("running search for", term);
        const results = await runSearch(term);
        resolve(results);
      });
    });
  }

  let searchCount = 0;

  async function autocomplete(term: string): Promise<number[] | false> {
    // if (
    //   (sourcePage && term === sourcePage.title) ||
    //   (targetPage && term === targetPage.title)
    // ) {
    //   console.debug("autocomplete attempted with same value:", term);
    //   return false;
    // }
    if (term === "") {
      console.debug("blank search term");
      return [];
    }
    const searchID = ++searchCount;

    console.log("searching for", term);
    const matches = await search(term);
    if (searchID !== searchCount) {
      return false;
    }
    console.log("matches", matches);
    return matches.map((p) => p.pageid);
  }

  async function computePaths() {
    if (!(sourcePageID && targetPageID)) {
      console.warn("tried to compute paths without pages set");
      return;
    }
    snackbar.forceOpen();
    pathData = await findPaths(sourcePageID, targetPageID);
    snackbar.close();
  }

  function getOptionLabel(option: number): string {
    if (!option) {
      return "";
    }
    return $pageStore.get(option)?.title;
  }

  $: console.log("source page:", sourcePageID);
</script>

<TopAppBar>
  <Row>
    <Section>
      <Title>Wikipedia Speedrun</Title>
    </Section>
  </Row>
</TopAppBar>

<main>
  <div class="page-inputs">
    <!--  -->
    <Autocomplete
      search={autocomplete}
      bind:value={sourcePageID}
      showMenuWithNoInput={false}
      {getOptionLabel}
      label="Source page"
    >
      <Text
        slot="loading"
        style="display: flex; width: 100%; justify-content: center; align-items: center;"
      >
        <LinearProgress style="height: 24px" indeterminate />
      </Text>
    </Autocomplete>

    <Autocomplete
      search={autocomplete}
      bind:value={targetPageID}
      showMenuWithNoInput={false}
      {getOptionLabel}
      label="Target page"
    >
      <Text
        slot="loading"
        style="display: flex; width: 100%; justify-content: center; align-items: center;"
      >
        <LinearProgress style="height: 24px" indeterminate />
      </Text>
    </Autocomplete>

    <Button
      on:click={computePaths}
      disabled={!(sourcePageID && targetPageID)}
      variant="raised"
    >
      <ButtonLabel>Compute paths</ButtonLabel>
    </Button>
  </div>

  {#if pathData}
    <div class="path-list">
      {#each pathData.paths as path}
        <Card class="path-card">
          <Content>
            <List twoLine avatarList>
              {#each path as page}
                <Item on:SMUI:action={() => window.open(page.link, "_blank")}>
                  {#if page.iconUrl}
                    <Graphic
                      style="background-image: url({page.iconUrl}); background-size: cover;"
                    />
                  {:else}
                    <Graphic />
                  {/if}
                  <Text>
                    <PrimaryText>{page.title}</PrimaryText>
                    <SecondaryText>{page.description}</SecondaryText>
                  </Text>
                </Item>
              {/each}
            </List>
          </Content>
        </Card>
      {/each}
    </div>
  {/if}

  <Snackbar bind:this={snackbar}>
    <SnackbarLabel>Finding paths!</SnackbarLabel>
    <Actions>
      <IconButton class="material-icons" title="Dismiss">close</IconButton>
    </Actions>
  </Snackbar>
</main>

<style>
  main {
    padding-top: 64px;
  }

  .page-inputs {
    display: flex;
    flex-direction: row;
    gap: 24px;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    margin-bottom: 16px;
  }

  .path-list {
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    justify-content: space-evenly;
  }

  /* shouldn't have to be global, but class on a component doesn't work */
  :global(.path-card) {
    margin-bottom: 10px;
    min-width: 386px;
    width: 30vw;
  }
</style>
