import * as React from 'react';
import Box from '@mui/material/Box';
import TextField from '@mui/material/TextField';
import Autocomplete from '@mui/material/Autocomplete';
import Grid from '@mui/material/Grid';
import Typography from '@mui/material/Typography';
import IconButton from '@mui/material/IconButton';
import Tooltip from '@mui/material/Tooltip';
import { Page, runSearch, fetchRandomPage } from './service';
import { PageSummary } from './PageSummary';
import { useTranslation } from 'react-i18next';

type PageInputParams = {
  page: Page | null;
  setPage: (p: Page) => void;
  label: string;
};

const DiceIcon = () => (
  <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
    <rect x="3" y="3" width="18" height="18" rx="2" stroke="currentColor" strokeWidth="2" fill="none"/>
    <circle cx="8" cy="8" r="1.5" fill="currentColor"/>
    <circle cx="16" cy="8" r="1.5" fill="currentColor"/>
    <circle cx="12" cy="12" r="1.5" fill="currentColor"/>
    <circle cx="8" cy="16" r="1.5" fill="currentColor"/>
    <circle cx="16" cy="16" r="1.5" fill="currentColor"/>
  </svg>
);

export const PageInput = ({ page, setPage, label }: PageInputParams) => {
  const { t } = useTranslation();
  const [inputValue, setInputValue] = React.useState('');
  const [options, setOptions] = React.useState<Page[]>([]);
  const [isRotating, setIsRotating] = React.useState(false);

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

  const handleRandomClick = async () => {
    setIsRotating(true);
    try {
      const randomPage = await fetchRandomPage();
      setPage(randomPage);
    } catch (error) {
      console.error('Failed to fetch random page:', error);
    } finally {
      setIsRotating(false);
    }
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", flexGrow: 1, width: "100%", gap: "24px" }}>
      <Box sx={{ display: "flex", gap: 1, alignItems: "flex-start" }}>
        <Autocomplete sx={{
          minWidth: 300,
          flexGrow: 1,
        }}
          getOptionLabel={(option: Page) => option.title}
          filterOptions={(x) => x}
          options={options}
          value={page}
          noOptionsText={t('noOptionsText')}
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
        <Tooltip title={t('randomArticle') || 'Random article'}>
          <IconButton
            onClick={handleRandomClick}
            sx={{
              mt: 1,
              bgcolor: 'primary.main',
              color: 'white',
              '&:hover': {
                bgcolor: 'primary.dark',
              },
              animation: isRotating ? 'spin 1s linear infinite' : 'none',
              '@keyframes spin': {
                '0%': {
                  transform: 'rotate(0deg)',
                },
                '100%': {
                  transform: 'rotate(360deg)',
                },
              },
            }}
          >
            <DiceIcon />
          </IconButton>
        </Tooltip>
      </Box>
      {page && <PageSummary page={page} />}
    </Box>
  )
}
