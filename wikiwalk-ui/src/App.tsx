import AppBar from '@mui/material/AppBar';
import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
import Button from '@mui/material/Button';
import Container from '@mui/material/Container';
import MultipleStopIcon from '@mui/icons-material/MultipleStop';

import {
  QueryClient,
  QueryClientProvider,
} from '@tanstack/react-query';

import { PageInput } from './PageInput';
import { PageSummary } from './PageSummary';
import { Suspense, useEffect, useState } from 'react';
import { Page } from './service';
import { Await, useLoaderData, useNavigate } from 'react-router-dom';
import { PathsDisplay } from './PathsDisplay';
import { PathLoaderData } from './loaders';

const queryClient = new QueryClient()

export default function App() {
  const [sourcePage, setSourcePage] = useState<Page | null>(null);
  const [targetPage, setTargetPage] = useState<Page | null>(null);
  const { pagePaths, source, target } = useLoaderData() as PathLoaderData;
  const navigate = useNavigate();

  useEffect(() => {
    (async () => {
      source && setSourcePage(await source);
      target && setTargetPage(await target);
    })();
  }, [source, target]);

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
        <Container maxWidth={false}>
          <Box sx={{ my: 4, display: 'flex', flexDirection: 'row', justifyContent: 'space-between', gap: 16 }}>
            <PageInput label='Source page' page={sourcePage} setPage={setSourcePage} />
            <PageInput label='Target page' page={targetPage} setPage={setTargetPage} />
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
                <MultipleStopIcon sx={{ fontSize: 48 }} />
              )}
            </Box>

            <Box sx={{ width: "45%" }}>
              {targetPage && <PageSummary page={targetPage} />}
            </Box>
          </Box>

          <Suspense fallback={<div>Loading...</div>}>
            <Await
              resolve={pagePaths}
              children={(paths) => {
                if (!paths) return null;
                return <PathsDisplay paths={paths} />
              }}
              errorElement={<div>Something went wrong</div>}
            />
          </Suspense>

        </Container>
      </QueryClientProvider>
    </>
  )
}
