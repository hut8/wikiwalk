import { Page } from "./service";

const cacheVersion = 1;

const pageKey = (pageId: number) => {
  return `page-${cacheVersion}-${pageId}`;
};

export const storePage = (page: Page) => {
  localStorage.setItem(pageKey(page.id), JSON.stringify(page));
};

export const loadPage = (pageId: number) => {
  const stored = localStorage.getItem(pageKey(pageId));
  if (!stored) {
    return null;
  }
  return JSON.parse(stored) as Page;
};

export type PageLoadResult = {
  pageMap: Record<number, Page>;
  missing: number[];
};

export const loadPages = (pageIds: Set<number>): PageLoadResult => {
  const pages = Array.from(pageIds.values())
    .map((id) => loadPage(id))
    .filter((p) => p) as Page[];
  const pageMap = Object.fromEntries(pages.map((p) => [p.id, p]));

  const loadedPageIds = new Set(pages.map((p) => p.id));
  const missing = Array.from(pageIds.values()).filter(
    (id) => !loadedPageIds.has(id)
  );
  return {
    pageMap,
    missing,
  };
};
