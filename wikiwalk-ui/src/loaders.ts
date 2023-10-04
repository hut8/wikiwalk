import { DBStatus, Page, PagePaths, fetchDatabaseStatus, fetchPageData, findPaths } from "./service";

type PathParams = {
  sourceId?: string;
  targetId?: string;
};

export type PathLoaderData = {
  source: Promise<Page|null>;
  target: Promise<Page|null>;
  pagePaths: Promise<PagePaths|null>;
  dbStatus: Promise<DBStatus>;
};

export const loadPaths = ({
  params,
}: {
  params: PathParams;
}): PathLoaderData => {
  const dbStatus = fetchDatabaseStatus();
  if (!params.sourceId || !params.targetId) {
    return {
      source: Promise.resolve(null),
      target: Promise.resolve(null),
      pagePaths: Promise.resolve(null),
      dbStatus,
    };
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
    dbStatus,
  };
};
