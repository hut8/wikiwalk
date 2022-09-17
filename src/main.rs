use clap::{Parser, Subcommand};
use dirs;
use indicatif::ProgressIterator;
use memmap2::{Mmap, MmapOptions};
use num_cpus;
use spinners::{Spinner, Spinners};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{prelude::*, BufWriter};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

mod dump;

#[derive(Debug, Clone)]
pub struct Vertex {
    pub id: u32,
    pub title: String,
}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Vertex {}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Edge {
    pub source_vertex_id: u32,
    pub dest_vertex_id: u32,
}

pub struct Link<'a> {
    pub source: &'a Vertex,
    pub dest: &'a Vertex,
}

struct GraphDBBuilder {
    // inputs
    pub page_path: PathBuf,
    pub pagelinks_path: PathBuf,
    // outputs
    vertex_path: PathBuf,
    pub ix_path: PathBuf,
    pub al_path: PathBuf,
    al_file: File,
    ix_file: File,
    vertex_file: File,
}

impl GraphDBBuilder {
    pub fn new(
        page: PathBuf,
        pagelinks: PathBuf,
        ix_path: PathBuf,
        al_path: PathBuf,
        vertex_path: PathBuf,
    ) -> GraphDBBuilder {
        let ix_file = match File::create(&ix_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &ix_path, why),
            Ok(file) => file,
        };
        let al_file = match File::create(&al_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &al_path, why),
            Ok(file) => file,
        };
        let vertex_file = match File::create(&vertex_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &vertex_path, why),
            Ok(file) => file,
        };

        GraphDBBuilder {
            page_path: page,
            pagelinks_path: pagelinks,
            ix_path,
            al_path,
            vertex_path,
            al_file,
            ix_file,
            vertex_file,
        }
    }

    pub fn build_database(&mut self) {
        log::info!("loading page.sql");
        let mut vertexes = self.load_vertexes_dump();
        log::debug!("finding max index");
        vertexes.sort_by(|x, y| x.id.cmp(&y.id));
        let max_page = vertexes.last().unwrap();
        log::debug!("max page: {:#?}", max_page);

        log::debug!("writing vertexes to {}", self.vertex_path.display());
        self.build_vertex_file(&vertexes);
        log::debug!("writing vertexes complete");

        let title_map = self.build_title_map(&vertexes);
        let edge_map = self.build_edge_map(&title_map);

        log::debug!(
            "building al [{}] and ix [{}] - {} vertexes",
            self.al_path.to_str().unwrap(),
            self.ix_path.to_str().unwrap(),
            max_page.id,
        );

        for n in 0..max_page.id {
            let vertex_al_offset: u64 = self.build_adjacency_list(n, &edge_map);
            self.ix_file.write(&vertex_al_offset.to_le_bytes()).unwrap();
            if n % 1000 == 0 {
                log::debug!("-> wrote {} entries", n);
            }
        }
        self.ix_file.flush().unwrap();
        self.al_file.flush().unwrap();
        log::info!("database build complete")
    }

    fn build_vertex_file(&mut self, vertexes: &Vec<Vertex>) {
        let mut writer = BufWriter::new(&self.vertex_file);
        for v in vertexes.iter() {
            write!(&mut writer, "{}\t{}\n", v.id, v.title).unwrap();
        }
        writer.flush().unwrap()
    }

    fn build_title_map(&mut self, vertexes: &Vec<Vertex>) -> HashMap<String, u32> {
        let mut m = HashMap::new();
        for v in vertexes.iter() {
            m.insert(v.title.clone(), v.id);
        }
        m
    }

    // builds outgoing edges
    fn build_edge_map(&self, title_map: &HashMap<String, u32>) -> HashMap<u32, Vec<u32>> {
        let mut m = HashMap::new();
        log::debug!("loading edges from dump");
        let edges = Self::load_edges_dump(&self.pagelinks_path, &title_map);
        for edge in edges.iter() {
            if !m.contains_key(&edge.source_vertex_id) {
                m.insert(edge.source_vertex_id, vec![]);
            }
            m.get_mut(&edge.source_vertex_id)
                .unwrap()
                .push(edge.dest_vertex_id);
        }
        m
    }

    // load edges from the pagelinks.sql dump
    fn load_edges_dump(
        path: &PathBuf,
        title_map: &HashMap<String, u32>,
    ) -> Vec<Edge> {
        use parse_mediawiki_sql::{
            utils::memory_map,
        };
        let pagelinks_sql = unsafe { Arc::new(memory_map(path).unwrap()) };

        let chunks = dump::dump_chunks(&pagelinks_sql);
        let mut threads = Vec::new();

        for chunk in chunks.into_iter() {
            let pagelinks_ref = Arc::clone(&pagelinks_sql);
            let t = thread::spawn(move || Self::load_edges_dump_chunk(&pagelinks_ref[chunk]));
            threads.push(t);
        }

        let mut links = Vec::new();
        for t in threads.into_iter() {
            log::debug!("joining on thread {:#?}", t);
            let mut res = t.join().unwrap();
            links.append(&mut res);
        }
        log::info!("loaded pagelinks");

        log::info!("linking via title");
        let edges = links
            .into_iter()
            .filter_map(|pl| match title_map.get(&pl.1 .0) {
                Some(dest_id) => Some(Edge {
                    source_vertex_id: pl.0 .0,
                    dest_vertex_id: *dest_id,
                }),
                None => None,
            });
        edges.collect()
    }

    fn load_edges_dump_chunk(chunk: &[u8]) -> Vec<(parse_mediawiki_sql::field_types::PageId, parse_mediawiki_sql::field_types::PageTitle)> {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
        };

        let links = iterate_sql_insertions(&chunk)
            .filter_map(
                |PageLink {
                     from,
                     from_namespace,
                     namespace,
                     title,
                 }| {
                    if from_namespace == PageNamespace(0) && namespace == PageNamespace(0) {
                        Some((from, title))
                    } else {
                        None
                    }
                },
            )
            .collect::<Vec<_>>();
            links
    }

    // load vertexes from the pages.sql dump
    fn load_vertexes_dump(&mut self) ->Vec<Vertex> {
        // let pb = ProgressBar::new(1024);
        use parse_mediawiki_sql::{
            utils::memory_map,
        };

        let page_sql = unsafe { Arc::new(memory_map(&self.page_path).unwrap()) };
        let chunks = dump::dump_chunks(&page_sql);
        let mut threads = Vec::new();

        for chunk in chunks.into_iter() {
            let page_ref = Arc::clone(&page_sql);
            let t = thread::spawn(move || Self::load_vertex_dump_chunk(&page_ref[chunk]));
            threads.push(t);
        }

        let mut vertexes: Vec<Vertex> = Vec::new();
        for t in threads.into_iter() {
            log::debug!("joining on thread {:#?}", t);
            let mut res = t.join().unwrap().unwrap();
            vertexes.append(&mut res);
        }
        vertexes
    }

    fn load_vertex_dump_chunk(chunk: &[u8]) -> Result<Vec<Vertex>, parse_mediawiki_sql::utils::Error> {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::Page,
        };
        let vertexes = iterate_sql_insertions(chunk)
            .filter_map(
                |Page {
                     id,
                     namespace,
                     title,
                     ..
                 }| {
                    if namespace == PageNamespace(0) {
                        Some(Vertex {
                            id: id.0,
                            title: title.0,
                        })
                    } else {
                        None
                    }
                },
            )
            .collect::<Vec<_>>();
        Ok(vertexes)
    }

    /// Writes appropriate null-terminated list of 4-byte values to al_file
    /// Each 4-byte value is a LE representation
    pub fn build_adjacency_list(
        &mut self,
        vertex_id: u32,
        edge_map: &HashMap<u32, Vec<u32>>,
    ) -> u64 {
        let edge_ids = edge_map.get(&vertex_id).unwrap();
        if edge_ids.len() == 0 {
            // No outgoing edges or no such vertex
            return 0;
        }

        // Position at which we are writing the thing.
        let al_position = self.al_file.stream_position().unwrap();
        log::debug!(
            "writing vertex {} list with {} edges {}",
            vertex_id,
            edge_ids.len(),
            al_position
        );
        for neighbor in edge_ids.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            self.al_file.write(&neighbor_bytes).unwrap();
        }
        // Null terminator
        self.al_file.write(&(0i32).to_le_bytes()).unwrap();
        al_position
    }
}

pub struct GraphDB {
    pub mmap_ix: memmap2::Mmap,
    pub mmap_al: memmap2::Mmap,
    pub vertex_file: File,
    pub visited_ids: HashSet<u32>,
    pub parents: HashMap<u32, u32>,
    pub q: VecDeque<u32>,
}

impl GraphDB {
    pub fn new(path_ix: &str, path_al: &str, path_vertex: &str) -> Result<GraphDB, std::io::Error> {
        let file_ix = File::open(path_ix)?;
        let file_al = File::open(path_al)?;
        let vertex_file = File::open(path_vertex)?;
        let mmap_ix = unsafe { MmapOptions::new().map(&file_ix)? };
        let mmap_al = unsafe { MmapOptions::new().map(&file_al)? };
        let visited_ids = HashSet::new();
        let parents = HashMap::new();
        let q: VecDeque<u32> = VecDeque::new();
        Ok(GraphDB {
            mmap_ix,
            mmap_al,
            vertex_file,
            visited_ids,
            parents,
            q,
        })
    }

    pub fn find_vertex_by_title(&mut self, title: String) -> Option<Vertex> {
        let canon_title = title.to_lowercase();
        log::debug!("loading vertex: {}", canon_title);
        self.vertex_file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let reader = BufReader::new(&self.vertex_file);
        reader.lines().find_map(|l| {
            let parts = l.as_ref().unwrap().split("\t").collect::<Vec<&str>>();
            if parts.len() != 2 {
                panic!("invalid line in vertex file: {}", l.unwrap());
            }
            if parts[1] == canon_title {
                let vertex_id: u32 = parts[0].parse().unwrap();
                Some(Vertex {
                    id: vertex_id,
                    title: parts[1].to_owned(),
                })
            } else {
                None
            }
        })
    }

    pub fn find_vertex_by_id(&mut self, id: u32) -> Option<Vertex> {
        log::debug!("loading vertex: id={}", id);
        self.vertex_file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let reader = BufReader::new(&self.vertex_file);
        reader.lines().find_map(|l| {
            let parts = l.as_ref().unwrap().split("\t").collect::<Vec<&str>>();
            if parts.len() != 2 {
                panic!("invalid line in vertex file: {}", l.unwrap());
            }
            let record_id: u32 = parts[0].parse().unwrap();
            if record_id == id {
                Some(Vertex {
                    id,
                    title: parts[1].to_owned(),
                })
            } else {
                None
            }
        })
    }

    fn check_al(&mut self) {
        let mut buf: [u8; 4] = [0; 4];
        buf.copy_from_slice(&self.mmap_al[0..4]);
        let magic: u32 = u32::from_be_bytes(buf);
        assert!(magic == 1337);
    }

    fn check_ix(&mut self) {
        // read index file and ensure that all 64-bit entries
        // point to within range
        let max_sz: u64 = (self.mmap_al.len() - 4) as u64;
        let mut buf: [u8; 8] = [0; 8];
        let mut position: usize = 0;
        while position <= (self.mmap_ix.len() - 8) {
            buf.copy_from_slice(&self.mmap_ix[position..position + 8]);
            let value: u64 = u64::from_be_bytes(buf);
            if value > max_sz {
                let msg = format!(
                    "check_ix: at index file: {}, got pointer to {} in AL file (maximum: {})",
                    position, value, max_sz
                );
                panic!("{}", msg);
            }
            position += 8;
        }
    }

    fn check_db(&mut self) {
        self.check_al();
        println!("checking index file");
        self.check_ix();
        println!("done");
    }

    fn build_path(&self, source: u32, dest: u32) -> Vec<u32> {
        let mut path: Vec<u32> = Vec::new();
        let mut current = dest;
        loop {
            path.push(current);
            if current == source {
                break;
            }
            current = *self
                .parents
                .get(&current)
                .expect(&format!("parent not recorded for {:#?}", current));
        }
        path.reverse();
        path
    }

    pub fn bfs(&mut self, src: u32, dest: u32) -> Option<Vec<u32>> {
        self.check_db();
        let mut sp = Spinner::new(Spinners::Dots9, "Computing path".into());

        let start_time = Instant::now();
        self.q.push_back(src);
        loop {
            match self.q.pop_front() {
                Some(current) => {
                    if current == dest {
                        sp.stop_with_message(format!(
                            "Computed path - visited {} pages",
                            self.visited_ids.len()
                        ));
                        let path = self.build_path(src, dest);
                        let elapsed = start_time.elapsed();
                        println!("\nelapsed time: {} seconds", elapsed.as_secs());
                        return Some(path);
                    }
                    let neighbors = self.load_neighbors(current);
                    let next_neighbors: Vec<u32> = neighbors
                        .into_iter()
                        .filter(|x| !self.visited_ids.contains(x))
                        .collect();
                    for &n in next_neighbors.iter() {
                        self.parents.insert(n, current);
                        self.visited_ids.insert(n);
                        self.q.push_back(n);
                    }
                }
                None => {
                    sp.stop();
                    let elapsed = start_time.elapsed();
                    println!("\nelapsed time: {} seconds", elapsed.as_secs());
                    return None;
                }
            }
        }
    }

    pub fn load_neighbors(&self, vertex_id: u32) -> Vec<u32> {
        let mut neighbors: Vec<u32> = Vec::new();
        let ix_position: usize = ((u64::BITS / 8) * vertex_id) as usize;
        // println!(
        //     "load_neighbors for {} from ix position: {}",
        //     vertex_id, ix_position
        // );
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(&self.mmap_ix[ix_position..ix_position + 8]);
        // println!("buf from ix = {:?}", buf);
        let mut al_offset: usize = u64::from_be_bytes(buf) as usize;
        if al_offset == 0 {
            // println!("vertex {} has no neighbors", vertex_id);
            return neighbors;
        }
        let mut vbuf: [u8; 4] = [0; 4];
        loop {
            // println!("looking at al_offset = {}", al_offset);
            vbuf.copy_from_slice(&self.mmap_al[al_offset..al_offset + 4]);
            // println!("vbuf from al = {:?}", vbuf);
            let i: u32 = u32::from_be_bytes(vbuf);
            // println!("vbuf -> int = {:?}", i);
            if i == 0 {
                break;
            }
            neighbors.push(i);
            al_offset += 4;
        }
        neighbors
    }
}

fn format_path(vertexes: Vec<Vertex>) -> String {
    let titles: Vec<String> = vertexes.into_iter().map(|v| v.title).collect();
    titles.join(" → ")
}

/// CLI Options
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Verbose output
    #[clap(short, long)]
    verbose: bool,
    /// Path to database
    #[clap(short, long)]
    data_path: Option<PathBuf>,
    /// Command to execute
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build the database from a MediaWiki Dump
    /// https://dumps.wikimedia.org/enwiki/latest/
    Build {
        /// Path to page.sql
        #[clap(long)]
        page: PathBuf,
        /// Path to pagelinks.sql
        #[clap(long)]
        pagelinks: PathBuf,
    },
    /// Find the shortest path
    Run {
        /// Source article
        source: String,
        /// Destination article
        destination: String,
    },
}

fn main() {
    stderrlog::new()
        .module(module_path!())
        .quiet(false)
        .verbosity(4)
        //    .timestamp(ts)
        .init()
        .unwrap();

    log::info!("Wikipedia Speedrun");
    let cli = Cli::parse();

    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("speedrun-data");
    let data_dir = cli.data_path.unwrap_or(default_data_dir);
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let vertex_path = data_dir.join("vertexes");
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_al_ix");

    match cli.command {
        Command::Build { page, pagelinks } => {
            log::info!("building database");
            let mut gddb =
                GraphDBBuilder::new(page, pagelinks, vertex_ix_path, vertex_al_path, vertex_path);
            gddb.build_database();
        }
        Command::Run {
            source,
            destination,
        } => {
            log::info!("computing path");
            let mut gdb = GraphDB::new(
                vertex_ix_path.to_str().unwrap(),
                vertex_al_path.to_str().unwrap(),
                vertex_path.to_str().unwrap(),
            )
            .unwrap();
            let source_title = source.replace(" ", "_").to_lowercase();
            let dest_title = destination.replace(" ", "_").to_lowercase();

            log::info!("speedrun: [{}] → [{}]", source_title, dest_title);

            let source_vertex = gdb
                .find_vertex_by_title(source_title)
                .expect("source not found");
            let dest_vertex = gdb
                .find_vertex_by_title(dest_title)
                .expect("destination not found");

            log::info!("speedrun: [{:#?}] → [{:#?}]", source_vertex, dest_vertex);

            match gdb.bfs(source_vertex.id as u32, dest_vertex.id as u32) {
                Some(path) => {
                    let vertex_path = path
                        .iter()
                        .map(|vid| gdb.find_vertex_by_id(*vid).unwrap())
                        .collect();
                    let formatted_path = format_path(vertex_path);
                    println!("\n{}", formatted_path);
                }
                None => {
                    println!("\nno path found");
                }
            }
        }
    }
}
