use std::{
    fmt::Display,
    net::{SocketAddr, TcpListener, TcpStream},
    process::exit,
    rc::Rc,
    time::Duration,
};

use nix::{
    libc::{self, siginfo_t},
    sys::{
        signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal},
        socket::{
            setsockopt,
            sockopt::{ReceiveTimeout, ReuseAddr, ReusePort},
        },
        time::TimeVal,
    },
    unistd::getpid,
};

use crate::server::Error;
use crate::worker::Worker;

static mut RUNNING: bool = true;

extern "C" fn tcpworker_exit_signal_handler(sig_no: i32, si: *mut siginfo_t, _: *mut libc::c_void) {
    unsafe { RUNNING = false };

    let pid = getpid();
    let si_code = (unsafe { *si }).si_code;
    log::trace!(target:"tcpworker_exit_signal_handler", "{sig_no}/{si_code} received in TcpWorker[{pid}]");
}

fn register_signal() {
    if let Err(e) = unsafe {
        sigaction(
            Signal::SIGINT,
            &SigAction::new(
                SigHandler::SigAction(tcpworker_exit_signal_handler),
                SaFlags::SA_SIGINFO,
                SigSet::empty(),
            ),
        )
    } {
        log::error!(target: "WorkerManager.run", "sigaction failed: {e}");
    }
}

pub struct TcpWorker {
    timeout_ms: u64,
    tcp_process: Rc<dyn Process>,
    host: String,
}

pub struct TcpWorkerContext {
    pub listener: TcpListener,
}

impl Worker for TcpWorker {
    type Context = TcpWorkerContext;

    fn run(&self, context: &mut Self::Context) {
        while unsafe { RUNNING } {
            let stream_result = context.listener.accept();

            match stream_result {
                Ok((stream, client)) => {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(self.timeout_ms)));
                    let process_result = self.tcp_process.process(stream, &client);
                    match process_result {
                        Ok((r, w)) => log::info!("{} r:{} o:{}", client, r, w),
                        Err(err) => log::warn!("process failed {:?}", err),
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(err) => {
                    log::error!(target: "TcpWorker::run", "Accept failed: {err}");
                    exit(1);
                }
            }
        }
    }

    fn init(&self) -> Self::Context {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.init", "TcpWorker start[{pid}:{process}]");
        log::debug!(target: "TcpWorker.new", "TcpWorker start host: {}", &self.host);
        let listener = TcpListener::bind(&self.host).unwrap();

        setsockopt(&listener, ReceiveTimeout, &TimeVal::new(1, 0)).unwrap();
        setsockopt(&listener, ReuseAddr, &true).unwrap();
        setsockopt(&listener, ReusePort, &true).unwrap();

        register_signal();

        return Self::Context { listener };
    }

    fn cleanup(&self, _: &mut Self::Context) {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.cleanup", "TcpWorker stop[{pid}:{process}]");
    }
}

impl TcpWorker {
    pub fn new(timeout_ms: u64, host: String, tcp_process: Rc<dyn Process>) -> Self {
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

    use crate::server::{tcp::Process, Error};

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
        let received = String::from_utf8(v).unwrap();
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
