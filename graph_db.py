#!/usr/bin/python3
import struct
import csv
import sys
import os.path
import unittest
import tempfile
from io import StringIO
from tqdm import tqdm

MAX_VERTEX = 69959490

index_rec = struct.Struct('>Q')
edge_rec = struct.Struct('>L')

class TestVertedDB(unittest.TestCase):
    def test_build_vertex_db(self):
        src = csv.reader(StringIO(TEST_SRC), delimiter='\t')
        with tempfile.TemporaryDirectory() as tempdir:
            al_ix_path = os.path.join(tempdir, 'al_ix')
            al_file_path = os.path.join(tempdir, 'al')
            al_ix_file = open(al_ix_path, 'wb')
            al_file = open(al_file_path, 'wb')
            build_vertex_db(src, al_ix_file, al_file, progress=False)

            # assertions
            self.assertEqual(edge_rec.size, 4, "edge rec should be 32 bits")
            self.assertEqual(index_rec.size, 8, "index rec should be 64 bits")
            al_ix_file = open(al_ix_path, 'rb')
            al_ix_data = al_ix_file.read()
            al_file = open(al_file_path, 'rb')
            al_data = al_file.read()

            (magic,) = edge_rec.unpack(al_data[0:4])
            self.assertEqual(1337, magic, "first record in al should be 1337")
            # 1 should point to 6 and 7
            (vertex_1_ix,) = index_rec.unpack(
                al_ix_data[(index_rec.size*1):(index_rec.size*2)]
            )
            (vertex_1_edge_1,) = edge_rec.unpack(
                al_data[vertex_1_ix:(vertex_1_ix+edge_rec.size)]
            )
            self.assertEqual(vertex_1_edge_1, 6,
                             "first edge of vertex 1 should point to 6")
            vertex_1_ix += edge_rec.size
            (vertex_1_edge_2,) = edge_rec.unpack(
                al_data[vertex_1_ix:(vertex_1_ix+edge_rec.size)]
            )
            self.assertEqual(vertex_1_edge_2, 7,
                             "second edge of vertex 1 should point to 7")
            vertex_1_ix += edge_rec.size
            (vertex_1_edge_null,) = edge_rec.unpack(
                al_data[vertex_1_ix:(vertex_1_ix+edge_rec.size)]
            )
            self.assertEqual(vertex_1_edge_null, 0,
                             "vertex 1 should have null terminator after second edge")


class NullProgress(object):
    def __init__(self, *args, **kwargs):
        pass
    def update(self, *args):
        pass
    def close(self):
        pass


def build_vertex_db(src, al_ix_file, al_file, progress=True):
    # 0 is used as a null value in the index to denote
    # there is no list to point to. so we cannot have a
    # valid list at 0. this puts a magic number at the 0
    # position so our real lists start at 4
    al_file.write(edge_rec.pack(1337))
    last_id = None
    # pb = tqdm(total=6446923)
    pb = tqdm(total=MAX_VERTEX)
    if not progress:
        pb = NullProgress()
    for vid, edges_str in src:
        vid = int(vid)
        edges = [int(x) for x in edges_str.split(',')]
        if last_id is None:
            last_id = -1
        # if this isn't sorted, we're in big trouble
        assert(last_id < vid)
         # don't write zero padding for existing record
        last_id += 1
        # zero pad between last id and this id
        # if last_id and vid are the same (because of increment),
        # nothing gets written
        for null_id in range(last_id, vid):
            actual_index_offset = al_ix_file.tell()
            expected_index_offset = 8 * null_id # 64-bit entries
            errmsg = f"padding from {last_id} to {vid} (current: {null_id}) with zeroes at wrong place: " +\
                f"expected: {expected_index_offset}; actual: {actual_index_offset}"
            assert actual_index_offset == expected_index_offset, errmsg
            al_ix_file.write(index_rec.pack(0))
            pb.update(1)
        last_id = vid
        # write list of adjacent vertexes
        al_pos = al_file.tell()
        for edge in edges:
            al_file.write(edge_rec.pack(edge))
        expected_index_offset = 8 * vid # 64-bit entries
        # bail if we're about to write the offset to the wrong place
        actual_index_offset = al_ix_file.tell()
        errmsg = f"about to write index record for {vid} at {actual_index_offset} instead of {expected_index_offset}"
        assert expected_index_offset == actual_index_offset, errmsg
        if len(edges) == 0:
            # optimization: existing vertexes with no edges
            # get no entry in database
            al_ix_file.write(index_rec.pack(0))
            pb.update(1)
        else:
            # record beginning of adjacency list in index
            # and add null terminator
            al_ix_file.write(index_rec.pack(al_pos))
            al_file.write(edge_rec.pack(0))
            pb.update(1)

    al_ix_file.close()
    al_file.close()
    pb.close()


def main():
    al_ix_path = os.path.expanduser('~/wpsr/vertex_al_ix')
    al_file_path = os.path.expanduser('~/wpsr/vertex_al')
    for f in [al_ix_path, al_file_path]:
        if os.path.exists(f):
            print(f"vertex database file already exists: {f}")
            print(f"this script will not overwrite these files:\n\t{al_ix_path}\n\t{al_file_path}")
            print(f"to build database, first delete existing files")
            sys.exit(1)
    al_ix_file = open(al_ix_path, 'wb')
    al_file = open(al_file_path, 'wb')
    src = csv.reader(sys.stdin, delimiter='\t')
    build_vertex_db(src, al_ix_file, al_file)

if __name__ == '__main__':
    main()

TEST_SRC="""1	6,7
4	9
5	4
6	4
7	9
9	1,4,5
"""
