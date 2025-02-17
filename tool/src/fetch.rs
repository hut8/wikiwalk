use chrono::{NaiveDate, Utc};
use futures::StreamExt;
use itertools::Itertools;
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
    let client = Client::default();
    let dump_index_response = client
        .get(DUMP_INDEX_URL)
        .send()
        .await
        .ok()
        .and_then(|res| res.error_for_status().ok());
    let dump_index = match dump_index_response {
        Some(res) => res.json::<DumpIndex>().await.ok(),
        None => None,
    }
    .map(|ix| ix.wikis.enwiki)
    .and_then(|ix| match ix.jobs.done() {
        true => Some(ix),
        false => None,
    })
    .and_then(|mut ix| match ix.jobs.dump_date() {
        Some(date) => {
            ix.dump_date = date;
            Some(ix)
        }
        None => None,
    });

    if dump_index.is_some() {
        log::info!("found complete dump via index file");
        return dump_index;
    }

    let today = Utc::now().date_naive();
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

pub async fn fetch_dump_status_for_date(
    client: &Client,
    date: &str,
) -> Result<DumpStatus, anyhow::Error> {
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
        status.jobs.linktarget_table,
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
    #[serde(rename = "linktargettable")]
    pub linktarget_table: JobStatus,
}

impl Jobs {
    pub fn done(&self) -> bool {
        self.all().iter().all(|job| job.done())
    }

    /// The dumps appear not to have a canonical "this dump is for this date" field. Instead, there are a couple of sources
    /// of truth for dump date:
    /// 1. The "updated" field in the job status objects - this is the date the job was last updated. It is unknown whether this ever changes
    ///    once that particular job reaches the "done" state for a particular date mentioned in #2.
    /// 2. The second part of the URL - this is, for example, "20240601" in "/enwiki/20240601/enwiki-20240601-redirect.sql.gz".
    /// We are going to use the second method, as it appears to be the date that the dump was started.
    /// Generally it looks like those are either the first of the month, or the 20th, but there's no guarantee of that.
    pub fn dump_date(&self) -> Option<String> {
        let updated_dates = self
            .all()
            .iter()
            .map(|f| {
                NaiveDate::parse_from_str(&f.updated, "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .format("%Y%m%d")
                    .to_string()
            })
            .unique()
            .collect_vec();
        if updated_dates.len() > 1 {
            log::warn!(
                "dump appears to contain data from multiple dates: {:?}",
                updated_dates
            );
        }


        let file_urls = self
            .all()
            .iter()
            .flat_map(|f| f.files.values().map(|x| x.url.clone()))
            .collect_vec();
        // "enwiki-20240601-redirect.sql.gz": {
        //     "size": 167307929,
        //     "url": "/enwiki/20240601/enwiki-20240601-redirect.sql.gz",
        //     "md5": "394075f6eb3ff05fbab7f6ffd9aa5128",
        //     "sha1": "41daa467c6c2e00515579035580d271f37f5e622"
        //   }
        let dump_date_strs = file_urls
            .iter()
            .map(|url| {
                url.split('/').nth(2).expect("extract date from url").to_string()
            })
            .unique()
            .collect_vec();
        match dump_date_strs.len() {
            0 => None,
            1 => Some(dump_date_strs[0].clone()),
            _ => {
                log::error!(
                    "dump appears to contain data from multiple dates: {:?}",
                    updated_dates
                );
                None
            }
        }
    }

    pub fn all(&self) -> Vec<&JobStatus> {
        vec![
            &self.redirect_table,
            &self.page_table,
            &self.pageprops_table,
            &self.pagelinks_table,
            &self.linktarget_table,
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
