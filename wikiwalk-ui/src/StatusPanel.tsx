import { Box, Link, Typography } from "@mui/material";
import { DBStatus } from "./service";
import { Warning } from "@mui/icons-material";

export function StatusPanel({dbStatus} : {dbStatus: DBStatus}) {
  return (
    <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '80vh' }}>
      <Typography variant="h4">WikiWalk</Typography>

      {dbStatus.dumpDate && (
        <>
          <Typography variant="h6">Searching {dbStatus.edgeCount} connections between {dbStatus.vertexCount} pages</Typography>
          <Typography variant="caption">
            Data current as of: {dbStatus.dumpDate} (updated from <Link href="https://dumps.wikimedia.org/backup-index.html">Wikipedia dumps</Link>)
          </Typography>
        </>)
      }

      {!dbStatus.dumpDate && (
        <>
          <Warning sx={{ fontSize: 48 }} /> Dump from <Link href="https://dumps.wikimedia.org/backup-index.html">Wikipedia dumps</Link> not found
        </>
      )}

    </Box>
  )
}
