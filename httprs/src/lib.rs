use std::time::Duration;

pub mod http;
pub mod server;
pub mod util;
pub mod worker;

use crate::{
    http::{Http1, handler::Handler},
    server::{Server, ServerArgs, ServerWorkerInfo, tcp::TcpWorker},
};

pub struct Httprs<T: Handler> {
    process_cnt: u32,
    thread_cnt: u32,

    host: String,
    port: u16,

    max_header_size: usize,

    read_timeout: Duration,
    write_timeout: Duration,

    handler: Option<T>,
}

impl<T: Handler> Httprs<T> {
    pub fn new() -> Self {
        Httprs {
            process_cnt: 1,
            thread_cnt: 8,
            host: "0.0.0.0".to_string(),
            port: 8000,
            max_header_size: 8196,
            read_timeout: Duration::from_millis(300),
            write_timeout: Duration::from_millis(300),
            handler: None,
        }
    }

    pub fn set_process_cnt(&mut self, cnt: u32) {
        self.process_cnt = cnt;
    }

    pub fn set_thread_cnt(&mut self, cnt: u32) {
        self.thread_cnt = cnt;
    }

    pub fn set_max_header_size(&mut self, size: usize) {
        self.max_header_size = size;
    }

    pub fn set_read_timeout(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    pub fn set_write_timeout(&mut self, timeout: Duration) {
        self.write_timeout = timeout;
    }

    pub fn set_hanlder(&mut self, handler: T) {
        self.handler = Some(handler);
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn set_host(&mut self, host: &str) {
        self.host = host.to_owned();
    }

    pub fn run<'a>(&'a self) -> Result<(), String> {
        if self.handler.is_none() {
            return Err("Handler must exist".to_owned());
        }

        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| "Handler must exist".to_string())?;

        let processor = Http1::new(self.max_header_size, handler);
        let worker = TcpWorker::new(
            self.read_timeout.as_millis() as u64,
            format!("{}:{}", self.host, self.port),
            &processor,
        );
        let worker_infos = vec![ServerWorkerInfo {
            worker_count: self.process_cnt,
            worker: &worker,
        }];

        let mut server = Server::new(ServerArgs {
            worker_infos: worker_infos,
            timeout_ms: self.read_timeout.as_millis() as u64,
        });

        server.open_server();
        Ok(())
    }
}
