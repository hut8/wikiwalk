import { Box } from '@mui/material';
import './Activity.css';

export function Activity() {
  return (
    <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '50vh', flexDirection: 'column' }}>
      <div className="la-pacman la-dark la-3x">
        <div />
        <div />
        <div />
        <div />
        <div />
        <div />
      </div>
    </Box>
  )

}
