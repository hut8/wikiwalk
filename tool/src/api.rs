const WIKIPEDIA_API_URL: &str = "https://en.wikipedia.org/w/api.php";

use chrono::Months;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use serde::Deserialize;

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopPagesRoot {
    pub items: Vec<Item>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    pub project: String,
    pub access: String,
    pub year: String,
    pub month: String,
    pub day: String,
    pub articles: Vec<Article>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Article {
    pub article: String,
    pub views: i64,
    pub rank: i64,
}

async fn top_page_titles() -> Vec<String> {
  use chrono::prelude::*;
  let last_month = Utc::now()
      .checked_sub_months(Months::new(1))
      .expect("subtract month");
  let date_component = last_month.format("%Y/%m").to_string();
  log::info!("sitemap: fetching top pages for {}", date_component);
  let top_pages_url = format!("https://wikimedia.org/api/rest_v1/metrics/pageviews/top/en.wikipedia.org/all-access/{}/all-days", date_component);
  let top_pages_response = reqwest::get(top_pages_url)
      .await
      .expect("get top pages from api");
  let top_pages_root: TopPagesRoot = top_pages_response
      .json()
      .await
      .expect("parse top pages json");
  top_pages_root
      .items
      .into_iter()
      .flat_map(|item| item.articles.into_iter().map(|article| article.article))
      .collect::<Vec<String>>()
}

pub async fn top_page_ids() -> Vec<u32> {
  let page_titles = top_page_titles().await;
  let chunks = page_titles.chunks(50);
  let chunk_iterator = chunks.into_iter();
  let chunk_futures = chunk_iterator.map(fetch_pages_data);
  let chunk_futures = chunk_futures.collect::<FuturesUnordered<_>>();
  chunk_futures
      .collect::<Vec<_>>()
      .await
      .into_iter()
      .flatten()
      .collect()
}

async fn fetch_pages_data(titles: &[String]) -> Vec<u32> {
    assert!(titles.len() <= 50);
    let titles = titles.join("|");
    log::info!("sitemap: fetch page data chunk: titles = {}", titles);
    let mut api_url = url::Url::parse(WIKIPEDIA_API_URL).expect("parse wikipedia api url");
    api_url
        .query_pairs_mut()
        .append_pair("action", "query")
        .append_pair("format", "json")
        .append_pair("prop", "pageprops")
        .append_pair("titles", &titles);
    log::info!(
        "sitemap: fetch page data chunk: api_url = {}",
        api_url.as_str()
    );
    let response = reqwest::get(api_url).await.expect("get page ids from api");
    let response_json: serde_json::Value = response.json().await.expect("parse page ids json");
    let pages = response_json["query"]["pages"]
        .as_object()
        .expect("pages object");
    log::info!(
        "sitemap: fetch page data chunk: received {} pages",
        pages.len()
    );
    pages
        .iter()
        .map(|(_, page)| page)
        .filter_map(|page| {
            page["ns"].as_i64().and_then(|ns| match ns {
                0 => {
                    let pageid_raw = page["pageid"].as_i64().expect("find pageid");
                    let page_id: Option<u32> = pageid_raw.try_into().ok();
                    page_id
                }
                _ => {
                    log::info!(
                        "top_page_ids: skipping page '{:?}' with ns = {}",
                        page["title"].as_str(),
                        ns
                    );
                    None
                }
            })
        })
        .collect::<Vec<_>>()
}