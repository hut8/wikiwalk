use std::{fs::{OpenOptions, File}, io::{BufWriter, Seek, Write}, path::PathBuf};

use memmap2::Mmap;
use rayon::slice::ParallelSliceMut;
use wikiwalk::{Edge, edge_db};
use memmap2::MmapMut;

use crate::EdgeSort;


// flat file database for sorting/aggregating edges
pub struct EdgeProcDB {
  /// directory containing raw edge list and both sorted files
  root_path: PathBuf,
  writer: BufWriter<File>,
  fail_writer: BufWriter<File>,
  unflushed_inserts: usize,
}

impl EdgeProcDB {
  pub fn new(path: PathBuf) -> EdgeProcDB {
      std::fs::create_dir_all(&path).expect("create edge db directory");
      let file = OpenOptions::new()
          .read(true)
          .write(true)
          .create(true)
          .open(path.join("edges"))
          .expect("open edge proc db file");
      let fail_log = OpenOptions::new()
          .read(true)
          .write(true)
          .create(true)
          .open(path.join("edges_fail.csv"))
          .expect("open edge proc db fail file");
      EdgeProcDB {
          root_path: path,
          writer: BufWriter::new(file),
          unflushed_inserts: 0,
          fail_writer: BufWriter::new(fail_log),
      }
  }

  pub fn truncate(self) -> Self {
      let mut file = self.writer.into_inner().unwrap();
      file.set_len(0).unwrap();
      file.rewind().unwrap();
      let mut fail_file = self.fail_writer.into_inner().unwrap();
      fail_file.set_len(0).unwrap();
      fail_file.rewind().unwrap();

      EdgeProcDB {
          root_path: self.root_path,
          writer: BufWriter::new(file),
          unflushed_inserts: 0,
          fail_writer: BufWriter::new(fail_file),
      }
  }

  pub fn write_edge(&mut self, edge: &Edge) {
      let edge_ptr = edge as *const Edge as *const u8;
      let edge_slice =
          unsafe { std::slice::from_raw_parts(edge_ptr, std::mem::size_of::<Edge>()) };
      self.writer.write_all(edge_slice).expect("write edge");
      self.unflushed_inserts += 1;
      if self.unflushed_inserts % 1024 == 0 {
          self.unflushed_inserts = 0;
          self.writer.flush().expect("flush edge proc db");
      }
  }

  pub fn write_fail(&mut self, source_vertex_id: u32, dest_page_title: String) {
      let line = format!("{source_vertex_id},{dest_page_title}\n");
      self.fail_writer
          .write_all(line.as_bytes())
          .expect("write edge fail");
  }

  fn sort_basename(sort_by: &EdgeSort) -> String {
      format!(
          "edges-{}",
          match sort_by {
              EdgeSort::Incoming => "incoming",
              EdgeSort::Outgoing => "outgoing",
          }
      )
  }

  fn open_sort_file_write(&self, sort_by: &EdgeSort) -> MmapMut {
      let basename = Self::sort_basename(sort_by);
      let path = &self.root_path.join(basename);
      let source_file = OpenOptions::new()
          .read(true)
          .write(true)
          .open(path)
          .expect("open edge db sort file for writing");
      let map = unsafe { MmapMut::map_mut(&source_file).expect("mmap edge sort file") };
      Self::configure_mmap_mut(&map);
      map
  }

  fn open_sort_file_read(&self, sort_by: &EdgeSort) -> Mmap {
      let basename = Self::sort_basename(sort_by);
      let path = &self.root_path.join(basename);
      let source_file = OpenOptions::new()
          .read(true)
          .open(path)
          .expect("open edge db sort file as source");
      let map = unsafe { Mmap::map(&source_file).expect("mmap edge sort file") };
      Self::configure_mmap(&map);
      map
  }

  #[cfg(unix)]
  fn configure_mmap(mmap: &Mmap) {
      mmap.advise(memmap2::Advice::sequential())
          .expect("set madvice sequential");
  }

  #[cfg(unix)]
  fn configure_mmap_mut(mmap: &MmapMut) {
      mmap.advise(memmap2::Advice::sequential())
          .expect("set madvice sequential");
  }

  #[cfg(windows)]
  /// configure_mmap is a nop in Windows
  fn configure_mmap(_mmap: &Mmap) {}

  #[cfg(windows)]
  /// configure_mmap is a nop in Windows
  fn configure_mmap_mut(_mmap: &MmapMut) {}

  pub fn make_sort_files(&self) {
      let source_path = self.root_path.join("edges");
      let incoming_sink_basename = Self::sort_basename(&EdgeSort::Incoming);
      let incoming_sink_path = &self.root_path.join(incoming_sink_basename);
      let outgoing_sink_basename = Self::sort_basename(&EdgeSort::Outgoing);
      let outgoing_sink_path = &self.root_path.join(outgoing_sink_basename);
      std::fs::copy(&source_path, outgoing_sink_path).expect("copy file for sort");
      std::fs::rename(&source_path, incoming_sink_path).expect("rename file for sort");
  }

  pub fn destroy(&self) {
      std::fs::remove_dir_all(&self.root_path).expect("remove edge proc db directory");
  }

  pub fn write_sorted_by(&mut self, sort_by: EdgeSort) {
      let mut sink = self.open_sort_file_write(&sort_by);

      log::debug!(
          "sorting edge db for direction: {}",
          match sort_by {
              EdgeSort::Incoming => "incoming",
              EdgeSort::Outgoing => "outgoing",
          }
      );
      let slice = &mut sink[..];
      let sink_byte_len = slice.len();
      let edges_ptr = slice.as_mut_ptr() as *mut Edge;
      let edges_len = sink_byte_len / std::mem::size_of::<Edge>();
      let edges = unsafe { std::slice::from_raw_parts_mut(edges_ptr, edges_len) };
      let sink_edge_len = edges.len();
      log::debug!("sink byte len={}", sink_byte_len);
      log::debug!("size of edge={}", std::mem::size_of::<Edge>());
      log::debug!("edge count={}", sink_edge_len);

      edges.par_sort_unstable_by(|x, y| match sort_by {
          EdgeSort::Incoming => x.dest_vertex_id.cmp(&y.dest_vertex_id),
          EdgeSort::Outgoing => x.source_vertex_id.cmp(&y.source_vertex_id),
      });
      drop(sink);
  }

  pub fn flush(&mut self) {
      self.writer.flush().expect("flush edge db");
  }

  pub fn iter(&self, max_page_id: u32) -> AdjacencySetIterator {
      let outgoing_source = self.open_sort_file_read(&EdgeSort::Outgoing);
      let incoming_source = self.open_sort_file_read(&EdgeSort::Incoming);

      AdjacencySetIterator {
          outgoing_source,
          incoming_source,
          incoming_i: 0,
          outgoing_i: 0,
          vertex_id: 0,
          max_page_id,
      }
  }
}

// AdjacencySet is an AdjacencyList combined with its vertex
pub struct AdjacencySet {
  pub adjacency_list: edge_db::AdjacencyList,
}

pub struct AdjacencySetIterator {
  incoming_source: Mmap,
  outgoing_source: Mmap,
  incoming_i: usize,
  outgoing_i: usize,
  vertex_id: u32,
  max_page_id: u32,
}

impl Iterator for AdjacencySetIterator {
  type Item = AdjacencySet;

  // iterates over range of 0..max_page_id,
  // combining data in incoming_source and outgoing_source
  // into adjacency lists
  fn next(&mut self) -> Option<Self::Item> {
      // are we done yet?
      if self.vertex_id > self.max_page_id {
          log::debug!(
              "adjacency set iter: done after {} iterations",
              self.max_page_id
          );
          return None;
      }

      let mut val = AdjacencySet {
          adjacency_list: edge_db::AdjacencyList::default(),
      };

      // put in all the outgoing edges
      // outgoing source is sorted by source vertex id
      loop {
          let outgoing_offset: usize = self.outgoing_i * std::mem::size_of::<Edge>();
          if outgoing_offset >= self.outgoing_source.len() {
              break;
          }

          let current_edge: Edge = Edge::from_bytes(
              &self.outgoing_source
                  [outgoing_offset..outgoing_offset + std::mem::size_of::<Edge>()],
          );

          if current_edge.source_vertex_id > self.vertex_id {
              break;
          }
          if current_edge.source_vertex_id < self.vertex_id {
              panic!("current edge source vertex id={} is before current vertex id={}; edge was missed",
                     current_edge.source_vertex_id, self.vertex_id);
          }

          if current_edge.dest_vertex_id > self.max_page_id {
              panic!(
                  "destination vertex id for edge: {:#?} is greater than max page id {}",
                  current_edge, self.max_page_id
              );
          }
          val.adjacency_list
              .outgoing
              .push(current_edge.dest_vertex_id);
          self.outgoing_i += 1;
      }

      // put in all the incoming edges
      // incoming source is sorted by destination vertex id
      loop {
          let incoming_offset: usize = self.incoming_i * std::mem::size_of::<Edge>();
          if incoming_offset >= self.incoming_source.len() {
              break;
          }

          let current_edge: Edge = Edge::from_bytes(
              &self.incoming_source
                  [incoming_offset..incoming_offset + std::mem::size_of::<Edge>()],
          );

          if current_edge.dest_vertex_id > self.vertex_id {
              break;
          }

          if current_edge.dest_vertex_id < self.vertex_id {
              panic!("current edge dest vertex id={} is before current vertex id={}; edge was missed",
                     current_edge.dest_vertex_id, self.vertex_id);
          }

          if current_edge.source_vertex_id > self.max_page_id {
              panic!(
                  "source vertex id for edge: {:#?} is greater than max page id {}",
                  current_edge, self.max_page_id
              );
          }

          val.adjacency_list
              .incoming
              .push(current_edge.source_vertex_id);
          self.incoming_i += 1;
      }

      self.vertex_id += 1;

      Some(val)
  }
}
