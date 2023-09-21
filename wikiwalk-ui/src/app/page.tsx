import Image from 'next/image'

import * as React from 'react';
import AppBar from '@mui/material/AppBar';
import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
import Button from '@mui/material/Button';
import Container from '@mui/material/Container';

export default function Home() {
    return (
        <>
            <Box sx={{flexGrow: 1}}>
                <AppBar position="static">
                    <Toolbar>
                        <Typography variant="h6" component="div" sx={{flexGrow: 1}}>
                            WikiWalk.app
                        </Typography>
                        <Button color="inherit">Stats</Button>
                    </Toolbar>
                </AppBar>
            </Box>
            <Container maxWidth="lg">

            </Container>
            </>
    )
}
