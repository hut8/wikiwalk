# wikipedia-speedrun

Shortest path algorithm between pages on Wikipedia. Let's say that you want to get from page `Exploding animal` to `Cucumber`. This will do that:

```sh
wikipedia-speedrun 'Exploding animal' 'Cucumber'
```

## Database

There are two files that comprise the adjacency list database of edges:

**vertex_al**

Vertex Adjacency List

An array of records of `edges` in this format:

```
null        ⇒ 0 as u32
vertex      ⇒ u32
vertexes    ⇒ vertex+
edges       ⇒ outgoing_vertexes, null, incoming_vertexes, null
```

To determine which vertex each record belongs to, see below (`vertex_al_ix`)

**vertex_al_ix**

Vertex Adjacency List Index

An array of `u32` indexed by vertex ID. Each `u32` is the offset into `vertex_al` at which the edge data is located. So to load the record for the page with ID `1337`:

* Read the eight bytes at offset `1337 * 8` in `vertex_al_ix` ⇒ `offset_bytes`
* Decode `offset_bytes` into a `u64` ⇒ `offset`
* If `offset == 0`, the page either does not exist, or has no edges attached to it.
* Otherwise, read the record at `offset` in `vertex_al` (see above)
