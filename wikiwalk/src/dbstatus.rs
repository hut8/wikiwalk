use std::{fs::File, path::PathBuf};

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct DBStatus {
    #[serde(rename = "dump_date")]
    pub dump_date_str: String,
    pub vertexes_loaded: bool,
    pub edges_resolved: bool,
    pub edges_sorted: bool,
    pub build_complete: bool,
    #[serde(skip)]
    pub status_path: Option<PathBuf>,
}

impl DBStatus {
    fn dump_date(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(&self.dump_date_str, "%Y%m%d").ok()
    }
}

impl DBStatus {
    pub fn load(status_path: PathBuf) -> DBStatus {
        match File::open(&status_path) {
            Ok(file) => {
                let mut val: DBStatus = serde_json::from_reader(file).unwrap();
                val.status_path = Some(status_path);
                val
            }
            Err(_) => DBStatus {
                status_path: Some(status_path),
                ..Default::default()
            },
        }
    }

    pub fn save(&self) {
        let status_path = self.status_path.as_ref().unwrap();
        let sink = File::create(status_path).expect("save db status");
        serde_json::to_writer_pretty(&sink, self).unwrap();
    }
}
