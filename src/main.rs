pub mod bucket;
pub mod settings;

use crate::settings::AppSettings;
use actix_web::get;
use actix_web::http::header;
use actix_web::middleware::{Compress, Logger, NormalizePath, TrailingSlash};
use actix_web::web::Data;
use actix_web::{web, App, Error as AWError, HttpResponse, HttpServer};
use dotenv::dotenv;
use env_logger::Env;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

//TODO: bigs todos
// Add web ui for management
// add system for securing buckets
// add apis for getting state of buckets
// - Use anyhow

#[derive(Deserialize)]
pub struct FileLocation {
    bucket_name: String,
    file_name: String,
}

pub struct BlobPath(PathBuf);
impl Deref for BlobPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct PathManager {
    settings: Data<AppSettings>,
}

impl PathManager {
    fn get_root(&self) -> PathBuf {
        PathBuf::from(&self.settings.storage_root)
    }

    /// Safely join a path to the root
    fn safe_join(&self, path: &Path) -> PathBuf {
        let root = self.get_root();

        let mut target = root.clone();

        for comp in path.components() {
            match comp {
                Component::Prefix(_) =>
                    /* Ignored, drive/share on windows is defined by our root */
                    {}
                Component::RootDir =>
                    /*Ignored, we always start from *our* root*/
                    {}
                Component::CurDir =>
                    /*Do nothing, we are always in current dir*/
                    {}
                Component::ParentDir =>
                    /* Ignore, we can't go up a dir */
                    {}
                Component::Normal(path) => {
                    target = target.join(path);
                }
            }
        }

        // Safety check, ALL paths MUST be under the root dir
        assert!(target.starts_with(root.as_os_str()));

        target
    }

    pub fn get_bucket(&self, bucket_name: &Path) -> PathBuf {
        self.safe_join(bucket_name)
    }

    pub fn get_bucket_file(&self, bucket_name: &Path, file: &Path) -> BlobPath {
        BlobPath(self.safe_join(&bucket_name.join(file)))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlobMetadata {
    content_type: String,
}

impl Default for BlobMetadata {
    fn default() -> Self {
        Self {
            content_type: "text".to_string(),
        }
    }
}

pub struct MetadataManager {
    sled: sled::Db,
}

impl MetadataManager {
    pub fn new() -> anyhow::Result<Self> {
        let sled = sled::open("./storage_root/metadata.db")?;

        Ok(Self { sled })
    }

    pub fn get_metadata(&self, blob_path: &BlobPath) -> anyhow::Result<BlobMetadata> {
        let meta = self.sled.get(blob_path.as_os_str().as_bytes())?;

        let meta = match meta {
            Some(data) => {
                let data_str = String::from_utf8(data.to_vec())?;
                serde_json::from_str(&data_str)?
            }
            None => BlobMetadata::default(),
        };

        Ok(meta)
    }

    fn create_metadata(&self, blob_path: &BlobPath, metadata: &BlobMetadata) -> anyhow::Result<()> {
        let meta_json = serde_json::to_string(&metadata)?;
        self.sled.remove(blob_path.as_os_str().as_bytes())?;
        self.sled
            .insert(blob_path.as_os_str().as_bytes(), meta_json.as_bytes())?;
        Ok(())
    }
}

#[get("/{bucket_name}/{file_name}")]
async fn get_file(
    paths: Data<PathManager>,
    metadata: Data<MetadataManager>,
    file: web::Path<FileLocation>,
) -> Result<HttpResponse, AWError> {
    let path = paths.get_bucket_file(Path::new(&file.bucket_name), Path::new(&file.file_name));
    let metadata = match metadata.get_metadata(&path) {
        Ok(m) => m,
        Err(_e) => return Ok(HttpResponse::InternalServerError().finish()),
    };
    tracing::info!("Got metadata {:?}", metadata);

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

    let settings = Data::new(settings::get_app_settings()?);
    let path_manager = Data::new(PathManager {
        settings: Data::clone(&settings),
    });
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
    })
    .bind(host)?
    .run()
    .await;
    Ok(())
}
