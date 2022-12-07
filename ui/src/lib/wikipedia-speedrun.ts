import { writable } from "svelte/store";

export interface WPThumbnail {
  source: string;
  width: number;
  height: number;
}

export interface WPTerms {
  description: string[];
}

export interface WPPage {
  pageid: number;
  ns: number;
  title: string;
  index: number;
  thumbnail: WPThumbnail;
  terms: WPTerms;
}

export async function runSearch(term: string): Promise<WPPage[]> {
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
  const data = await response.json();
  if ("error" in data) {
    console.error("wikipedia api error:", data.error);
    return [];
  }
  const pages = Object.values(data.query.pages) as WPPage[];
  pages.sort((x: any, y: any) => x.index - y.index);
  console.log(pages);
  return pages;
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
};

// Run through Wikipedia API to get more data
type PagePaths = {
  paths: Page[][];
};

export async function findPaths(sourceId: number, targetId: number) {
  const endpoint = `/paths/${sourceId}/${targetId}`;
  const pageIdPaths = (await fetch(endpoint)) as unknown as PageIDPaths;
}

async function fetchPathPageData(pageIdPaths: PageIDPaths): Promise<PagePaths> {
  const pageIdSet = new Set(pageIdPaths.paths.flatMap((x) => x));
  return {
    paths: [],
  };
}

export const paths = writable<PagePaths>({
  paths: [],
});
