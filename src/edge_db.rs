use memmap2::Mmap;

pub struct EdgeDB {
    vertex_al: Mmap,
    vertex_al_ix: Mmap,
}

impl EdgeDB {
    pub fn new(vertex_al: Mmap, vertex_al_ix: Mmap) -> EdgeDB {
        EdgeDB {
            vertex_al,
            vertex_al_ix,
        }
    }

    pub fn read_edges(&self, vertex_id: u32) -> AdjacencyList {
        let index_offset: usize = ((u64::BITS / 8) * vertex_id) as usize;
        let offset: usize = usize::from_le_bytes(
            self.vertex_al_ix[index_offset..index_offset + 8]
                .try_into()
                .unwrap(),
        );
        AdjacencyList::read(&self.vertex_al[offset..])
    }


    pub fn check_db(&mut self) {
      //self.check_al();
      println!("checking index file");
      self.check_ix();
      println!("done");
  }

    fn check_ix(&mut self) {
      // read index file and ensure that all 64-bit entries
      // point to within range
      let max_sz: u64 = (self.vertex_al.len() - 4) as u64;
      let mut buf: [u8; 8] = [0; 8];
      let mut position: usize = 0;
      while position <= (self.vertex_al_ix.len() - 8) {
          buf.copy_from_slice(&self.vertex_al_ix[position..position + 8]);
          let value: u64 = u64::from_le_bytes(buf);
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

}

#[derive(Debug, Default)]
pub struct AdjacencyList {
    pub outgoing: Vec<u32>,
    pub incoming: Vec<u32>,
}

impl AdjacencyList {
    pub fn read(data: &[u8]) -> AdjacencyList {
        let mut outgoing: Vec<u32> = Vec::new();
        let mut incoming: Vec<u32> = Vec::new();
        let mut i = 0_usize;
        let mut buf: [u8; 4] = [0; 4];
        let mut val: u32 = 0;
        buf.copy_from_slice(&data[..4]);
        val = u32::from_le_bytes(buf);
        if val != 0xCAFECAFE {
            panic!("corrupt database; expected 0xCAFECAFE");
        }
        loop {
            i += 4;
            buf.copy_from_slice(&data[i..i + 4]);
            val = u32::from_le_bytes(buf);
            if val == 0 {
                break;
            }
            outgoing.push(val);
        }
        loop {
            i += 4;
            buf.copy_from_slice(&data[i..i + 4]);
            val = u32::from_le_bytes(buf);
            if val == 0 {
                break;
            }
            incoming.push(val);
        }
        AdjacencyList { outgoing, incoming }
    }

    pub fn is_empty(&self) -> bool {
        self.incoming.is_empty() && self.outgoing.is_empty()
    }
}
