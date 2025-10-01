import { Box, Link, Paper } from "@mui/material";
import { DBStatus } from "./service";
import { Warning } from "@mui/icons-material";
import { TopNetworkGraph } from "./TopNetworkGraph";
import DeviceSwitch from "./DeviceSwitch";

export function StatusPanel({ dbStatus }: { dbStatus: DBStatus }) {
    return (
        <Paper elevation={8} sx={{ flexGrow: 1, maxHeight: '80vh', display: 'flex', justifyContent: 'center', alignItems: 'center', flexDirection: 'column' }}>
            <DeviceSwitch desktop={<TopNetworkGraph />} />
            {!dbStatus.date && (
                <>
                    <Box>
                        <Warning sx={{ fontSize: 48 }} />
                        <p>Dump from {' '}<Link href="https://dumps.wikimedia.org/backup-index.html" target="_blank" rel="noopener noreferrer">Wikipedia dumps</Link>{' '}not found</p>
                    </Box>
                </>
            )}

        </Paper>
    )
}
