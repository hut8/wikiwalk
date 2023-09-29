import { Box } from "@mui/material";
import Grid from '@mui/material/Unstable_Grid2'; // Grid version 2

import { Page, PagePaths } from "./service";

function PathDisplay({ path }: { path: Page[] }) {
  return (
    <Grid container spacing={2}>
      {path.map(page => (
        <Grid xs key={page.id}>
          <Box sx={{ mr: 2, width: "30%" }}>
            {page.iconUrl && <img src={page.iconUrl} alt="" width="64" />}
          </Box>
          <Box>{page.title}</Box>
        </Grid>
      ))}
    </Grid>
  )
}

export function PathsDisplay({ paths }: { paths: PagePaths }) {
  if (paths.count === 0) {
    return <Box>No paths found</Box>
  }
  return (
    <>
      <Grid container justifyContent={"center"}>
        <Grid xs justifyContent={"center"}>
        Found {paths.count} paths of degree {paths.degrees} in {paths.duration} milliseconds
        </Grid>
      </Grid>
      <Box sx={{ display: "flex", flexDirection: "column" }}>
        {paths.paths.map(path => (
          <PathDisplay key={path.map(p => p.id).join("-")} path={path} />
        ))}
      </Box>
    </>
  )
}
