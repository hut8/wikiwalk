import { Box } from "@mui/material";
import { Page, PagePaths } from "./service";

function PathDisplay({ path }: { path: Page[] }) {
  return (
    <Box sx={{ display: "flex", flexDirection: "row" }}>
      {path.map(page => (
        <Box>
          <Box sx={{ mr: 2, width: "30%" }}>
            {page.iconUrl && <img src={page.iconUrl} alt="" width="64" />}
          </Box>
          <Box>{page.title}</Box>
        </Box>
      ))}
    </Box>
  )
}

export function PathsDisplay({ paths }: { paths: PagePaths }) {
  if (paths.count === 0) {
    return <Box>No paths found</Box>
  }
  return (
    <>
      <Box sx={{ display: "flex", flexDirection: "row" }}>
        Found {paths.count} paths of degree {paths.degrees} in {paths.duration} milliseconds
      </Box>
      <Box sx={{ display: "flex", flexDirection: "column" }}>
        {paths.paths.map(path => (
          <PathDisplay path={path} />
        ))}
      </Box>
    </>
  )
}
