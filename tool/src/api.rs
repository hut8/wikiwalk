const WIKIPEDIA_API_URL: &str = "https://en.wikipedia.org/w/api.php";

use chrono::Months;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use itertools::Itertools;
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
pub struct Article {
    pub article: String,
    pub views: i64,
    pub rank: i64,
    pub id: Option<u32>,
}

pub async fn top_pages() -> Vec<Article> {
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
    let articles: Vec<Article> = top_pages_root
        .items
        .into_iter()
        .flat_map(|item| item.articles.into_iter())
        .map(|mut article| {
            article.article = article.article.replace('_', " ");
            article
        })
        .collect();

    let page_titles = articles
        .iter()
        .map(|article| article.article.clone())
        .collect::<Vec<_>>();
    let chunks = page_titles.chunks(50);
    let chunk_iterator = chunks.into_iter();
    let chunk_futures = chunk_iterator.map(fetch_pages_data);
    let chunk_futures = chunk_futures.collect::<FuturesUnordered<_>>();
    let page_title_ids: Vec<(u32, String)> = chunk_futures
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect();
    // Add the IDs to the articles
    log::info!("top pages: built id map: {:?}", page_title_ids);

    let mut articles = articles
        .into_iter()
        .filter_map(|mut article| {
            let id = page_title_ids
                .iter()
                .find(|(_id, title)| title == &article.article)
                .map(|(id, _)| *id);
            id.map(|article_id| {
                article.id = Some(article_id);
                article
            })
        })
        .collect::<Vec<_>>();

    articles.sort_by(|a, b| a.rank.cmp(&b.rank));

    articles
        .iter_mut()
        .enumerate()
        .map(|(i, article)| {
            article.rank = i as i64 + 1;
            article.to_owned()
        })
        .collect_vec()
}

pub async fn top_page_ids(count: Option<usize>) -> Vec<u32> {
    let top = top_pages().await;
    let mut top_ids: Vec<u32> = top.iter().filter_map(|article| article.id).collect();
    if let Some(count) = count {
        top_ids.truncate(count);
    }
    top_ids
}

// Fetch the page ids for a chunk of page titles
// The returned vector is a list of (page_id, page_title) pairs, where the page_title has been normalized without underscores
async fn fetch_pages_data(titles: &[String]) -> Vec<(u32, String)> {
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
            log::debug!("evaluating page: {}", page);
            // Pages that are marked as "missing" have {"missing":""}
            if page.get("missing").is_some() {
                return None;
            }
            page["ns"].as_i64().and_then(|ns| match ns {
                0 => {
                    let pageid_raw = page["pageid"].as_i64().expect("find pageid");
                    let title = page["title"].as_str().expect("find title").to_string();
                    let page_id: u32 = pageid_raw.try_into().expect("pageid to u32");
                    Some((page_id, title))
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
