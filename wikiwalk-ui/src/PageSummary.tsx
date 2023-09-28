import { Box, Typography } from "@mui/material";
import { WPPage } from "./service";

export const PageSummary = ({ page }: { page: WPPage }) => {
  return (
    <Box sx={{ display: "flex" }}>
      <Box sx={{ mr: 2, width: "30%" }}>
        {page.thumbnail && <img src={page.thumbnail.source} alt="" width="64" />}
      </Box>
      <Box>
        <Typography variant="h4">{page.title}</Typography>
      </Box>
    </Box>
  )
}
