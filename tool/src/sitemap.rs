use flate2::write::GzEncoder;
use flate2::Compression;
use itertools::Itertools;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::io::prelude::*;
use xml::{writer::XmlEvent, EventWriter};

use wikiwalk::schema::prelude::Vertex;

const BASE_URL: &str = "https://wikiwalk.app";

// 7,000,000 pages in pairs = 49,000,000,000,000
// 50,000 pairs per chunk = 980,000,000 chunks (sitemap files - sitemap-0.xml.gz, sitemap-1.xml.gz, etc.)
// 50,000 sitemap files per sitemap index = 980,000,000 / 50,000 = 19,600 sitemap index files (sitemap.xml.gz, sitemap-1.xml.gz, etc.)
// maximum number of sitemaps per sitemap index = 50,000
// this is do-able, but pushing it. Instead, we'll get the top 1,000 articles that we can link to

// 1,000 pages in pairs = less than 1,000,000 pairs
// 50,000 pairs per chunk = 20 chunks
// 50,000 sitemap files per sitemap index = 20 / 50,000 = 1 sitemap index file

pub async fn make_sitemap(db: &sea_orm::DatabaseConnection, sitemaps_path: &std::path::Path) {
    std::fs::create_dir_all(sitemaps_path).expect("create sitemaps directory");
    log::info!("sitemap: finding top pages");
    let top_page_ids = crate::api::top_page_ids(None).await;
    log::info!("sitemap: found {} valid top pages", top_page_ids.len());
    let page_ids: Vec<u32> = Vertex::find()
        .select_only()
        .filter(wikiwalk::schema::vertex::Column::Id.is_in(top_page_ids.clone()))
        .column(wikiwalk::schema::vertex::Column::Id)
        .order_by(wikiwalk::schema::vertex::Column::Id, sea_orm::Order::Asc)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes");
    let missing_page_ids: Vec<u32> = top_page_ids
        .clone()
        .into_iter()
        .filter(|page_id| !page_ids.contains(page_id))
        .collect();
    log::info!(
        "sitemap: found {} valid top pages in db (missing {:?})",
        page_ids.len(),
        missing_page_ids
    );
    let sources = page_ids.clone();
    let targets = page_ids.clone();
    let pairs = sources
        .into_iter()
        .cartesian_product(targets.into_iter())
        .filter(|(source, target)| *source != *target)
        .map(|(source, target)| path_url(source, target));
    let site_urls = std::iter::once(BASE_URL.to_string()).chain(pairs);

    log::info!("sitemap: generated pairs iterator");
    let chunk_iterator = site_urls.chunks(50_000);
    let pair_chunks = chunk_iterator.into_iter();
    let chunk_count = pair_chunks
        .enumerate()
        .map(|(i, chunk)| {
            let urls  = chunk.collect::<Vec<String>>();
            std::fs::create_dir_all(sitemaps_path).expect("create sitemap directory");
            write_chunk(i, sitemaps_path, &urls).expect("write sitemap chunk");
        })
        .count();
    log::info!("sitemap: wrote {} chunks", chunk_count);
    write_sitemap_index(chunk_count, sitemaps_path)
}

fn write_sitemap_index(count: usize, directory: &std::path::Path) {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    let mut writer = EventWriter::new_with_config(
        &mut encoder,
        xml::writer::EmitterConfig {
            perform_indent: false,
            ..Default::default()
        },
    );

    let root_element = XmlEvent::start_element("sitemapindex")
        .attr("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9");
    writer.write(root_element).expect("write root element");

    for i in 0..count {
        let sitemap_url = format!("{}/sitemaps/sitemap-{}.xml.gz", BASE_URL, i);
        writer
            .write(XmlEvent::start_element("sitemap"))
            .expect("write sitemap element");
        writer
            .write(XmlEvent::start_element("loc"))
            .expect("write loc element");
        writer
            .write(XmlEvent::characters(&sitemap_url))
            .expect("write sitemap url");
        writer
            .write(XmlEvent::end_element())
            .expect("write end loc element");
        writer
            .write(XmlEvent::end_element())
            .expect("write end sitemap element");
    }

    writer
        .write(xml::writer::XmlEvent::end_element())
        .expect("write end root");

    let buf = encoder.finish().expect("encode gzip");
    let path = directory.join("sitemap-index.xml.gz");
    let mut sink = std::fs::File::create(&path).expect("create sitemap-index.xml.gz");
    sink.write_all(&buf).expect("write sitemap.xml.gz");
    log::info!("sitemap: wrote index to {}", path.display());
}

fn write_chunk(
    chunk_number: usize,
    directory: &std::path::Path,
    urls: &[String],
) -> Result<(), xml::writer::Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    let mut writer = EventWriter::new_with_config(
        &mut encoder,
        xml::writer::EmitterConfig {
            perform_indent: false,
            ..Default::default()
        },
    );
    writer.write(
        xml::writer::XmlEvent::start_element("urlset")
            .attr("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"),
    )?;
    for u in urls {
        write_url(&mut writer, u, None)?;
    }
    writer.write(xml::writer::XmlEvent::end_element())?;

    let buf = encoder.finish().expect("encode gzip");

    let path = directory.join(format!("sitemap-{}.xml.gz", chunk_number));
    let mut sink = std::fs::File::create(&path).expect("create sitemap.xml");

    sink.write_all(&buf).expect("write sitemap.xml.gz");
    log::info!(
        "sitemap: wrote chunk {} to {}",
        chunk_number,
        path.display()
    );
    Ok(())
}

fn write_url<W: std::io::Write>(
    writer: &mut EventWriter<W>,
    url: &str,
    priority: Option<f32>,
) -> Result<(), xml::writer::Error> {
    writer.write(xml::writer::XmlEvent::start_element("url"))?;

    writer.write(XmlEvent::start_element("loc"))?;
    writer.write(XmlEvent::characters(url))?;
    writer.write(XmlEvent::end_element())?;

    writer.write(XmlEvent::start_element("changefreq"))?;
    writer.write(XmlEvent::characters("monthly"))?;
    writer.write(XmlEvent::end_element())?;

    if let Some(priority) = priority {
        writer.write(XmlEvent::start_element("priority"))?;
        writer.write(XmlEvent::characters(&format!("{}", priority)))?;
        writer.write(XmlEvent::end_element())?;
    }

    writer.write(XmlEvent::end_element())?;
    Ok(())
}

fn path_url(source: u32, target: u32) -> String {
    format!("{}/paths/{}/{}", BASE_URL, source, target)
}
