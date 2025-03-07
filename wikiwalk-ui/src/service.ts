import { loadPage, loadPages, storePage } from "./storage";

export type Page = {
  title: string;
  id: number;
  iconUrl?: string;
  link: string;
  description?: string;
};

// Run through Wikipedia API to get more data
export type PagePaths = {
  paths: Page[][];
  degrees?: number;
  count: number;
  duration: number; // milliseconds
};

// Data returned our service at /paths/:sourceId/:targetId
export type PathData = {
  paths: number[][];
  degrees?: number;
  count: number;
  duration: number; // milliseconds
};

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
  fullurl: string;
  pageprops: Record<string, string>;
}

export type DBStatus = {
  vertexCount: number;
  edgeCount: number;
  date: string;
};

export type Vertex = {
  id: string;
  title: string;
  color?: string;
  top: boolean;
  rank?: number;
};

export type Edge = {
  source: string;
  target: string;
  color?: string;
};

export type GraphPayload = {
  vertexes: Vertex[];
  edges: Edge[];
};

export async function runSearch(term: string): Promise<Page[]> {
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
  const response = await fetch(endpoint, {
    headers: {
      "User-Agent": "WikiWalk wikiwalk.app liambowen@gmail.com",
    },
  });
  const data = await response.json();
  if ("error" in data) {
    console.error("wikipedia api error:", data.error);
    return [];
  }
  const results = Object.values(data.query.pages) as WPPage[];
  results.sort((x, y) => x.index - y.index);
  console.log(results);
  const pages = results.map((page) => transformPage(page));
  return pages;
}

const serviceEndpointBase = new URL("https://wikiwalk.app/");

export async function topGraph(): Promise<GraphPayload> {
  const endpoint = new URL(`/top-graph`, serviceEndpointBase);

  const response = await fetch(endpoint, {
    headers: {
      Accept: "application/json",
    },
  });
  if (!response.ok) {
    throw new Error("bad response code from server");
  }
  const data = (await response.json()) as GraphPayload;
  for (const v of data.vertexes) {
    v.rank ??= 0;
  }

  return data;
}

export function pathsGraph(pd: PagePaths): GraphPayload {
  const vertexMap = pd.paths
    .flatMap((x) => x)
    .reduce((agg, val) => {
      agg[val.id] = {
        id: val.id.toString(),
        top: false,
        title: val.title,
      };
      return agg;
    }, {} as Record<number, Vertex>);
  const vertexes = Object.values(vertexMap);

  const makeEdges = (path: Page[]) =>
    path
      .slice(0, -1)
      .map((currentPage, ix, pages) => {
        const source = currentPage.id.toString();
        const target = pages[ix + 1].id.toString();
        return {source, target};
      });

  const edges: Edge[] = pd.paths.map(makeEdges).flat();
  return {
    edges,
    vertexes,
  };
}

export async function findPaths(
  sourceId: number,
  targetId: number
): Promise<PagePaths> {
  const endpoint = new URL(
    `/paths/${sourceId}/${targetId}`,
    serviceEndpointBase
  );

  gtag("event", "search_paths", {
    sourceId,
    targetId,
  });

  const startTime = Date.now();

  const response = await fetch(endpoint, {
    headers: {
      Accept: "application/json",
    },
  });
  if (!response.ok) {
    throw new Error("bad response code from server");
  }
  const data = (await response.json()) as PathData;

  const elapsed = Date.now() - startTime;
  gtag("event", "path_search_duration", {
    value: elapsed,
  });

  const pagePaths = await fetchPathPageData(data);
  console.log("page paths", pagePaths);
  return pagePaths;
}

const CHUNK_SIZE = 50;

function* batchArray<T>(arr: T[], n: number): Generator<T[], void> {
  for (let i = 0; i < arr.length; i += n) {
    yield arr.slice(i, i + n);
  }
}

export async function fetchPageData(pageId: number): Promise<Page> {
  const cachedPage = loadPage(pageId);
  if (cachedPage) {
    return cachedPage;
  }
  const pageData = await fetchPageDataChunk([pageId]);
  const page = transformPage(pageData[pageId]);
  storePage(page);
  return page;
}

async function fetchPageDataChunk(pageIds: number[]) {
  const pageIDStr = pageIds.join("|");
  const wikiParams = new URLSearchParams();
  wikiParams.set("action", "query");
  wikiParams.set("format", "json");
  wikiParams.set("pageids", pageIDStr);
  wikiParams.set("prop", "info|pageprops|pageimages|pageterms");
  wikiParams.set("inprop", "url");
  wikiParams.set("piprop", "thumbnail");
  wikiParams.set("pithumbsize", "160");
  wikiParams.set("pilimit", "50");
  wikiParams.set("wbptterms", "description");
  wikiParams.set("origin", "*");

  const endpoint = new URL("https://en.wikipedia.org/w/api.php");
  endpoint.search = wikiParams.toString();
  const response = await fetch(endpoint, {
    headers: {
      "User-Agent": "WikiWalk wikiwalk.app liambowen@gmail.com",
    },
  });
  const data = await response.json();
  if ("error" in data) {
    console.error("wikipedia api error:", data.error);
    return null;
  }
  return data.query.pages;
}

function transformPage(page: WPPage): Page {
  const p: Page = {
    id: page.pageid,
    title: page.title,
    link: page.fullurl,
  };
  if (page.pageprops && page.pageprops["wikibase-shortdesc"]) {
    p.description = page.pageprops["wikibase-shortdesc"];
  }
  if (page.terms?.description) {
    p.description = page.terms.description[0];
  }
  if (page.thumbnail?.source) {
    p.iconUrl = page.thumbnail.source;
  }
  return p;
}

async function fetchPathPageData(data: PathData): Promise<PagePaths> {
  const pageIdPaths: number[][] = data.paths;
  const pageIdSet = new Set(pageIdPaths.flatMap((x) => x));

  const pageLoadResult = loadPages(pageIdSet);
  console.log("page load result:", pageLoadResult);

  const fetchPageIds = pageLoadResult.missing;
  let pageData = {} as Record<string, WPPage>;
  console.log("running page info query for ids:", fetchPageIds);
  const batches = batchArray(fetchPageIds, CHUNK_SIZE);
  for (const batch of Array.from(batches)) {
    const pageDataChunk = await fetchPageDataChunk(batch);
    console.log("page data chunk:", pageDataChunk);
    pageData = Object.assign(pageData, pageDataChunk);
  }
  console.log("page data:", pageData);

  const pagePaths: PagePaths = {
    paths: [],
    degrees: data.degrees,
    count: data.count,
    duration: data.duration,
  };

  for (const pageIdPath of pageIdPaths) {
    const pages = pageIdPath.map((pageId) => {
      if (pageId in pageLoadResult.pageMap) {
        return pageLoadResult.pageMap[pageId];
      }
      const p = transformPage(pageData[pageId]);
      storePage(p);
      return p;
    });
    pagePaths.paths.push(pages);
  }

  return pagePaths;
}

export async function fetchDatabaseStatus(): Promise<DBStatus> {
  const endpoint = new URL("/status", serviceEndpointBase);
  const response = await fetch(endpoint, {
    headers: {
      Accept: "application/json",
    },
  });
  if (!response.ok) {
    throw new Error("bad response code from server");
  }
  const data = (await response.json()) as DBStatus;
  return data;
}
