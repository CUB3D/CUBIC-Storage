use actix::*;

use actix_web::{HttpResponse, web, HttpServer, App, Error as AWError, middleware};
use serde::{Deserialize, Serialize};
use futures::future::Future;
use futures::stream::Stream;
use std::path::Path;
use std::fs::{File, create_dir};
use std::io::{Read, Write};
use walkdir::WalkDir;
use sha1::Sha1;

extern crate futures;

//fn walk_bucket(name: String) -> Vec<&'static Path> {
//    let mut paths = Vec::new();
//
//    for e in WalkDir::new(format!("storage_root/{}/", name)).into_iter().filter_map(|e| e.ok()) {
//        if e.metadata().unwrap().is_file() {
//            println!("{}", e.path().display());
//            paths.push(e.path().clone())
//        }
//    }
//
//    return paths;
//}

#[derive(Deserialize, Serialize)]
struct Blob {
    blob_name: String,
    blob_sha1: String,
}

#[derive(Serialize)]
struct Bucket {
    blobs: Vec<Blob>
}

fn bucket_verify(
    file: web::Path<BucketLocation>
) -> Result<HttpResponse, AWError> {
    let mut blobs = Vec::new();

    for e in WalkDir::new(format!("storage_root/{}/", &file.name)).into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            println!("{}", e.path().display());
            let path = e.path();

            let mut sha = Sha1::new();

            let mut blobFile = File::open(path).unwrap();
            let mut content = String::new();
            blobFile.read_to_string(&mut content);

            sha.update(content.as_bytes());

            let hex_string = sha.digest().to_string();
            let path_string = path.to_str().unwrap().replace(&format!("storage_root/{}/", &file.name), "");

            blobs.push(Blob {
                blob_name: path_string,
                blob_sha1: hex_string
            })
        }
    }

    Ok(HttpResponse::Ok().json(Bucket {
        blobs
    }))
}

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
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let path_str = format!("storage_root/{}/{}", &file.bucket_name, &file.file_name);
    let file = File::create(path_str);
//    if let Ok(mut file) = file {
//        data.concat2().then(|bytes| {
//            web::block(move || {
//                if let Ok(bytes) = bytes {
//                    file.write_all(&bytes).expect("Unable to save file")
//                }
//            }).from_err()
//        })
//    } else {
//        HttpResponse::BadRequest().finish()
//    }

    data.concat2()
        .then(|bytes|
            match bytes {
                Ok(res) => {
                    file.expect("Unable to open file").write_all(&res).expect("Unable to save file");
                    HttpResponse::Ok().finish()
                },
                Err(reason) => HttpResponse::InternalServerError().body(format!("{}", reason))
            }
        )
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
                web::put().to_async(bucket_upload)
            ))
            .service(web::resource("/api/bucket/{name}/verify").to(bucket_verify))
    })
        .bind("0.0.0.0:8080").unwrap()
        .start();

    system.run()
}
