use std::{
    fmt::Display,
    io::ErrorKind,
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use nix::sys::signal::{SigSet, Signal::SIGINT};
use socket2::{Domain, Protocol, Socket, Type};

#[cfg(any(target_os = "macos"))]
use crate::server::acceptor::KqueueAcceptor;

#[cfg(any(target_os = "linux"))]
use crate::server::acceptor::EpollAcceptor;

use crate::server::{
    Error,
    acceptor::{
        Accepted::{self},
        Acceptor,
    },
};
use crate::worker::Worker;

fn register_signal() -> SigSet {
    let mut sigset = SigSet::empty();

    sigset.add(SIGINT);
    sigset.thread_block().expect("Signal must block");
    return sigset;
}

pub struct TcpWorker<'a> {
    timeout_ms: u64,
    tcp_process: &'a (dyn Process + 'a),
    host: String,
}

pub struct TcpWorkerContext {
    pub listener: TcpListener,
    pub sigset: SigSet,
}

impl Worker for TcpWorker<'_> {
    type Context = TcpWorkerContext;

    fn run(&self, context: &mut Self::Context) {
        #[cfg(any(target_os = "linux"))]
        let mut acceptor = EpollAcceptor::new(&context.listener, &context.sigset).unwrap();

        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        let mut acceptor = KqueueAcceptor::new(&context.listener, &context.sigset).unwrap();

        loop {
            match acceptor.accept(1000 as u16) {
                Ok(accepted) => {
                    log::trace!(target: "TcpWorker::run", "Accepted event: {}", &accepted.len());
                    for event in accepted {
                        match event {
                            Accepted::Connection((stream, addr)) => {
                                let timeout = Some(Duration::from_millis(self.timeout_ms));
                                let _ = stream.set_write_timeout(timeout);
                                stream
                                    .set_nonblocking(false)
                                    .expect("accepted stream must blocking");
                                let process_result = self.tcp_process.process(stream, &addr);
                                match process_result {
                                    Ok((r, w)) => log::info!("{} r:{} o:{}", addr, r, w),
                                    Err(err) => log::warn!("process failed {:?}", err),
                                }
                            }
                            Accepted::Signal(signal_info) => {
                                log::trace!(target: "TcpWorker::run", "Signal received {:?}", signal_info);
                                return;
                            }
                            Accepted::Timeout => (),
                        }
                    }
                }
                Err(err) => {
                    log::trace!(target: "TcpWorker::run", "Accept failed: {}", err);
                    return;
                }
            }
        }
    }

    fn init(&self) -> Self::Context {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.init", "TcpWorker start[{pid}:{process}]");
        log::debug!(target: "TcpWorker.new", "TcpWorker start host: {}", &self.host);

        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
        socket
            .set_nonblocking(true)
            .and_then(|_| socket.set_reuse_address(true))
            .and_then(|_| socket.set_reuse_port(true))
            .and_then(|_| {
                self.host
                    .parse::<SocketAddr>()
                    .map_err(|e| std::io::Error::new(ErrorKind::AddrNotAvailable, e))
            })
            .and_then(|addr| socket.bind(&addr.into()))
            .and_then(|_| socket.listen(128))
            .unwrap();
        let listener = socket.into();

        let sigset = register_signal();

        return Self::Context { listener, sigset };
    }

    fn cleanup(&self, _: &mut Self::Context) {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.cleanup", "TcpWorker stop[{pid}:{process}]");
    }
}

impl<'a> TcpWorker<'a> {
    pub fn new(timeout_ms: u64, host: String, tcp_process: &'a (dyn Process + 'a)) -> Self {
        return Self {
            timeout_ms,
            tcp_process,
            host,
        };
    }
}

#[allow(dead_code)]
pub trait Process {
    fn process(&self, stream: TcpStream, client_addr: &SocketAddr)
    -> Result<(usize, usize), Error>;

    fn name(&self) -> String {
        return "process".to_string();
    }
}

impl Display for dyn Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f.write_str(&self.name());
    }
}

mod test {
    use std::{
        io::{ErrorKind, Read, Write},
        net::SocketAddr,
    };

    use crate::server::{Error, tcp::Process};

    #[test]
    fn success() {
        let process = EchoProcess {
            prefix: Some("test".to_string()),
        };

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let local_addr = listener.local_addr().unwrap();

        let t = std::thread::spawn(move || {
            let (stream, remote_addr) = listener.accept().unwrap();
            process.process(stream, &remote_addr)
        });

        let mut client = std::net::TcpStream::connect(local_addr).unwrap();
        let _ = client.set_read_timeout(Some(std::time::Duration::from_secs(1)));

        let written = client.write("echo".as_bytes()).unwrap();
        let _ = client.flush();

        let mut v = vec![0; written + 6];
        client.read_exact(&mut v).unwrap();
        let received = String::from_utf8_lossy(&v);
        println!("received : {}", received);
        assert_eq!("test: echo".to_string(), received);

        drop(client);

        let (readed, writed) = t.join().unwrap().unwrap();
        assert_eq!(readed, 4);
        assert_eq!(writed, 4);
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct EchoProcess {
        pub prefix: Option<String>,
    }

    impl Process for EchoProcess {
        fn process(
            &self,
            mut stream: std::net::TcpStream,
            client: &SocketAddr,
        ) -> Result<(usize, usize), Error> {
            let pid = nix::unistd::getpid();
            let mut all_readed = 0;
            let mut all_writed = 0;

            let mut bufs: Vec<u8> = vec![0; 1024];

            loop {
                let read_result = stream.read(&mut bufs);

                let echo_result = match read_result {
                    Ok(readed) => {
                        if readed == 0 {
                            break;
                        }
                        all_readed += readed;
                        if let Some(prefix) = &self.prefix {
                            let _ = stream.write(prefix.as_bytes());
                            let _ = stream.write(": ".as_bytes());
                        }

                        let received = &bufs[..readed];

                        log::debug!(
                            "Echo server received : {}",
                            String::from_utf8(received.to_vec()).unwrap()
                        );
                        stream.write(received)
                    }
                    Err(ref read_err) if read_err.kind() == ErrorKind::WouldBlock => Ok(0),
                    Err(ref read_err) => {
                        match read_err.kind() {
                            ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => break,
                            _ => {
                                log::error!(target: "MainWorker.process", "Read error: {}", read_err.kind())
                            }
                        };
                        stream.write_fmt(format_args!("{read_err}")).map(|_| 0)
                    }
                };

                if echo_result.is_err() {
                    break;
                }

                let _ = stream.flush();

                all_writed += echo_result.unwrap();
            }

            log::info!(target:"access log", "{pid} {client} {all_readed} {all_writed}");

            return Ok((all_readed, all_writed));
        }

        fn name(&self) -> String {
            return "EchoProcess".to_string();
        }
    }
}
