import { IconButton } from '@mui/material';
import Brightness4Icon from '@mui/icons-material/Brightness4';
import Brightness7Icon from '@mui/icons-material/Brightness7';
import { useThemeMode } from './useThemeMode';

export default function DarkModeToggle() {
    const { mode, toggleTheme } = useThemeMode();

    return (
        <IconButton
            onClick={toggleTheme}
            sx={{ p: 0, marginRight: 2 }}
            aria-label={mode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        >
            {mode === 'dark' ? (
                <Brightness7Icon />
            ) : (
                <Brightness4Icon />
            )}
        </IconButton>
    );
}
