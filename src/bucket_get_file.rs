use crate::file_location::FileLocation;
use crate::metadata::MetadataManager;
use crate::path::PathManager;
use actix_web::get;
use actix_web::http::header;
use actix_web::web::Data;
use actix_web::{Error as AWError, HttpResponse, web};
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::Path;
use tracing::log;

#[get("/{bucket_name}/{file_name}")]
async fn get_file(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: web::Path<FileLocation>,
) -> Result<HttpResponse, AWError> {
    let bucket = match paths.get_bucket(Path::new(&file.bucket_name)) {
        Some(b) => b,
        None => {
            tracing::warn!("Failed to find bucket {}", &file.bucket_name);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let path = match paths.get_bucket_file(&bucket, Path::new(&file.file_name)) {
        Some(path) => path,
        None => {
            tracing::warn!("Failed to find bucket file {}", &file.file_name);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let mut file_meta = match metadata.get_metadata(&path) {
        Ok(m) => m,
        Err(_e) => {
            tracing::warn!("Failed to find metadata {}", &path.deref().display());
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    file_meta.download_count += 1;

    if let Err(e) = metadata.save_metadata(&path, &file_meta) {
        tracing::warn!("Failed to save metadata {}", e);
    }

    if file_meta.deletion_date.is_some() {
        log::warn!("Attempt to access soft-deleted file");
        return Ok(HttpResponse::NotFound().finish());
    }

    let file = File::open(path.deref());

    if let Ok(mut file) = file {
        let mut content: Vec<u8> = Vec::new();
        file.read_to_end(&mut content)?;

        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, file_meta.content_type))
            .body(content))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}
