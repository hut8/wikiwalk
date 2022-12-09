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
    type WPPage,
    type PagePaths,
  } from "./lib/wikipedia-speedrun";
  import { Timer } from "./lib/timer";

  let sourcePage: WPPage;
  let targetPage: WPPage;
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

  async function autocomplete(term: string): Promise<WPPage[] | false> {
    if (
      (sourcePage && term === sourcePage.title) ||
      (targetPage && term === targetPage.title)
    ) {
      console.debug("autocomplete attempted with same value:", term);
      return [];
    }
    if (term === "") {
      console.debug("blank search term");
      return [];
    }
    console.log("searching for", term);
    return search(term);
  }

  async function computePaths() {
    if (!(sourcePage && targetPage)) {
      console.warn("tried to compute paths without pages set");
      return;
    }
    const sourceId = sourcePage.pageid;
    const targetId = targetPage.pageid;
    snackbar.forceOpen();
    pathData = await findPaths(sourceId, targetId);
    snackbar.close();
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
  <div class="page-inputs">
    <Autocomplete
      search={autocomplete}
      bind:value={sourcePage}
      getOptionLabel={(option) => (option ? option.title : "")}
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

    <Autocomplete
      search={autocomplete}
      bind:value={targetPage}
      getOptionLabel={(option) => (option ? option.title : "")}
      showMenuWithNoInput={false}
      label="Target page"
    >
      <Text
        slot="loading"
        style="display: flex; width: 100%; justify-content: center; align-items: center;"
      >
        <LinearProgress style="height: 24px" indeterminate />
      </Text>
    </Autocomplete>

    <Button on:click={computePaths} variant="raised">
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
