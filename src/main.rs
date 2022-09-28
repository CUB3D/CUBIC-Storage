pub mod bucket;
pub mod settings;

use actix_web::get;
use actix_web::middleware::{Compress, Logger, NormalizePath, TrailingSlash};
use actix_web::{web, App, Error as AWError, HttpResponse, HttpServer};
use dotenv::dotenv;
use env_logger::Env;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

//TODO: bigs todos
// Add dotenv for config
// Add web ui for management
// Add metadata to buckets
// add system for securing buckets
// add apis for getting state of buckets

#[derive(Deserialize, Serialize)]
pub struct Blob {
    blob_name: String,
    blob_sha1: String,
}

#[derive(Serialize)]
pub struct Bucket {
    blobs: Vec<Blob>,
}

#[derive(Deserialize)]
pub struct BucketLocation {
    name: String,
}

#[derive(Deserialize)]
pub struct FileLocation {
    bucket_name: String,
    file_name: String,
}

#[get("/{bucket_name}/{file_name}")]
async fn get_file(file: web::Path<FileLocation>) -> Result<HttpResponse, AWError> {
    let path_str = format!("storage_root/{}/{}", &file.bucket_name, &file.file_name);

    let path = Path::new(&path_str);
    let file = File::open(path);

    if let Ok(mut file) = file {
        let mut content: Vec<u8> = Vec::new();
        file.read_to_end(&mut content)
            .expect("Unable to read entire blob");

        Ok(HttpResponse::Ok().body(content))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

async fn root_handler() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok().body("Success"))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let host = settings::get_host_domain();
    tracing::info!("Running on http://{}", host);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Compress::default())
            .wrap(NormalizePath::new(TrailingSlash::MergeOnly))
            .service(web::resource("/").to(root_handler))
            .service(get_file)
            .service(bucket::get_bucket_create)
            .service(bucket::put_bucket_upload)
            .service(bucket::bucket_verify)
    })
    .bind(host)?
    .run()
    .await
}
