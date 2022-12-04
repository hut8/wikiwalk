# wikipedia-speedrun

Shortest path algorithm between pages on Wikipedia. Let's say that you want to get from page `Exploding animal` to `Cucumber`. This will do that:

```sh
wikipedia-speedrun 'Exploding animal' 'Cucumber'
```

## Database

There are two files that comprise the adjacency list database of edges:

### vertex_al

Vertex Adjacency List

An array of records of `edges` in this format:

```none
null        ⇒ 0 as u32
vertex      ⇒ u32
vertexes    ⇒ vertex+
edges       ⇒ outgoing_vertexes, null, incoming_vertexes, null
```

To determine which vertex each record belongs to, see below (`vertex_al_ix`)

### vertex_al_ix

Vertex Adjacency List Index

An array of `u32` indexed by vertex ID. Each `u32` is the offset into `vertex_al` at which the edge data is located. So to load the record for the page with ID `1337`:

* Read the eight bytes at offset `1337 * 8` in `vertex_al_ix` ⇒ `offset_bytes`
* Decode `offset_bytes` into a `u64` ⇒ `offset`
* If `offset == 0`, the page either does not exist, or has no edges attached to it.
* Otherwise, read the record at `offset` in `vertex_al` (see above)

## Importing MySQL data for test

If you want to spot-check some data from MySQL, it's faster to import the dumps without indexes. First define the tables thusly:

```sql
CREATE TABLE `pagelinks` (
  `pl_from` int(8) unsigned NOT NULL DEFAULT 0,
  `pl_namespace` int(11) NOT NULL DEFAULT 0,
  `pl_title` varbinary(255) NOT NULL DEFAULT '',
  `pl_from_namespace` int(11) NOT NULL DEFAULT 0
  ) ENGINE=InnoDB DEFAULT CHARSET=binary;

 CREATE TABLE `redirect` (
  `rd_from` int(8) unsigned NOT NULL DEFAULT 0,
  `rd_namespace` int(11) NOT NULL DEFAULT 0,
  `rd_title` varbinary(255) NOT NULL DEFAULT '',
  `rd_interwiki` varbinary(32) DEFAULT NULL,
  `rd_fragment` varbinary(255) DEFAULT NULL
  ) ENGINE=InnoDB DEFAULT CHARSET=binary ROW_FORMAT=COMPRESSED;

 CREATE TABLE `page` (
  `page_id` int(8) unsigned NOT NULL,
  `page_namespace` int(11) NOT NULL DEFAULT 0,
  `page_title` varbinary(255) NOT NULL DEFAULT '',
  `page_is_redirect` tinyint(1) unsigned NOT NULL DEFAULT 0,
  `page_is_new` tinyint(1) unsigned NOT NULL DEFAULT 0,
  `page_random` double unsigned NOT NULL DEFAULT 0,
  `page_touched` binary(14) NOT NULL,
  `page_links_updated` varbinary(14) DEFAULT NULL,
  `page_latest` int(8) unsigned NOT NULL DEFAULT 0,
  `page_len` int(8) unsigned NOT NULL DEFAULT 0,
  `page_content_model` varbinary(32) DEFAULT NULL,
  `page_lang` varbinary(35) DEFAULT NULL
) ENGINE=InnoDB DEFAULT CHARSET=binary;
```

Then read in dumps, skipping DDL

```sh
pv enwiki-*-pagelinks.sql | tail +39 | mysql wiki
pv enwiki-*-page.sql | tail +45 | mysql wiki
pv enwiki-*-redirect.sql | tail +38 | mysql wiki
```

Then build indexes

```sql
create index page_title_ix on page (page_title);
create index page_title_id on page(page_id);
create index page_title_namespace on page(page_namespace);
```

To export for later:

```sql
select * from page into outfile '/tmp/wiki-page-dump';
select * from pagelinks into outfile '/tmp/wiki-pagelinks-dump';
select * from redirect into outfile '/tmp/wiki-redirect-dump';
```

Then compress and such:

```sh
sudo mv /tmp/wiki-*-dump ~/data
sudo chown $(id -u):$(id -g) ~/data/wiki-*-dump
zstd -T0 ~/data/wiki-*-dump
```

Then to import (assuming wiki-page-dump is on the server at some location):

```sql
LOAD DATA INFILE 'wiki-page-dump' INTO TABLE page;
LOAD DATA INFILE 'wiki-pagelinks-dump' INTO TABLE pagelinks;
LOAD DATA INFILE 'wiki-redirect-dump' INTO TABLE redirect;
```
