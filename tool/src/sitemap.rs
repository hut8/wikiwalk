use sea_orm::{EntityTrait, QuerySelect};
use xml::{writer::XmlEvent, EventWriter};

use wikiwalk::schema::prelude::Vertex;

pub async fn make_sitemap(db: &sea_orm::DatabaseConnection, sitemaps_path: &std::path::Path) {
    std::fs::create_dir_all(sitemaps_path).expect("create sitemaps directory");
    let vertexes = Vertex::find()
        .select_only()
        .column(wikiwalk::schema::vertex::Column::Id)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes");
    let pairs = generate_pairs(vertexes);
    log::info!("sitemap: generated {} pairs", pairs.len());
    let pair_chunks = pairs.chunks(50_000);
    pair_chunks.enumerate().for_each(|(i, chunk)| {
        let directory = std::path::Path::new("sitemap");
        std::fs::create_dir_all(directory).expect("create sitemap directory");
        write_chunk(i, directory, chunk).expect("write sitemap chunk");
    });
}

fn generate_pairs(vertices: Vec<u32>) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for source in &vertices {
        for target in &vertices {
            if *source != *target {
                pairs.push((*source, *target));
            }
        }
    }
    pairs
}

fn write_chunk(
    chunk_number: usize,
    directory: &std::path::Path,
    pairs: &[(u32, u32)],
) -> Result<(), xml::writer::Error> {
    let path = directory.join(format!("sitemap-{}.xml", chunk_number));
    let sink = std::fs::File::create(path).expect("create sitemap.xml");
    let mut writer = EventWriter::new_with_config(
        sink,
        xml::writer::EmitterConfig {
            perform_indent: true,
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

const BASE_URL: &str = "https://wikiwalk.app";

fn path_url(source: u32, target: u32) -> String {
    format!("{}/path/{}/{}", BASE_URL, source, target)
}
