import { findPaths } from "./service";

type PathParams = {
  sourceId?: string;
  targetId?: string;
};

export const loadPaths = async ({ params }: { params: PathParams }) => {
  if (!params.sourceId || !params.targetId) {
    return null;
  }
  const sourceId = parseInt(params.sourceId);
  const targetId = parseInt(params.targetId);
  console.log("loader:", "sourceId", sourceId, "targetId", targetId);
  const pathData = await findPaths(sourceId, targetId);
  return pathData;
};
