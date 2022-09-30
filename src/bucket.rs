use crate::{AWError, BlobMetadata, FileLocation, MetadataManager, PathManager, StreamExt};
use actix_multipart::Multipart;
use actix_web::put;
use actix_web::web::{Data, Path as WebPath};
use actix_web::HttpResponse;
use actix_web::{get, HttpRequest};
use serde::{Deserialize, Serialize};
use sha1::Digest;
use sha1::Sha1;
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

#[derive(Deserialize)]
pub struct BucketLocation {
    name: String,
}

#[derive(Serialize)]
pub struct Bucket {
    blobs: Vec<Blob>,
}

#[derive(Deserialize, Serialize)]
pub struct Blob {
    blob_name: String,
    blob_sha1: String,
}

#[get("/api/bucket/{name}/create")]
pub async fn get_bucket_create(
    paths: Data<PathManager>,
    file: WebPath<BucketLocation>,
) -> Result<HttpResponse, AWError> {
    let path = paths.get_bucket(Path::new(&file.name));
    tokio::fs::create_dir(path)
        .await
        .expect("Unable to create directory");

    Ok(HttpResponse::Ok().finish())
}

#[get("/api/bucket/{name}/verify")]
pub async fn bucket_verify(
    paths: Data<PathManager>,
    file: WebPath<BucketLocation>,
) -> Result<HttpResponse, AWError> {
    let mut blobs = Vec::new();

    let path = paths.get_bucket(Path::new(&file.name));

    for e in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            println!("{}", e.path().display());
            let path = e.path();

            let mut sha = Sha1::new();

            let mut blob_file = File::open(path).unwrap();
            let mut content = String::new();
            blob_file
                .read_to_string(&mut content)
                .expect("Failed to read file");

            sha.update(content.as_bytes());

            let hex_string = format!("{:X}", sha.finalize());
            let path_string = path
                .to_str()
                .unwrap()
                .replace(&format!("storage_root/{}/", &file.name), "");

            blobs.push(Blob {
                blob_name: path_string,
                blob_sha1: hex_string,
            })
        }
    }

    Ok(HttpResponse::Ok().json(Bucket { blobs }))
}

#[put("/api/bucket/{bucket_name}/{file_name}/upload")]
pub async fn put_bucket_upload(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: WebPath<FileLocation>,
    mut data: Multipart,
    req: HttpRequest,
) -> Result<HttpResponse, AWError> {
    let path = paths.get_bucket_file(Path::new(&file.bucket_name), Path::new(&file.file_name));
    let mut file = tokio::fs::File::create(path.deref()).await?;
    let mut meta = BlobMetadata::default();

    if let Some(ct) = req.headers().get("X-Blob-Content-Type") {
        meta.content_type = ct.to_str().expect("content type str").to_string();
    }

    tracing::info!("Headers = {:?}", req.headers());

    while let Some(item) = data.next().await {
        let mut field = item?;

        while let Some(chunk) = field.next().await {
            let data = chunk?;
            // filesystem operations are blocking, we have to use threadpool
            file.write_all(&data).await?;
        }
    }

    match metadata.create_metadata(&path, &meta) {
        Ok(_m) => {}
        Err(_e) => return Ok(HttpResponse::InternalServerError().finish()),
    }

    Ok(HttpResponse::Ok().finish())
}
