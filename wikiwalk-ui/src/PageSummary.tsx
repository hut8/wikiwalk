import { Box, Typography } from "@mui/material";
import { Page } from "./service";

export const PageSummary = ({ page }: { page: Page }) => {
  console.log(page)
  return (
    <Box sx={{ display: "flex" }}>
      <Box sx={{ mr: 2, width: "64px" }}>
        {page.iconUrl && <img src={page.iconUrl} alt="" width="64" />}
      </Box>
      <Box>
        <Box>
          <Typography variant="h4">{page.title}</Typography>
        </Box>
        <Box>
          <Typography variant="caption">{page.description}</Typography>
        </Box>
      </Box>
    </Box>
  )
}
