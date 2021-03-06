#![feature(await_macro, async_await, futures_api)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

use actix_web;
use actix_web::{
    fs::NamedFile, fs::StaticFiles, http, middleware::Logger, server, App, HttpRequest,
    HttpResponse, Json, Path, ResponseError,
};
use std::path::PathBuf;

#[macro_use]
extern crate tera;
use tera::Tera;

use env_logger;

use std::fs::File;
use std::io::Read;

#[macro_use]
extern crate failure;

use log::info;

//mod realtime;
mod util;
use crate::util::*;

lazy_static! {
    static ref TEMPLATE: Tera = compile_templates!("templates/*.html");
}

#[derive(Debug, Fail)]
enum Error {
    #[fail(display = "Template Error: {}", _0)]
    Template(String),
    #[fail(display = "IO Error: {}", _0)]
    IO(std::io::Error),
    #[fail(display = "Compile Error")]
    Compile,
    #[fail(display = "Uri Error: {}", _0)]
    UriSegment(actix_web::error::UriSegmentError),
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().finish()
    }
}

const DEFAULT_CODE: &str = "@require: stdjabook

document (|
  title = {\\SATySFi; Playground};
  author = {Your Name};
  show-title = true;
  show-toc = false;
|) '<
    +p { Hello, \\SATySFi; Playground! }
>";

const DEFAULT_PDF: &str =
    "5652e501b1475942edee2a69e75891cddb7c26195e9435da7a06fca70a3d6ffe";

fn permalink(query: Path<String>) -> Result<HttpResponse, Error> {
    log::info!("permalink query = {}", query);
    let s = TEMPLATE
        .render(
            "index.html",
            &create_context(query.into_inner(), DEFAULT_CODE.into(), DEFAULT_PDF.into()),
        )
        .map_err(|e| Error::Template(e.description().to_owned()))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

fn index(_: HttpRequest) -> Result<HttpResponse, Error> {
    let s = TEMPLATE
        .render(
            "index.html",
            &create_context(
                DEFAULT_PDF.into(),
                DEFAULT_CODE.into(),
                DEFAULT_PDF.into(),
            ),
        )
        .map_err(|e| Error::Template(e.description().to_owned()))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

/*
// https://github.com/SergioBenitez/Rocket/issues/95#issuecomment-354824883
struct CachedFile(NamedFile);

impl<'a> response::Responder<'a> for CachedFile {
    fn respond_to(self, req: &rocket::Request) -> response::Result<'a> {
        response::Response::build_from(self.0.respond_to(req)?)
            .raw_header("Cache-Control", "max-age=86400") // a day
            .ok()
    }
}
*/

async fn files(req: HttpRequest) -> Result<NamedFile, Error> {
    use futures::prelude::*;

    let hash: PathBuf = req
        .match_info()
        .query("hash")
        .map_err(Error::UriSegment)?;
    match NamedFile::open(make_output_path(&hash)) {
        Ok(file) => Ok(file),
        _ => {
            let mut f = File::open(make_input_path(&hash)).map_err(Error::IO)?;
            let mut content = vec![];
            f.read_to_end(&mut content).map_err(Error::IO)?;
            let output = tokio::await!(compile(&content).map_err(|e| {
                info!("compile error: {:?}", e);
                Error::Compile
            }))?;
            NamedFile::open(output.name).map_err(Error::IO)
        }
    }
}

async fn compile_handler(input: Json<Input>) -> Result<Json<Output>, Error> {
    match tokio::await!(compile(input.content.as_bytes())) {
        Ok(x) => Ok(Json(x)),
        Err(e) => {
            info!("compile error: {:?}", e);
            Err(Error::Compile)
        }
    }
}

fn main() {
    use futures::prelude::*;

    env_logger::init();

    server::new(|| {
        App::new()
            .resource("/", |r| r.method(http::Method::GET).with(index))
            .handler("/assets", StaticFiles::new("./assets").unwrap())
            .resource("/files/{hash}", |r| {
                r.method(http::Method::GET)
                    .with_async(|x| Box::pin(files(x)).compat())
            })
            .resource("/compile", |r| {
                r.method(http::Method::POST)
                    .with_async(|x| Box::pin(compile_handler(x)).compat())
            })
            .resource("/permalink/{query}", |r| {
                r.method(http::Method::GET).with(permalink)
            })
            .middleware(Logger::default())
    })
    .bind("localhost:8000")
    .unwrap()
    .run();
}
