import { Box, Link, Paper, Typography } from "@mui/material";
import { DBStatus } from "./service";
import { Warning } from "@mui/icons-material";

export function StatusPanel({ dbStatus }: { dbStatus: DBStatus }) {
  return (
    <Paper elevation={8} sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '50vh', flexDirection: 'column' }}>
      <Box>
        <Typography variant="h4">WikiWalk</Typography>
      </Box>
      <Box>
        <Typography variant="h6">Find the shortest paths between two Wikipedia pages</Typography>
      </Box>

      {dbStatus.date && (
        <>
          <Box>
            <Typography variant="h6">Searching {dbStatus.edgeCount.toLocaleString()} connections between {dbStatus.vertexCount.toLocaleString()} pages</Typography>
          </Box>
          <Box>
            <Typography variant="caption">
              Data current as of: {dbStatus.date} (updated from <Link href="https://dumps.wikimedia.org/backup-index.html">Wikipedia dumps</Link>)
            </Typography>
          </Box>
        </>)
      }

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
