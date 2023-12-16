import * as React from 'react';
import Box from '@mui/material/Box';
import TextField from '@mui/material/TextField';
import Autocomplete from '@mui/material/Autocomplete';
import Grid from '@mui/material/Grid';
import Typography from '@mui/material/Typography';
import { Page, runSearch } from './service';
import { PageSummary } from './PageSummary';

type PageInputParams = {
  page: Page | null;
  setPage: (p: Page) => void;
  label: string;
};

export const PageInput = ({ page, setPage, label }: PageInputParams) => {
  const [inputValue, setInputValue] = React.useState('');
  const [options, setOptions] = React.useState<Page[]>([]);

  React.useEffect(() => {
    let active = true;
    async function searchPages() {
      if (inputValue === '') {
        setOptions([]);
        return undefined;
      }
      const pageResults = await runSearch(inputValue);
      if (!active) {
        return undefined;
      }
      setOptions(pageResults);
    }
    searchPages();
    return () => {
      active = false;
    }
  }, [inputValue]);

  return (
    <Box sx={{ display: "flex", flexDirection: "column", flexGrow: 1, width: "100%", gap: "24px" }}>
      <Autocomplete sx={{
        minWidth: 300,
        flexGrow: 1,
      }}
        getOptionLabel={(option: Page) => option.title}
        filterOptions={(x) => x}
        options={options}
        value={page}
        noOptionsText={"No pages found"}
        isOptionEqualToValue={(option, value) => option.id === value.id}
        onChange={(_event, newValue) => {
          setOptions(newValue ? [newValue, ...options] : options);
          setPage(newValue as Page);
        }}
        onInputChange={(_event, newInputValue) => {
          setInputValue(newInputValue);
        }}
        renderOption={(props, option) => {
          return (
            <Box component="li" sx={{ '& > img': { mr: 2, flexShrink: 0 }, }} {...props}>
              <Grid container alignItems="center" spacing={2}>
                <Grid item height={"40px"} width={"40px"}>
                  {option?.iconUrl && <img src={option.iconUrl} alt="" width="32" />}
                </Grid>
                <Grid item>
                  <Typography variant="body2" color="text.primary">
                    {option.title}
                  </Typography>
                </Grid>
              </Grid>
            </Box>
          )
        }}
        renderInput={(params) => (<TextField {...params} label={label} fullWidth />)}
      />
      {page && <PageSummary page={page} />}
    </Box>
  )
}
