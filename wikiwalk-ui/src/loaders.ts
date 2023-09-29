import { Page, PagePaths, fetchPageData, findPaths } from "./service";

type PathParams = {
  sourceId?: string;
  targetId?: string;
};

export type PathLoaderData = {
  source: Promise<Page|null>;
  target: Promise<Page|null>;
  pagePaths: Promise<PagePaths|null>;
};

export const loadPaths = ({
  params,
}: {
  params: PathParams;
}): PathLoaderData | null => {
  if (!params.sourceId || !params.targetId) {
    return null;
  }
  const sourceId = parseInt(params.sourceId);
  const targetId = parseInt(params.targetId);
  console.log("loader:", "sourceId", sourceId, "targetId", targetId);
  const pagePaths = findPaths(sourceId, targetId);
  const source = fetchPageData(sourceId);
  const target = fetchPageData(targetId);
  return {
    source,
    target,
    pagePaths,
  };
};
