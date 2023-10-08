use std::path::Path;

use google_cloud_storage::{
    client::{Client, ClientConfig},
    http::objects::upload::{Media, UploadObjectRequest, UploadType},
};
use reqwest::Body;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use wikiwalk::paths::DBPaths;

pub async fn push_built_files(db_paths: DBPaths) -> anyhow::Result<()> {
    let config = ClientConfig::default().with_auth().await.unwrap();
    let client = Client::new(config);

    let paths = [
        db_paths.db_status_path(),
        db_paths.graph_db(),
        db_paths.vertex_al_ix_path(),
        db_paths.vertex_al_path(),
    ];
    for p in paths.iter() {
        push_file(&client, p, &db_paths.base).await?;
    }
    Ok(())
}

async fn push_file(client: &Client, path: &Path, base: &Path) -> anyhow::Result<()> {
    let cloud_path = path.strip_prefix(base)?.to_string_lossy().to_string();
    log::info!(
        "pushing file to google cloud: {} relative to {} at {}",
        &path.display(),
        &base.display(),
        cloud_path
    );
    let source = File::open(path).await?;
    let body = ReaderStream::new(source);

    let upload_type = UploadType::Simple(Media::new(cloud_path));
    let uploaded = client
        .upload_object(
            &UploadObjectRequest {
                bucket: "wikiwalk".to_string(),
                ..Default::default()
            },
            Body::wrap_stream(body),
            &upload_type,
        )
        .await?;

    log::info!("pushed file to google cloud: {:?}", uploaded);

    Ok(())
}
