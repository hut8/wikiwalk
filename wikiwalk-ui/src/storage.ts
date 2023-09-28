import { Page } from "./service";

const cacheVersion = 1;

const pageKey = (page: Page) => {
  return `page-${cacheVersion}-${page.id}`;
}

export const storePage = (page: Page) => {
  localStorage.setItem(pageKey(page), JSON.stringify(page));
}

export const loadPage = (page: Page) => {
  const stored = localStorage.getItem(pageKey(page));
  if (!stored) {
    return null;
  }
  return JSON.parse(stored) as Page;
}
