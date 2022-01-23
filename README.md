# wikipedia-speedrun

Shortest path algorithm between pages on Wikipedia. Let's say that you want to get from page `Exploding animal` to `Cucumber`. This will do that:

```sh
wikipedia-speedrun 'Exploding animal' 'Cucumber'
```

## Dependencies

Install `mariadb-server`. You proably want `pv` too for progress when importing.
## Data setup

Add this line to `/etc/mysql/mariadb.conf.d/50-server.conf`:

```
innodb_buffer_pool_size = 1G
```

Then restart mariadb: `sudo systemctl restart mysql`

Download the `page` and `pagelinks` tables, and make a database:

### `page` table

```bash
wget https://dumps.wikimedia.org/enwiki/latest/enwiki-latest-page.sql.gz
mysql -e 'CREATE DATABASE wiki;'
```

Then import the page table.

```sh
pv enwiki-latest-page.sql.gz | zcat | mysql wiki
```

Now create a `vertexes` table that is a stripped down version of the `page` table:

```sql
CREATE TABLE vertexes AS
      SELECT page_id, page_namespace, page_title
        FROM page;

-- Then you can drop `page` if you want to save some space!
DROP TABLE page;

ALTER TABLE vertexes ADD PRIMARY KEY (page_id); -- 5 Minutes
ALTER TABLE vertexes ADD INDEX page_title_index (page_title); -- 5 Minutes
```

### `pagelinks` table

`pagelinks` is a huge table with a bit over a billion rows. Immediately after the pagelinks table is importing, you will quickly want to drop all the indexes and the primary key. Alternatively, you could try editing the dump to not include any keys/indexes, but this is easier. If you don't do this, it will take forever to import (well over a week), and will take over 100 GB more than it would without the indexes.

```sh
wget https://dumps.wikimedia.org/enwiki/latest/enwiki-latest-pagelinks.sql.gz
# this will take around 7 or 8 hours
pv enwiki-latest-pagelinks.sql.gz | zcat | mysql wiki
```

As this is importing, as soon as possible, fire up a SQL REPL with `mysql wiki` and enter this:

```sql
-- This will cause the inserts on pagelinks to block while the entire table is copied, so don't wait long to start it!
ALTER TABLE pagelinks DROP PRIMARY KEY;
ALTER TABLE pagelinks DROP INDEX pl_namespace;
ALTER TABLE pagelinks DROP INDEX pl_backlinks_namespace;
```

Wait until the pagelinks table is imported entirely. Clean up all the non-zero-namespace links (we don't care about talk pages, user pages, user talk pages, etc.):

```sql
DELETE FROM vertexes WHERE page_namespace <> 0;
ALTER TABLE vertexes DROP COLUMN page_namespace;

ALTER TABLE pagelinks ADD INDEX pl_namespace_index (pl_namespace);

SET @@AUTOCOMMIT=0;
LOCK TABLES pagelinks WRITE;
DELETE FROM pagelinks WHERE pl_from_namespace <> 0 OR pl_namespace <> 0;
UNLOCK TABLES;
COMMIT;
ALTER TABLE pagelinks DROP INDEX pl_namespace_index;

ALTER TABLE pagelinks ADD INDEX pl_from_namespace_index (pl_from_namespace);
LOCK TABLES pagelinks WRITE;
DELETE FROM pagelinks WHERE pl_from_namespace <> 0;
UNLOCK TABLES;
COMMIT;
ALTER TABLE pagelinks DROP INDEX pl_from_namespace_index;
-- ALTER TABLE pagelinks ADD INDEX pl_title_index (pl_title);
```

Populate the edges table

```sql
CREATE TABLE `edges` (
  `source_page_id` int(8) unsigned NOT NULL,
  `dest_page_id` int(8) unsigned NOT NULL,
  PRIMARY KEY (`source_page_id`,`dest_page_id`)
) ENGINE=InnoDB;

INSERT INTO edges (source_page_id, dest_page_id)
     SELECT pl.pl_from, v.page_id
       FROM pagelinks pl
 INNER JOIN vertexes v
         ON v.page_title = pl.pl_title; 
```