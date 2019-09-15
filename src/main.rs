use actix::*;

use actix_web_actors::{ws, HttpContext};
use actix_web::{HttpResponse, web, HttpRequest, HttpServer, App, Error as AWError, middleware};
use actix_web_actors::ws::Message;
use serde::Deserialize;
use futures::future::{Future, ok};
use std::net::SocketAddr;
use futures::stream::Stream;
use serde_json;
use std::path::Path;
use std::fs::{File, create_dir};
use std::io::{Read, Write};
use std::borrow::Borrow;

extern crate futures;
#[macro_use]
extern crate debug_rs;

#[derive(Deserialize)]
struct BucketLocation {
    name: String,
}

fn bucket_create(
    file: web::Path<BucketLocation>
) -> Result<HttpResponse, AWError> {

    let path_str = format!("storage_root/{}", &file.name);
    create_dir(path_str);

    Ok(HttpResponse::Ok().finish())
}

fn bucket_upload(
    file: web::Path<FileLocation>,
    data: web::Payload
) -> Result<HttpResponse, AWError> {

    let path_str = format!("storage_root/{}/{}", &file.bucket_name, &file.file_name);
    let file = File::create(path_str);
    if let Ok(mut file) = file {

        Arbiter::spawn_fn({
                              data.concat2().then(|bytes| {
                                  web::block(move || {
                                      if let Ok(bytes) = bytes {
                                          file.write_all(&bytes);
                                      }

                                      Ok(())
                                  }).from_err()
                              })
                          });
}

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct FileLocation {
    bucket_name: String,
    file_name: String
}

fn get_file(
    file: web::Path<FileLocation>
) -> Result<HttpResponse, AWError> {

    let path_str = format!("storage_root/{}/{}", &file.bucket_name,&file.file_name);

    let path = Path::new(&path_str);
    let file = File::open(path);

    if let Ok(mut file) = file {
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        Ok(HttpResponse::Ok().body(contents))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

fn root_handler() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok().body("Success"))
}

fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let system = actix::System::new("storage");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .service(web::resource("/").to(root_handler))
            .service(web::resource("/{bucket_name}/{file_name}").to(get_file))
            .service(web::resource("/api/bucket/{name}/create").to(bucket_create))
            .service(web::resource("/api/bucket/{bucket_name}/{file_name}/upload").route(
                web::post().to(bucket_upload)
            ))
    })
        .bind("0.0.0.0:8081").unwrap()
        .start();

    system.run()
}
