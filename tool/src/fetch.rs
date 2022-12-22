use chrono::{NaiveDate, Utc};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::min,
    collections::HashMap,
    fs::File,
    io::Write,
    path::PathBuf,
};

/// Look back this many days for the oldest dump
pub static OLDEST_DUMP: u64 = 60;

pub async fn find_latest() -> Option<DumpStatus> {
    let today = Utc::now().date_naive();
    let client = reqwest::Client::default();
    for past_days in 0..OLDEST_DUMP {
        let date = today
            .checked_sub_days(chrono::Days::new(past_days))
            .unwrap();
        log::info!("checking dump for {date:?}");
        match fetch_dump_status(&client, date).await {
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

pub async fn fetch_dump_status(
    client: &reqwest::Client,
    date: NaiveDate,
) -> Result<DumpStatus, anyhow::Error> {
    let date_fmt = date.format("%Y%m%d").to_string();
    let url_str = format!("https://dumps.wikimedia.org/enwiki/{date_fmt}/dumpstatus.json");
    log::info!("fetching dump status from: {url_str}");
    let dump_status: DumpStatus = client
        .get(url_str)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(dump_status)
}

pub async fn fetch_dump(dump_dir: &PathBuf, status: &DumpStatus) -> Result<(), anyhow::Error> {
    log::info!("fetching dump for {status:?}");
    let client = reqwest::Client::default();
    for job in [
        &status.jobs.redirect_table,
        &status.jobs.page_table,
        &status.jobs.pageprops_table,
        &status.jobs.pagelinks_table,
    ]
    .iter()
    {
        fetch_job(dump_dir, &client, job).await?;
    }
    Ok(())
}

async fn fetch_job(
    dump_dir: &PathBuf,
    client: &Client,
    job: &JobStatus,
) -> Result<(), anyhow::Error> {
    std::fs::create_dir_all(dump_dir)?;
    for (file, file_info) in job.files.iter() {
        log::info!("fetching file: {file}");
        let url = format!(
            "https://dumps.wikimedia.org{rel_url}",
            rel_url = file_info.url
        );
        fetch_file(client, &url, dump_dir, file, file_info).await?;
    }
    Ok(())
}

pub async fn fetch_file(
    client: &Client,
    url: &str,
    dump_dir: &PathBuf,
    basename: &String,
    file_info: &DumpFileInfo,
) -> Result<(), anyhow::Error> {
    let sink_path = dump_dir.join(basename);
    let total_size = file_info.size as u64;

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    let style = ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").expect("set progress bar style");
    pb.set_style(style);
    pb.set_message(basename.clone());

    // Partial resume support
    let offset = match std::fs::metadata(&sink_path) {
        Ok(metadata) => {
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
        .create(true).open(&sink_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(chunk_state) = stream.next().await {
        let chunk = chunk_state?;
        file.write_all(&chunk)?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!(
        "fetched {} to {}",
        url,
        sink_path.clone().display()
    ));
    Ok(())
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpStatus {
    pub jobs: Jobs,
    pub version: String,
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
        [
            &self.redirect_table,
            &self.page_table,
            &self.pageprops_table,
            &self.pagelinks_table,
        ]
        .iter()
        .all(|job| job.done())
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
