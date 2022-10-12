pub mod bucket;
pub mod metadata;
pub mod path;
pub mod settings;

use crate::metadata::MetadataManager;
use crate::path::PathManager;
use actix_web::get;
use actix_web::http::header;
use actix_web::middleware::{Compress, Logger, NormalizePath, TrailingSlash};
use actix_web::web::Data;
use actix_web::{web, App, Error as AWError, HttpResponse, HttpServer};
use dotenv::dotenv;
use env_logger::Env;
use futures::StreamExt;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::Path;
use tracing::log;

//TODO: bigs todos
// Add web ui for management
// add system for securing buckets
// add apis for getting state of buckets

#[derive(Deserialize)]
pub struct FileLocation {
    bucket_name: String,
    file_name: String,
}

#[get("/{bucket_name}/{file_name}")]
async fn get_file(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: web::Path<FileLocation>,
) -> Result<HttpResponse, AWError> {
    let bucket = match paths.get_bucket(Path::new(&file.bucket_name)) {
        Some(b) => b,
        None => return Ok(HttpResponse::InternalServerError().finish()),
    };

    let path = match paths.get_bucket_file(&bucket, Path::new(&file.file_name)) {
        Some(path) => path,
        None => return Ok(HttpResponse::InternalServerError().finish()),
    };

    let metadata = match metadata.get_metadata(&path) {
        Ok(m) => m,
        Err(_e) => return Ok(HttpResponse::InternalServerError().finish()),
    };
    tracing::info!("Got metadata {:?}", metadata);

    if metadata.deletion_date.is_some() {
        log::warn!("Attempt to access soft-deleted file");
        return Ok(HttpResponse::NotFound().finish());
    }

    let file = File::open(path.deref());

    if let Ok(mut file) = file {
        let mut content: Vec<u8> = Vec::new();
        file.read_to_end(&mut content)
            .expect("Unable to read entire blob");

        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, metadata.content_type))
            .body(content))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

async fn root_handler() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok().body("Success"))
}

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let host = settings::get_host_domain();
    tracing::info!("Running on http://{}", host);

    let settings = Data::new(settings::AppSettings::from_env()?);
    let path_manager = Data::new(PathManager::new(Data::clone(&settings)));
    let metadata_manager = Data::new(MetadataManager::new()?);

    let _ = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Compress::default())
            .wrap(NormalizePath::new(TrailingSlash::MergeOnly))
            .app_data(settings.clone())
            .app_data(path_manager.clone())
            .app_data(metadata_manager.clone())
            .service(web::resource("/").to(root_handler))
            .service(get_file)
            .service(bucket::get_bucket_create)
            .service(bucket::put_bucket_upload)
            .service(bucket::bucket_verify)
            .service(bucket::delete_bucket_remove)
    })
    .bind(host)?
    .run()
    .await;
    Ok(())
}
