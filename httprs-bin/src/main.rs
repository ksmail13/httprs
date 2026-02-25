use args::Args;
use clap::Parser;
use nix::unistd::{getpid, gettid};
use std::cmp::min;
use std::io::{Read, Write};
use std::rc::Rc;

use httprs::{
    http::{
        handler::Handler,
        header::{HttpHeaderValue, content_type},
        http::Http1,
        response::HeaderSetter,
        value::HttpResponseCode,
    },
    server::{Server, ServerArgs, WorkerInfo},
    util::date::Date,
};

mod args;

struct SimpleHandler;

const BUF_SIZE: usize = 1024;

impl Handler for SimpleHandler {
    fn handle(
        &self,
        req: &mut httprs::http::request::HttpRequest,
        res: &mut httprs::http::response::HttpResponse,
    ) {
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

        let mut req_body_len = req
            .header()
            .get("Content-Length")
            .unwrap_or(&vec!["0"])
            .first()
            .unwrap()
            .parse::<usize>()
            .unwrap();

        let mut body_buf = [0; BUF_SIZE];
        while req_body_len > 0 {
            match &req.read(&mut body_buf[0..min(req_body_len, BUF_SIZE)]) {
                Ok(readed) => {
                    let _ = res.write_all(&body_buf[0..*readed]);
                    req_body_len -= *readed;
                }
                Err(err) => {
                    res.set_response_code(HttpResponseCode::InternalServerError);
                    log::error!("read body error / {}", err);
                    log::error!(
                        "readed {:?}, {:?}",
                        req.header()
                            .get("Content-Length")
                            .unwrap_or(&vec!["0"])
                            .first()
                            .unwrap(),
                        String::from_utf8_lossy(&body_buf)
                    );
                    return;
                }
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
                "{} [{:>25}:{:4}] [{:>6}:{:>6}] [{}]\t{} {}",
                Date::from_system_time(std::time::SystemTime::now()).to_rfc1123(),
                record.file().unwrap_or(""),
                record.line().unwrap_or(0),
                getpid(),
                gettid(),
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
