use crate::{AWError, Blob, Bucket, BucketLocation, FileLocation, StreamExt};
use actix_multipart::Multipart;
use actix_web::get;
use actix_web::put;
use actix_web::web::Path;
use actix_web::HttpResponse;
use sha1::Digest;
use sha1::Sha1;
use std::fs::File;
use std::io::Read;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

#[get("/api/bucket/{name}/create")]
pub async fn get_bucket_create(file: Path<BucketLocation>) -> Result<HttpResponse, AWError> {
    println!("file = {}", file.name);
    let path_str = format!("storage_root/{}", &file.name);
    tokio::fs::create_dir(path_str)
        .await
        .expect("Unable to create directory");

    Ok(HttpResponse::Ok().finish())
}

#[get("/api/bucket/{name}/verify")]
pub async fn bucket_verify(file: Path<BucketLocation>) -> Result<HttpResponse, AWError> {
    let mut blobs = Vec::new();

    for e in WalkDir::new(format!("storage_root/{}/", &file.name))
        .into_iter()
        .filter_map(|e| e.ok())
    {
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
    file: Path<FileLocation>,
    mut data: Multipart,
) -> Result<HttpResponse, AWError> {
    let path_str = format!("storage_root/{}/{}", &file.bucket_name, &file.file_name);
    let mut file = tokio::fs::File::create(path_str).await?;

    while let Some(item) = data.next().await {
        let mut field = item?;
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            // filesystem operations are blocking, we have to use threadpool
            file.write_all(&data).await?;
        }
    }

    Ok(HttpResponse::Ok().finish())
}
