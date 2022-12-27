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

export const pageStore = writable<Map<number, WPPage>>(new Map());

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
    const response = await fetch(endpoint, {
        headers: {
            "User-Agent":
                "Wikipedia Speedrun wikipediaspeedrun.com liambowen@gmail.com",
        },
    });
    const data = await response.json();
    if ("error" in data) {
        console.error("wikipedia api error:", data.error);
        return [];
    }
    const pages = Object.values(data.query.pages) as WPPage[];
    pages.sort((x: any, y: any) => x.index - y.index);
    console.log(pages);
    pageStore.update(($pageStore) => {
        for (const p of pages) {
            $pageStore.set(p.pageid, p);
        }
        return $pageStore;
    });
    return pages;
}

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
    degrees?: number,
    count: number,
};

export type PathData = {
    paths: number[][],
    degrees?: number,
    count: number,
}
export async function findPaths(
    sourceId: number,
    targetId: number
): Promise<PagePaths> {
    const endpoint = `/paths/${sourceId}/${targetId}`;
    const response = await fetch(endpoint);
    if (!response.ok) {
        throw new Error("bad response code from server");
    }
    const data = await response.json() as PathData;
    const pagePaths = await fetchPathPageData(data);
    return pagePaths;
}

const CHUNK_SIZE = 50;

function batchArray<T>(arr: T[]): T[][] {
    return arr.reduce((all, one, i) => {
        const ch = Math.floor(i / CHUNK_SIZE);
        all[ch] = [].concat(all[ch] || [], one);
        return all;
    }, []);
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
            "User-Agent":
                "Wikipedia Speedrun wikipediaspeedrun.com liambowen@gmail.com",
        },
    });
    const data = await response.json();
    if ("error" in data) {
        console.error("wikipedia api error:", data.error);
        return null;
    }
    return data.query.pages;
}

async function fetchPathPageData(data: PathData): Promise<PagePaths> {
    const pageIdPaths: number[][] = data.paths;
    const pageIdSet = new Set(pageIdPaths.flatMap((x) => x));
    let pageData = {};
    console.log("running page info query for ids:", pageIdSet.keys());
    let batches = batchArray(Array.from(pageIdSet.values()));
    for (let batch of batches) {
        let pageDataChunk = await fetchPageDataChunk(batch);
        console.log("page data chunk:", pageDataChunk);
        pageData = Object.assign(pageData, pageDataChunk);
    }
    console.log("page data:", pageData);

    const pagePaths: PagePaths = {
        paths: [],
        degrees: data.degrees,
        count: data.count,
    };

    for (const pageIdPath of pageIdPaths) {
        const pages: Page[] = [];
        for (const pageId of pageIdPath) {
            const wpPage = pageData[pageId];
            const p: Page = {
                id: wpPage.pageid,
                title: wpPage.title,
                link: wpPage.fullurl,
            };
            if (wpPage.pageprops && wpPage.pageprops["wikibase-shortdesc"]) {
                p.description = wpPage.pageprops["wikibase-shortdesc"];
            }
            if (wpPage.thumbnail && wpPage.thumbnail.source) {
                p.iconUrl = wpPage.thumbnail.source;
            }
            pages.push(p);
        }
        pagePaths.paths.push(pages);
    }

    return pagePaths;
}
