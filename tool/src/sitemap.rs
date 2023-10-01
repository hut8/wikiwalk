use flate2::write::GzEncoder;
use flate2::Compression;
use itertools::Itertools;
use sea_orm::{EntityTrait, QuerySelect};
use std::io::prelude::*;
use xml::{writer::XmlEvent, EventWriter};

use wikiwalk::schema::prelude::Vertex;

const BASE_URL: &str = "https://wikiwalk.app";

pub async fn make_sitemap(db: &sea_orm::DatabaseConnection, sitemaps_path: &std::path::Path) {
    std::fs::create_dir_all(sitemaps_path).expect("create sitemaps directory");
    let vertexes: Vec<u32> = Vertex::find()
        .select_only()
        .column(wikiwalk::schema::vertex::Column::Id)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes");
    let sources = vertexes.clone();
    let targets = vertexes.clone();
    let pairs = sources
        .into_iter()
        .cartesian_product(targets.into_iter())
        .filter(|(source, target)| *source != *target);

    log::info!("sitemap: generated pairs iterator");
    let chunk_iterator = pairs.chunks(50_000);
    let pair_chunks = chunk_iterator.into_iter();
    let chunk_count = pair_chunks
        .enumerate()
        .map(|(i, chunk)| {
            let pairs = chunk.collect::<Vec<(u32, u32)>>();
            std::fs::create_dir_all(sitemaps_path).expect("create sitemap directory");
            write_chunk(i, sitemaps_path, &pairs).expect("write sitemap chunk");
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
    }

    writer
        .write(xml::writer::XmlEvent::end_element())
        .expect("write end root");

    let buf = encoder.finish().expect("encode gzip");
    let path = directory.join("sitemap.xml.gz");
    let mut sink = std::fs::File::create(&path).expect("create sitemap.xml");

    sink.write_all(&buf).expect("write sitemap.xml.gz");
    log::info!("sitemap: wrote index to {}", path.display());
}

fn write_chunk(
    chunk_number: usize,
    directory: &std::path::Path,
    pairs: &[(u32, u32)],
) -> Result<(), xml::writer::Error> {
  // TODO: Ensure that each chunk is at most 50MB uncompressed
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
    for (source, target) in pairs {
        write_url(&mut writer, *source, *target)?;
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
    source: u32,
    target: u32,
) -> Result<(), xml::writer::Error> {
    writer.write(xml::writer::XmlEvent::start_element("url"))?;

    writer.write(XmlEvent::start_element("loc"))?;
    writer.write(XmlEvent::characters(&path_url(source, target)))?;
    writer.write(XmlEvent::end_element())?;
    // url = url.append(xml::writer::XmlEvent::start_element("lastmod").append(xml::writer::XmlEvent::characters(&page.last_modified().to_rfc3339())));

    writer.write(XmlEvent::start_element("changefreq"))?;
    writer.write(XmlEvent::characters("monthly"))?;
    writer.write(XmlEvent::end_element())?;

    writer.write(XmlEvent::start_element("priority"))?;
    writer.write(XmlEvent::characters("0.5"))?;
    writer.write(XmlEvent::end_element())?;

    writer.write(XmlEvent::end_element())?;
    Ok(())
}

fn path_url(source: u32, target: u32) -> String {
    format!("{}/path/{}/{}", BASE_URL, source, target)
}
