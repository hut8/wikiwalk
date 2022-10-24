cargo build && `
.\target\debug\wikipedia-speedrun.exe build `
  --page C:\Users\liamb\Data\enwiki-20220901-page.sql `
  --pagelinks C:\Users\liamb\Data\pagelinks.sql `
  --redirects C:\Users\liamb\Data\enwiki-20220901-redirect.sql # *>&1 | Tee-Object -FilePath "log.txt"
