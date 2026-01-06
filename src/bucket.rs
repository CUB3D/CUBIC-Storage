use crate::file_location::FileLocation;
use crate::metadata::BlobMetadata;
use crate::metadata::MetadataManager;
use crate::{AWError, PathManager, StreamExt};
use actix_multipart::Multipart;
use actix_web::HttpResponse;
use actix_web::delete;
use actix_web::put;
use actix_web::web::{Data, Path as WebPath, Query};
use actix_web::{HttpRequest, get};
use serde::{Deserialize, Serialize};
use sha1::Digest;
use sha1::Sha1;
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;
use crate::settings::AppSettings;

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

#[derive(Deserialize)]
pub struct CreateBucketQuery {
    auth: String,
}

#[get("/api/bucket/{name}/create")]
pub async fn get_bucket_create(
    paths: Data<PathManager>,
    file: WebPath<BucketLocation>,
    auth: Query<CreateBucketQuery>,
    settings: Data<AppSettings>,
) -> Result<HttpResponse, AWError> {
    if auth.auth != settings.bucket_creation_key {
        return Ok(HttpResponse::BadRequest().finish());
    }
    
    let path = match paths.create_bucket(Path::new(&file.name)) {
        Some(b) => b,
        None => return Ok(HttpResponse::InternalServerError().finish()),
    };

    tokio::fs::create_dir(&*path).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/api/bucket/{name}/verify")]
pub async fn bucket_verify(
    paths: Data<PathManager>,
    file: WebPath<BucketLocation>,
) -> Result<HttpResponse, AWError> {
    let mut blobs = Vec::new();

    let path = match paths.get_bucket(Path::new(&file.name)) {
        Some(b) => b,
        None => return Ok(HttpResponse::InternalServerError().finish()),
    };

    for e in WalkDir::new(&*path).into_iter().filter_map(|e| e.ok()) {
        let m = match e.metadata() {
            Ok(e) => e,
            Err(_e) => {
                tracing::warn!("Failed to get metadata");
                return Ok(HttpResponse::InternalServerError().finish());
            }
        };

        if m.is_file() {
            let path = e.path();

            let mut sha = Sha1::new();

            let mut blob_file = File::open(path)?;
            let mut content = String::new();
            blob_file.read_to_string(&mut content)?;

            sha.update(content.as_bytes());

            let hex_string = format!("{:X}", sha.finalize());
            let path_string = path
                .to_str()
                .expect("Failed to convert path to string")
                .replace(&format!("storage_root/{}/", &file.name), "");

            blobs.push(Blob {
                blob_name: path_string,
                blob_sha1: hex_string,
            })
        }
    }

    Ok(HttpResponse::Ok().json(Bucket { blobs }))
}

#[derive(Serialize)]
struct FileUploadResult {
    access_key: String,
}

impl FileUploadResult {
    fn new(access_key: String) -> Self {
        Self { access_key }
    }
}

#[derive(Deserialize)]
pub struct BucketUploadQuery {
    auth: Option<String>,
}

#[put("/api/bucket/{bucket_name}/{file_name}/upload")]
pub async fn put_bucket_upload(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: WebPath<FileLocation>,
    mut data: Multipart,
    req: HttpRequest,
    auth: Query<BucketUploadQuery>,
    settings: Data<AppSettings>,
) -> Result<HttpResponse, AWError> {
    let bucket = match paths.get_bucket(Path::new(&file.bucket_name)) {
        Some(b) => b,
        None => return Ok(HttpResponse::InternalServerError().body("Failed to find bucket")),
    };

    // Trying to create a file that exists will failm even if it is deleted, so we do an early check here
    // If the file has been soft-deleted here then something else is being re-uploaded over it so we will remove it so the `create_bucket_file`
    // below won't fail.
    // For now if you never want something to be perminently lost, use a unique identifier for the path
    // TODO: consider having a deleted bucket and when a file is soft deleted move it to a unique path in that bucket and track this history in the metadata

    if let Some(p) = paths.get_bucket_file(&bucket, Path::new(&file.file_name)) {
        if let Ok(meta) = metadata.get_metadata(&p) {
            if meta.deletion_date.is_some() {
                tracing::warn!(
                    "Removing file {} as it it being overwritten, use unique paths to avoid this for now",
                    p.deref().display()
                );
                std::fs::remove_file(p.deref())?;
                match metadata.remove_metadata(&p) {
                    Ok(_) => {}
                    Err(_e) => {
                        tracing::warn!("Failed to remove metadata for {}", p.deref().display());
                        return Ok(HttpResponse::InternalServerError()
                            .body("Failed to create file, already exists"));
                    }
                }
            } else {
                tracing::warn!(
                    "Attempt to upload {} over existing file, delete it first",
                    p.deref().display()
                );
            }
        } else {
            tracing::warn!("File exists but has no metadata??? {}", p.deref().display());
            return Ok(
                HttpResponse::InternalServerError().body("Failed to create file, already exists")
            );
        }
    } else {
        // New files must have an upload key which is correct, existing files are checked by anti-overwrite, delete requires blob auth
        if let Some(auth) = &auth.auth {
            if *auth != settings.bucket_upload_key {
                return Ok(HttpResponse::BadRequest().finish());
            }
        } else {
            return Ok(HttpResponse::BadRequest().finish());
        }
    }

    let path = match paths.create_bucket_path(&bucket, Path::new(&file.file_name)) {
        Some(b) => b,
        None => {
            return Ok(
                HttpResponse::InternalServerError().body("Failed to create file, already exists")
            );
        }
    };

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
        Ok(_) => {}
        Err(_e) => return Ok(HttpResponse::InternalServerError().finish()),
    }
    let res = FileUploadResult::new(meta.access_key);

    Ok(HttpResponse::Ok().json(&res))
}

#[delete("/api/bucket/{bucket_name}/{file_name}/delete")]
pub async fn delete_bucket_remove(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: WebPath<FileLocation>,
    req: HttpRequest,
) -> Result<HttpResponse, AWError> {
    let bucket = match paths.get_bucket(Path::new(&file.bucket_name)) {
        Some(b) => b,
        None => {
            tracing::warn!("Failed to find bucket {}", &file.bucket_name);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let path = match paths.get_bucket_file(&bucket, Path::new(&file.file_name)) {
        Some(b) => b,
        None => {
            tracing::warn!("Failed to find bucket file {}", &file.file_name);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let mut meta = match metadata.get_metadata(&path) {
        Ok(b) => b,
        Err(_e) => {
            tracing::warn!("Failed to find metadata {}", &path.deref().display());
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    if meta.deletion_date.is_some() {
        return Ok(HttpResponse::BadRequest().body("Already deleted"));
    }

    // Get the given auth header
    let access_key = match req.headers().get("X-Blob-Access-Key") {
        Some(ct) => ct.to_str().expect("content type str").to_string(),
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    if access_key != meta.access_key {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    meta.deletion_date = Some(chrono::Utc::now());
    match metadata.save_metadata(&path, meta) {
        Ok(_) => {}
        Err(_e) => {
            tracing::warn!("Failed to save file metadata {}", &path.deref().display());
            return Ok(HttpResponse::InternalServerError().finish());
        }
    }

    Ok(HttpResponse::Ok().finish())
}
