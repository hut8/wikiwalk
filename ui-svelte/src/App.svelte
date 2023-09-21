<script lang="ts">
  import TopAppBar, {
    Row,
    Section,
    Title,
  } from "@smui/top-app-bar";
  import Card, { Content } from "@smui/card";
  import Button, { Label as ButtonLabel } from "@smui/button";
  import Banner, { Label as BannerLabel } from "@smui/banner";
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
  } from "./lib/wikiwalk";
  import Activity from "./components/Activity.svelte";
  import { Timer } from "./lib/timer";

  enum PageState {
    Pristine,
    Loading,
    Complete
  };

  let sourcePageID: number | undefined = undefined;
  let targetPageID: number | undefined = undefined;
  let pathData: PagePaths|null = null;
  let errorSnackbar: Snackbar;

  const searchTimer = new Timer(500);

  async function search(term: string): Promise<WPPage[]> {
    return new Promise((resolve, _reject) => {
      searchTimer.run(async () => {
        const results = await runSearch(term);
        resolve(results);
      });
    });
  }

  let loading = false;
  let elapsed: number|null = null;

  async function autocomplete(term: string): Promise<number[] | false> {
    if (term === "") {
      console.debug("blank search term");
      return [];
    }
    const matches = await search(term);
    return matches.map((p) => p.pageid);
  }

  async function computePaths() {
    if (!(sourcePageID && targetPageID)) {
      console.warn("tried to compute paths without pages set");
      return;
    }
    const instant = new Date().getTime();
    try {
      loading = true;
      pathData = await findPaths(sourcePageID, targetPageID);
    } catch (e: any) {
      errorSnackbar.forceOpen();
    } finally {
      loading = false;
      elapsed = (new Date().getTime()) - instant;
    }
  }

  function getOptionLabel(option: number): string {
    if (!option) {
      return "";
    }
    return $pageStore.get(option)?.title;
  }
</script>

<TopAppBar variant="static">
  <Row>
    <Section>
      <Title>WikiWalk</Title>
    </Section>
  </Row>
</TopAppBar>

<div class="parameter-container">
  <Autocomplete
    style="min-width: 350px; width: 30vw;"
    textfield$style="width: 100%;"
    textfield$helperLine$style="width: 100%;"
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
    style="min-width: 350px; width: 30vw;"
    textfield$style="width: 100%;"
    textfield$helperLine$style="width: 100%;"
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

<Banner
  open={!!pathData}
  centered={true}
  mobileStacked={true}
  fixed={false}
  content$style="max-width: max-content;"
  >
  <BannerLabel slot="label">
    {#if pathData && pathData.count > 0}
      Found {pathData.count} {pathData.count === 1 ? 'path' : 'paths'} of degree {pathData.degrees} in {(elapsed/1000).toFixed(3)} seconds
    {/if}
    {#if pathData && pathData.count === 0}
      Found no paths in {(elapsed/1000).toFixed(3)} seconds
    {/if}
  </BannerLabel>
  <svelte:fragment slot="actions">
    <Button>OK</Button>
  </svelte:fragment>
</Banner>

<main>
  {#if pathData}
    <div class="path-list">
      {#each pathData.paths as path}
        <Card class="path-card" variant="outlined" padded>
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
                      {#if page.description}
                        <SecondaryText>{page.description}</SecondaryText>
                      {/if}
                    </Text>
                </Item>
              {/each}
            </List>
          </Content>
        </Card>
      {/each}
    </div>
  {/if}

{#if loading}
  <div class="loading-container">
    <Activity />
  </div>
{/if}

<Snackbar bind:this={errorSnackbar}>
  <SnackbarLabel>Something has gone terribly wrong ☹️</SnackbarLabel>
  <Actions>
    <IconButton class="material-icons" title="Dismiss">close</IconButton>
  </Actions>
</Snackbar>
</main>

<style>
  main {
    padding-top: 64px;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-height: 90vh;
  }

  .loading-container {
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    flex-grow: 1;
  }

  :global(.parameter-container) {
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    gap: 24px;
    align-items: center;
    justify-content: space-around;
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
