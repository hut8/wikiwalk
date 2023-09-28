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

const queryClient = new QueryClient()

export default function App() {

  return (
    <>
      <QueryClientProvider client={queryClient}>
        <Box sx={{ flexGrow: 1 }}>
          <AppBar position="static">
            <Toolbar>
              <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
                WikiWalk.app
              </Typography>
              <Button color="inherit">Stats</Button>
            </Toolbar>
          </AppBar>
        </Box>
        <Container maxWidth="lg">
          <Box sx={{ my: 4, display: 'flex', flexDirection: 'row', justifyContent: 'space-between' }}>
            <PageInput />
            <PageInput />
            <Button variant="contained" sx={{flexShrink: 1}}>
              Go
            </Button>
          </Box>

        </Container>
      </QueryClientProvider>
    </>
  )
}
