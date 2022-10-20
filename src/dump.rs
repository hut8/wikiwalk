use std::ops::Range;

// Utitilies for working with dumps
use memmap2::Mmap;

// given an mmap and a start index, find the next newline
// if there is no next newline, return the last possible index
fn next_newline_ix(m: &Mmap, start: usize) -> usize {
    let mut current = start;
    while current < m.len() {
        if m[current] == b'\n' {
            break;
        }
        current += 1;
    }
    current
}

pub fn dump_chunks(sql: &Mmap) -> Vec<Range<usize>> {
    let sql_len = sql.len();
    let cpu_count = num_cpus::get();
    let chunk_size = sql_len / cpu_count;
    log::debug!(
        "sql-length={} cores={} chunk_sz={}",
        sql_len,
        cpu_count,
        chunk_size
    );
    let mut chunk_start: usize = 0;
    let mut chunks = Vec::new();
    for i in 0..cpu_count {
        let far_end = chunk_start + chunk_size;
        let chunk_end = if far_end <= sql_len { far_end } else { sql_len };
        let range_end = next_newline_ix(&sql, chunk_end);
        log::debug!(
            "chunk {}: {} -> chunk_end={} range_end={}",
            i,
            chunk_start,
            chunk_end,
            range_end
        );
        // [chunk_start..range_end]
        chunks.push(chunk_start..range_end);
        chunk_start = range_end + 1;
    }
    chunks
}
