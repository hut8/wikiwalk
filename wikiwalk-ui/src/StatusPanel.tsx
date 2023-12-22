import { Box, Link, Paper } from "@mui/material";
import { DBStatus } from "./service";
import { Warning } from "@mui/icons-material";
import { TopGraph } from "./Graph";

export function StatusPanel({ dbStatus }: { dbStatus: DBStatus }) {
  return (
    <Paper elevation={8} sx={{ flexGrow: 1, maxHeight: '80vh', display: 'flex', justifyContent: 'center', alignItems: 'center', flexDirection: 'column' }}>
      <TopGraph />
      {!dbStatus.date && (
        <>
          <Box>
            <Warning sx={{ fontSize: 48 }} />
            <p>Dump from {' '}<Link href="https://dumps.wikimedia.org/backup-index.html">Wikipedia dumps</Link>{' '}not found</p>
          </Box>
        </>
      )}

    </Paper>
  )
}
