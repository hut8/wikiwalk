use chrono::Utc;
use futures::StreamExt;
// use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{cmp::min, collections::HashMap, fs::File, io::Write, path::Path};

/// Look back this many days for the oldest dump
pub static OLDEST_DUMP: u64 = 60;

static DUMP_INDEX_URL: &str = "https://dumps.wikimedia.org/index.json";

pub async fn find_latest() -> Option<DumpStatus> {
    // let client = Client::default();
    // let dump_index: DumpIndex = client
    //     .get(DUMP_INDEX_URL)
    //     .send()
    //     .await
    //     .expect("fetch dump index")
    //     .error_for_status()
    //     .expect("check dump index status")
    //     .json()
    //     .await
    //     .expect("parse dump index");
    //   let dump_status = dump_index.wikis.enwiki;

    let today = Utc::now().date_naive();
    let client = Client::default();
    for past_days in 0..OLDEST_DUMP {
        let date = today
            .checked_sub_days(chrono::Days::new(past_days))
            .unwrap();
        log::info!("checking dump for {date:?}");
        let date_fmt = date.format("%Y%m%d").to_string();
        match fetch_dump_status_for_date(&client, &date_fmt).await {
            Ok(status) => {
                log::info!("dump status: {status:?}");
                if status.jobs.done() {
                    log::info!("found most recent complete dump: {date:?}");
                    return Some(status);
                }
            }
            Err(err) => {
                log::info!("dump status error: {err:?}");
            }
        }
    }
    None
}

pub async fn fetch_dump_status_for_date(client: &Client, date: &str) -> Result<DumpStatus, anyhow::Error> {
    let url_str = format!("https://dumps.wikimedia.org/enwiki/{date}/dumpstatus.json");
    log::info!("fetching dump status from: {url_str}");
    let mut dump_status: DumpStatus = client
        .get(url_str)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    dump_status.dump_date = date.to_owned();
    Ok(dump_status)
}

pub async fn fetch_dump(dump_dir: &Path, status: DumpStatus) -> Result<(), anyhow::Error> {
    log::info!("fetching dump for {status:?}");
    let client = Client::default();
    let jobs = [
        status.jobs.redirect_table,
        status.jobs.page_table,
        status.jobs.pageprops_table,
        status.jobs.pagelinks_table,
    ];

    let job_futures = jobs.into_iter().map(|j| fetch_job(dump_dir, &client, j));
    let stream = futures::stream::iter(job_futures).buffer_unordered(3);
    let job_results = stream.collect::<Vec<_>>().await;

    for job_status in job_results.iter() {
        match job_status {
            Ok(job_status) => {
                log::info!("job complete: {job_status:?}");
            }
            Err(err) => {
                log::error!("job error: {err:?}");
            }
        }
    }

    if job_results.iter().any(|f| f.is_err()) {
        return Err(anyhow::Error::msg("one or more jobs failed"));
    }

    Ok(())
}

pub fn absolute_dump_url(rel_url: &str) -> String {
    format!("https://dumps.wikimedia.org{rel_url}")
}

async fn fetch_job(
    dump_dir: &Path,
    client: &Client,
    job: JobStatus,
) -> Result<JobStatus, anyhow::Error> {
    std::fs::create_dir_all(dump_dir)?;
    for (file, file_info) in job.files.iter() {
        log::info!("fetching file: {file}");
        fetch_file(
            client,
            &absolute_dump_url(&file_info.url),
            dump_dir,
            file,
            file_info,
        )
        .await?;
    }
    Ok(job)
}

pub fn clean_dump_dir(dump_dir: &Path) {
    log::info!("cleaning dump directory: {}", dump_dir.display());
    std::fs::read_dir(dump_dir)
        .expect("read dump directory")
        .for_each(|e| {
            let entry = e.expect("read entry");
            let path = entry.path();
            if path.extension().is_some_and(|e| e != "gz") {
                log::debug!("skipping non-gz file: {}", path.display());
                return;
            }
            log::debug!("removing: {}", path.display());
            std::fs::remove_file(path).expect("remove file");
        });
}

pub async fn fetch_file(
    client: &Client,
    url: &str,
    dump_dir: &Path,
    basename: &String,
    file_info: &DumpFileInfo,
) -> Result<(), anyhow::Error> {
    let sink_path = dump_dir.join(basename);
    let total_size = file_info.size as u64;

    // Indicatif setup
    // FIXME: Replace with multiple progress bars
    // let pb = ProgressBar::new(total_size);
    // let style = ProgressStyle::default_bar()
    //     .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").expect("set progress bar style");
    // pb.set_style(style);
    // pb.set_message(basename.clone());

    // Partial resume support
    let offset = match std::fs::metadata(&sink_path) {
        Ok(metadata) => {
            if metadata.len() == total_size {
                // pb.finish_with_message(format!(
                //     "{} already downloaded to {}",
                //     url,
                //     sink_path.clone().display()
                // ));
                log::info!(
                    "{sink_path} is already complete",
                    sink_path = sink_path.display()
                );
                return Ok(());
            }
            log::info!(
                "resuming download at byte {offset} to {sink_path}",
                offset = metadata.len(),
                sink_path = sink_path.display()
            );
            metadata.len()
        }
        Err(_) => 0,
    };

    // Reqwest setup
    let mut headers = HeaderMap::new();
    if offset > 0 {
        let header_val =
            HeaderValue::from_str(format!("{}-", offset).as_str()).expect("construct range header");
        headers.insert(reqwest::header::RANGE, header_val);
    }
    let res = client.get(url).headers(headers).send().await?;
    res.error_for_status_ref()?;

    // download chunks
    let mut file = File::options()
        .append(true)
        .write(true)
        .read(true)
        .create(true)
        .open(&sink_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(chunk_state) = stream.next().await {
        let chunk = chunk_state.map_err(|err| {
            log::error!("chunk error for url {url} at byte {downloaded} of {total_size}: {err}");
            err
        })?;
        file.write_all(&chunk)?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        // pb.set_position(new);
    }

    // pb.finish_with_message(format!(
    //     "fetched {} to {}",
    //     url,
    //     sink_path.clone().display()
    // ));
    log::info!("fetched {} to {}", url, sink_path.clone().display());
    Ok(())
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpStatus {
    pub jobs: Jobs,
    pub version: String,
    #[serde(skip_deserializing)]
    pub dump_date: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpIndex {
  pub wikis: DumpWikis,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpWikis {
  pub enwiki: DumpStatus,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Jobs {
    #[serde(rename = "redirecttable")]
    pub redirect_table: JobStatus,
    #[serde(rename = "pagetable")]
    pub page_table: JobStatus,
    #[serde(rename = "pagepropstable")]
    pub pageprops_table: JobStatus,
    #[serde(rename = "pagelinkstable")]
    pub pagelinks_table: JobStatus,
}

impl Jobs {
    pub fn done(&self) -> bool {
        self.all().iter().all(|job| job.done())
    }

    pub fn all(&self) -> Vec<&JobStatus> {
        vec![
            &self.redirect_table,
            &self.page_table,
            &self.pageprops_table,
            &self.pagelinks_table,
        ]
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub status: String,
    pub updated: String,
    pub files: HashMap<String, DumpFileInfo>,
}

impl JobStatus {
    pub fn done(&self) -> bool {
        self.status == "done"
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpFileInfo {
    pub size: i64,
    pub url: String,
    pub md5: String,
    pub sha1: String,
}
