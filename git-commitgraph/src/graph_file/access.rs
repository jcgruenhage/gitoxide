use crate::graph_file::{Kind, LexPosition, COMMIT_DATA_ENTRY_SIZE};
use crate::{CommitData, GraphFile};
use git_object::{borrowed, HashKind, SHA1_SIZE};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Formatter};
use std::path::Path;

impl GraphFile {
    /// Returns the commit data for the commit located at the given lex position.
    ///
    /// `pos` must range from 0 to self.num_commits().
    ///
    /// # Panics
    ///
    /// Panics if `pos` is out of bounds.
    pub fn commit_at(&self, pos: LexPosition) -> CommitData {
        CommitData::new(self, pos)
    }

    pub fn hash_kind(&self) -> HashKind {
        HashKind::Sha1
    }

    // copied from git-odb/src/pack/index/access.rs
    /// Returns 20 bytes sha1 at the given index in our list of (sorted) sha1 hashes.
    /// The position ranges from 0 to self.num_commits()
    pub fn id_at(&self, pos: LexPosition) -> borrowed::Id {
        assert!(
            pos.0 < self.num_commits(),
            "expected lex position less than {}, got {}",
            self.num_commits(),
            pos.0
        );
        let pos: usize = pos
            .0
            .try_into()
            .expect("an architecture able to hold 32 bits of integer");
        let start = self.oid_lookup_offset + (pos * SHA1_SIZE);
        borrowed::Id::try_from(&self.data[start..start + SHA1_SIZE]).expect("20 bytes SHA1 to be alright")
    }

    pub fn iter_base_graph_ids(&self) -> impl Iterator<Item = borrowed::Id> {
        let base_graphs_list = match self.base_graphs_list_offset {
            Some(v) => &self.data[v..v + (SHA1_SIZE * self.base_graph_count as usize)],
            None => &[],
        };
        base_graphs_list
            .chunks_exact(SHA1_SIZE)
            .map(|bytes| borrowed::Id::try_from(bytes).expect("20 bytes SHA1 to be alright"))
    }

    pub fn iter_commits(&self) -> impl Iterator<Item = CommitData> {
        (0..self.num_commits()).map(move |i| self.commit_at(LexPosition(i)))
    }

    pub fn iter_ids(&self) -> impl Iterator<Item = borrowed::Id> {
        (0..self.num_commits()).map(move |i| self.id_at(LexPosition(i)))
    }

    pub fn kind(&self) -> Kind {
        Kind::V1
    }

    // copied from git-odb/src/pack/index/access.rs
    pub fn lookup(&self, id: borrowed::Id) -> Option<LexPosition> {
        let first_byte = id.first_byte() as usize;
        let mut upper_bound = self.fan[first_byte];
        let mut lower_bound = if first_byte != 0 { self.fan[first_byte - 1] } else { 0 };

        // Bisect using indices
        // TODO: Performance of V2 could possibly be better if we would be able to do a binary search
        // on 20 byte chunks directly, but doing so requires transmuting and that is unsafe, even though
        // it should not be if the bytes match up and the type has no destructor.
        while lower_bound < upper_bound {
            let mid = (lower_bound + upper_bound) / 2;
            let mid_sha = self.id_at(LexPosition(mid));

            use std::cmp::Ordering::*;
            match id.cmp(&mid_sha) {
                Less => upper_bound = mid,
                Equal => return Some(LexPosition(mid)),
                Greater => lower_bound = mid + 1,
            }
        }
        None
    }

    /// Returns the number of commits in this graph file.
    ///
    /// The maximum valid `LexPosition` that can be used with this file is one less than
    /// `num_commits()`.
    pub fn num_commits(&self) -> u32 {
        self.fan[255]
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl GraphFile {
    /// Returns the byte slice for the given commit in this file's Commit Data (CDAT) chunk.
    pub(crate) fn commit_data_bytes(&self, pos: LexPosition) -> &[u8] {
        assert!(
            pos.0 < self.num_commits(),
            "expected lex position less than {}, got {}",
            self.num_commits(),
            pos.0
        );
        let pos: usize = pos
            .0
            .try_into()
            .expect("an architecture able to hold 32 bits of integer");
        let start = self.commit_data_offset + (pos * COMMIT_DATA_ENTRY_SIZE);
        &self.data[start..start + COMMIT_DATA_ENTRY_SIZE]
    }

    /// Returns the byte slice for this file's entire Extra Edge List (EDGE) chunk.
    pub(crate) fn extra_edges_data(&self) -> Option<&[u8]> {
        Some(&self.data[self.extra_edges_list_range.clone()?])
    }
}

impl Debug for GraphFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, r#"GraphFile("{:?}")"#, self.path.display())
    }
}
