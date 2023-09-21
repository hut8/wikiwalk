import * as React from 'react';
import Box from '@mui/material/Box';
import TextField from '@mui/material/TextField';
import Autocomplete from '@mui/material/Autocomplete';
import Grid from '@mui/material/Grid';
import Typography from '@mui/material/Typography';
import { debounce } from '@mui/material/utils';

export const PageInput = () => {
    return (
        <Autocomplete renderInput={(params) => (<TextField {...params} label={"Page"} fullWidth />)}
        />
    )
}