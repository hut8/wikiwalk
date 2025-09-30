use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Paths {
    pub base: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        let home_dir = dirs::home_dir().unwrap();
        let default_data_dir = home_dir.join("data").join("wikiwalk");
        let data_dir = match std::env::var("DATA_ROOT").ok() {
            Some(data_dir_str) => PathBuf::from(data_dir_str),
            None => default_data_dir,
        };
        std::fs::create_dir_all(&data_dir).unwrap();
        Paths { base: data_dir }
    }

    pub fn with_base(base: &Path) -> Self {
        Paths {
            base: base.to_path_buf(),
        }
    }

    pub fn path_master_database(&self) -> PathBuf {
        self.base.join("master.db")
    }

    pub fn dump_paths(&self, date: &str) -> DumpPaths {
        DumpPaths::new(self.base.clone(), date)
    }

    pub fn db_paths(&self, date: &str) -> DBPaths {
        DBPaths::new(self.base.clone(), date)
    }
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct DumpPaths {
    pub base: PathBuf,
    pub date: String,
}

impl DumpPaths {
    pub fn new(base: PathBuf, date: &str) -> Self {
        let base = base.join("dumps");
        DumpPaths {
            base,
            date: date.to_owned(),
        }
    }

    pub fn dump_path(&self, table: &str) -> PathBuf {
        let basename = format!("enwiki-{date}-{table}.sql.gz", date = self.date);
        self.base.join(basename)
    }

    pub fn page(&self) -> PathBuf {
        self.dump_path("page")
    }

    pub fn pagelinks(&self) -> PathBuf {
        self.dump_path("pagelinks")
    }

    pub fn redirect(&self) -> PathBuf {
        self.dump_path("redirect")
    }

    pub fn link_targets(&self) -> PathBuf {
        self.dump_path("linktarget")
    }
}

/// Tracks paths for files built by tool and consumed by tool and server
#[derive(Clone)]
pub struct DBPaths {
    pub base: PathBuf,
    pub date: String,
}

impl DBPaths {
    pub fn new(base: PathBuf, date: &str) -> Self {
        let base = base.join(date);
        if date != "current" {
            // current is a symlink to a real path
            std::fs::create_dir_all(&base).unwrap();
        }
        DBPaths {
            base,
            date: date.to_owned(),
        }
    }

    pub fn ensure_exists(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.base)
    }

    pub fn path_for(&self, basename: &str) -> PathBuf {
        self.base.join(basename)
    }

    pub fn db_status_path(&self) -> PathBuf {
        self.path_for("status.json")
    }

    pub fn vertex_al_path(&self) -> PathBuf {
        self.path_for("vertex-al")
    }

    pub fn vertex_al_ix_path(&self) -> PathBuf {
        self.path_for("vertex-al-ix")
    }

    pub fn redirects_path(&self) -> PathBuf {
        self.path_for("redirects")
    }

    pub fn graph_db(&self) -> PathBuf {
        self.path_for("graph.db")
    }

    pub fn sitemaps_path(&self) -> PathBuf {
        self.path_for("sitemaps")
    }

    pub fn topgraph_path(&self) -> PathBuf {
        self.path_for("topgraph.json")
    }
}
