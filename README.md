# wikipedia-speedrun

Shortest path algorithm between pages on Wikipedia. Let's say that you want to get from page `Exploding animal` to `Cucumber`. This will do that:

```sh
wikipedia-speedrun 'Exploding animal' 'Cucumber'
```

## Dependencies

Install `mariadb-server`. You proably want `pv` too for progress when importing.
## Data setup

Before trying to run anything, download the `page` and `pagelinks` tables, and make a database:

```bash
wget https://dumps.wikimedia.org/enwiki/latest/enwiki-latest-page.sql.gz
wget https://dumps.wikimedia.org/enwiki/latest/enwiki-latest-pagelinks.sql.gz
mysql -e 'CREATE DATABASE wiki;'
```

Then import these. It will take a very long time. The page table is relatively small, but the pagelinks table contains a bit over a billion rows (YMMV)

```sh
pv enwiki-latest-page.sql.gz | zcat | mysql wiki
pv enwiki-latest-pagelinks.sql.gz | zcat | mysql wiki
```

Immediately after the pagelinks table is importing, you will quickly want to drop all the indexes and the primary key. Alternatively, you could try editing the dump to not include any keys/indexes, but this is easier. If you don't do this, it will take forever to import (well over a week), and will take over 100 GB more than it would without the indexes.

```sql
-- This will cause the inserts on pagelinks to block while the entire table is copied, so don't wait long to start it!
ALTER TABLE pagelinks DROP PRIMARY KEY;
ALTER TABLE pagelinks DROP INDEX pl_namespace;
ALTER TABLE pagelinks DROP INDEX pl_backlinks_namespace;
```

Now create the main edges table:

```sql
CREATE TABLE `edges` (
  `source_page_id` int(8) unsigned NOT NULL,
  `dest_page_id` int(8) unsigned NOT NULL,
  PRIMARY KEY (`source_page_id`,`dest_page_id`)
) ENGINE=InnoDB;
```
