import { writable } from "svelte/store";

export async function runSearch(term: string) {
  const wikiParams = new URLSearchParams();
  wikiParams.set("action", "query");
  wikiParams.set("format", "json");
  wikiParams.set("gpssearch", term);
  wikiParams.set("generator", "prefixsearch");
  wikiParams.set("prop", "pageprops|pageimages|pageterms");
  wikiParams.set("redirects", "");
  wikiParams.set("ppprop", "displaytitle");
  wikiParams.set("piprop", "thumbnail");
  wikiParams.set("pithumbsize", "160");
  wikiParams.set("pilimit", "30");
  wikiParams.set("wbptterms", "description");
  wikiParams.set("gpsnamespace", "0");
  wikiParams.set("gpslimit", "5");
  wikiParams.set("origin", "*");

  const endpoint = new URL("https://en.wikipedia.org/w/api.php");
  endpoint.search = wikiParams.toString();
  const response = await fetch(endpoint);
  return response.json();
}

export type Page = {
  title: string;
  id: number;
  iconUrl: string;
  link: string;
};

// Given directly from backend
type PageIDPaths = {
  // [ [source, intermediate..., dest ], ... ]
  paths: number[][];
}

// Run through Wikipedia API to get more data
type PagePaths = {
  paths: Page[][]
}

export async function findPaths(sourceId: number, targetId: number) {
  const endpoint = `/paths/${sourceId}/${targetId}`;
  const pageIdPaths = (await fetch(endpoint)) as unknown as PageIDPaths;

}

async function fetchPathPageData(pageIdPaths: PageIDPaths): PagePaths {
  const pageIdSet = new Set(pageIdPaths.paths.flatMap((x) => x));

}

export const paths = writable<PagePaths>();
