use args::Args;
use clap::Parser;
use nix::unistd::getpid;
use std::io::{Read, Write};
use std::rc::Rc;

use crate::{
    http::{
        handler::Handler,
        header::{HttpHeaderValue, content_length, content_type},
        http::Http1,
        response::HeaderSetter,
        value::HttpResponseCode,
    },
    server::{Server, ServerArgs, WorkerInfo},
    util::date::Date,
};

mod args;
mod http;
mod process;
mod server;
mod util;
mod worker;

struct SimpleHandler;

impl Handler for SimpleHandler {
    fn handle(&self, req: &mut http::request::HttpRequest, res: &mut http::response::HttpResponse) {
        res.set_response_code(HttpResponseCode::Ok);
        res.set_header(&content_type(HttpHeaderValue::Str("text/plain")));

        if let Err(e) = writeln!(res, "Echo response") {
            log::error!("error {}", e);
        }

        if let Err(e) = writeln!(res, "{} {} {:?}", req.method(), req.path(), req.param()) {
            log::error!("error {}", e);
        }

        for (k, v) in req.header().iter() {
            if let Err(e) = writeln!(res, "{}: {}", k, v.join(";")) {
                log::error!("error {}", e);
            }
        }

        let mut body = String::new();
        match &req.read_to_string(&mut body) {
            Ok(usize) => {
                let mut size: usize = 0;
                usize.clone_into(&mut size);
                res.set_header(&content_length(size));
                let _ = res.write_all(body.as_bytes());
            }
            Err(err) => {
                res.set_response_code(HttpResponseCode::InternalServerError);
                log::error!("read body error / {}", err);
                log::error!("readed {body}");
            }
        }
    }
}

fn main() {
    colog::basic_builder()
        .filter_level(log::LevelFilter::Info)
        .format(|f, record| {
            writeln!(
                f,
                "{} [{:>25}:{:4}] [pid: {:>6}] [{}]\t{} {}",
                Date::from_system_time(std::time::SystemTime::now()).to_rfc1123(),
                record.file().unwrap_or(""),
                record.line().unwrap_or(0),
                getpid(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .write_style(env_logger::fmt::WriteStyle::Always)
        .init();

    let arg = Args::parse();
    log::info!("httprs: {:?}", arg);

    let worker_infos = vec![WorkerInfo {
        host: arg.host.clone(),
        port: arg.port,
        worker: arg.worker,
        process: Rc::new(Http1::new(arg.max_header_size, SimpleHandler)),
    }];

    let mut server = Server::new(ServerArgs {
        worker_infos: worker_infos,
        timeout_ms: arg.timeout_ms,
    });
    server.open_server();
}
