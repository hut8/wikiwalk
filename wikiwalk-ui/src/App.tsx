import AppBar from '@mui/material/AppBar';
import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
import Button from '@mui/material/Button';
import Container from '@mui/material/Container';

import {
  QueryClient,
  QueryClientProvider,
} from '@tanstack/react-query';

import { PageInput } from './PageInput';
import { PageSummary } from './PageSummary';
import { useState } from 'react';
import { Page, PagePaths } from './service';
import { Await, useLoaderData, useNavigate } from 'react-router-dom';
import { PathsDisplay } from './PathsDisplay';

const queryClient = new QueryClient()

export default function App() {
  const [sourcePage, setSourcePage] = useState<Page | null>(null);
  const [targetPage, setTargetPage] = useState<Page | null>(null);
  const pathData = useLoaderData() as Promise<PagePaths | null>;
  const navigate = useNavigate();

  const triggerSearch = () => {
    if (!(sourcePage && targetPage)) {
      return;
    }
    const sourceId = sourcePage.id;
    const targetId = targetPage.id;
    const url = `/paths/${sourceId}/${targetId}`;
    navigate(url);
  }

  return (
    <>
      <QueryClientProvider client={queryClient}>
        <AppBar position="static">
          <Toolbar>
            <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
              WikiWalk.app
            </Typography>
            <Button color="inherit">Stats</Button>
          </Toolbar>
        </AppBar>
        <Container maxWidth="lg">
          <Box sx={{ my: 4, display: 'flex', flexDirection: 'row', justifyContent: 'space-between' }}>
            <PageInput page={sourcePage} setPage={setSourcePage} />
            <PageInput page={targetPage} setPage={setTargetPage} />
            <Button variant="contained" sx={{ flexShrink: 1 }} onClick={triggerSearch}>
              Go
            </Button>
          </Box>

          <Box sx={{ my: 4, display: "flex", flexDirection: "row" }}>
            <Box sx={{ width: "45%" }}>
              {sourcePage && <PageSummary page={sourcePage} />}
            </Box>

            <Box sx={{ width: "10%", display: "flex", alignItems: "center" }}>
              {sourcePage && targetPage && (
                <Typography variant="h2">►</Typography>
              )}
            </Box>

            <Box sx={{ width: "45%" }}>
              {targetPage && <PageSummary page={targetPage} />}
            </Box>
          </Box>

          <Await resolve={pathData} children={(paths) => {
            if (!paths) return null;
            return <PathsDisplay paths={paths} />
          }} />

        </Container>
      </QueryClientProvider>
    </>
  )
}
