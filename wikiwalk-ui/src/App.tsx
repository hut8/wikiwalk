import AppBar from "@mui/material/AppBar";
import Box from "@mui/material/Box";
import Toolbar from "@mui/material/Toolbar";
import Typography from "@mui/material/Typography";
import Button from "@mui/material/Button";
import Container from "@mui/material/Container";
//import MultipleStopIcon from "@mui/icons-material/MultipleStop";
import ForwardIcon from "@mui/icons-material/Forward";
import GitHubIcon from '@mui/icons-material/GitHub';

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { PageInput } from "./PageInput";
import { Suspense, useEffect, useState } from "react";
import { Page } from "./service";
import { Await, useLoaderData, useNavigate } from "react-router-dom";
import { PathsDisplay } from "./PathsDisplay";
import { PathLoaderData } from "./loaders";
import { Activity } from "./Activity";
import { StatusPanel } from "./StatusPanel";
import { IconButton, Link } from "@mui/material";

const queryClient = new QueryClient();

export default function App() {
  const [sourcePage, setSourcePage] = useState<Page | null>(null);
  const [targetPage, setTargetPage] = useState<Page | null>(null);
  const { pagePaths, source, target, dbStatus } = useLoaderData() as PathLoaderData;
  const navigate = useNavigate();

  const setTitle = (sourcePage: Page, targetPage: Page) => {
    document.title = (() => {
      if (sourcePage && targetPage) {
        return `WikiWalk - ${sourcePage.title} ➔ ${targetPage.title}`;
      }
      return "WikiWalk.app";
    })();
  };

  useEffect(() => {
    (async () => {
      const [s, t] = await Promise.all([source, target]);
      s && setSourcePage(s);
      t && setTargetPage(t);
      (s && t) && setTitle(s, t);
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
  };

  const openGitHub = () => {
    window.open("https://github.com/hut8/wikiwalk", "_blank");
  };

  return (
    <>
      <QueryClientProvider client={queryClient}>
        <AppBar position="static">
          <Toolbar>
            <Box sx={{ flexGrow: 1 }}>
              <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
                WikiWalk.app
              </Typography>
            </Box>
            <Box sx={{ flexGrow: 1 }}>
              <Typography variant="caption">
                Find the shortest paths between two Wikipedia pages
              </Typography>
            </Box>
            <Box>
              <Await resolve={dbStatus} children={(status) =>
                <Typography variant="caption">
                  <Box>
                    Searching {status.edgeCount.toLocaleString()} {' '}connections between{' '}
                    {status.vertexCount.toLocaleString()}{' '}pages.
                    {' '}
                    <Link color={'#ffffff'} href="https://dumps.wikimedia.org/backup-index.html" target="_blank">
                      Data from {status.date}
                    </Link>
                  </Box>
                </Typography>
              } />
            </Box>
            <Box sx={{ flexGrow: 0, marginLeft: 3 }}>
              <IconButton onClick={() => openGitHub()} sx={{ p: 0 }}>
                <GitHubIcon sx={{ color: "white" }} />
              </IconButton>
            </Box>
          </Toolbar>
        </AppBar>
        <Container sx={{flexGrow: 1, display: "flex", flexDirection: "column"}} maxWidth={false}>
          <Box
            sx={{
              my: 4,
              display: "flex",
              flexDirection: "row",
              justifyContent: "space-between",
              gap: "64px",
            }}
          >
            <PageInput
              label="Source page"
              page={sourcePage}
              setPage={setSourcePage}
            />
            <ForwardIcon sx={{ fontSize: 48 }} />
            <PageInput
              label="Target page"
              page={targetPage}
              setPage={setTargetPage}
            />
            <Button
              variant="contained"
              sx={{ flexShrink: 1 }}
              onClick={triggerSearch}
            >
              Go
            </Button>
          </Box>

          {!(sourcePage || targetPage) &&
            <Await resolve={dbStatus} children={(status) => <StatusPanel dbStatus={status} />} />
          }

          <Suspense fallback={<Activity />}>
            <Await
              resolve={pagePaths}
              children={(paths) => {
                if (!paths) return null;
                return <PathsDisplay paths={paths} />;
              }}
              errorElement={<div>Something went wrong</div>}
            />
          </Suspense>
        </Container>
      </QueryClientProvider >
    </>
  );
}
