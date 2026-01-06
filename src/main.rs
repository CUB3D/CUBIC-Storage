#[deny(clippy::unwrap_used)]
pub mod bucket;
pub mod bucket_get_file;
pub mod file_location;
pub mod metadata;
pub mod path;
pub mod settings;

use crate::metadata::MetadataManager;
use crate::path::PathManager;
use actix_web::middleware::{Compress, Logger, NormalizePath, TrailingSlash};
use actix_web::web::Data;
use actix_web::{App, Error as AWError, HttpResponse, HttpServer, web};
use dotenv::dotenv;
use env_logger::Env;
use futures::StreamExt;

//TODO: bigs todos
// Add web ui for management
// add system for securing buckets
// add apis for getting state of buckets

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
            .service(bucket_get_file::get_file)
            .service(bucket::get_bucket_create)
            .service(bucket::put_bucket_upload)
            .service(bucket::bucket_verify)
            .service(bucket::delete_bucket_remove)
            .service(bucket::get_bucket_details)
    })
    .bind(host)?
    .run()
    .await;
    Ok(())
}
