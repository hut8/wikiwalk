import { Box, Link, Paper } from "@mui/material";
import Grid from '@mui/material/Unstable_Grid2'; // Grid version 2
import Divider from '@mui/material/Divider';
import List from '@mui/material/List';
import ListItem from '@mui/material/ListItem';
import Avatar from '@mui/material/Avatar';
import ListItemText from '@mui/material/ListItemText';
import ListItemAvatar from '@mui/material/ListItemAvatar';

import { Page, PagePaths } from "./service";

function PagePathDisplay({ page }: { page: Page }) {
  return (
    <ListItem alignItems="flex-start">
      <ListItemAvatar>
        {page.iconUrl && <Avatar src={page.iconUrl} sx={{ borderRadius: 0 }} />}
      </ListItemAvatar>
      <ListItemText primary={
        <Link href={page.link} target="_blank" rel="noopener noreferrer">{page.title}</Link>
      }
        secondary={page.description} />
    </ListItem>
  )
}

function PathDisplay({ path }: { path: Page[] }) {
  return (
    <Grid width={"30vw"} minWidth={"300px"}>
      <Paper elevation={8} sx={{ p: 2, mb: 2, height: "90%" }}>
        <List>
          {path.map((page, i) => (
            <>
              <PagePathDisplay key={page.id} page={page} />
              {(i + 1 !== path.length) && <Divider variant="inset" component="li" />}
            </>
          ))}
        </List>
      </Paper>
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
        <Grid xs justifyContent={"center"} textAlign={"center"}>
          <Paper sx={{ p: 2, mb: 2 }}>
            Found {paths.count} paths of degree {paths.degrees} in {paths.duration} milliseconds
          </Paper>
        </Grid>
      </Grid>
      <Grid container direction={"row"} gap={4} justifyContent={"space-between"} alignItems={"stretch"} wrap="wrap">
        {paths.paths.map(path => (
          <PathDisplay key={path.map(p => p.id).join("-")} path={path} />
        ))}
      </Grid>
    </>
  )
}
