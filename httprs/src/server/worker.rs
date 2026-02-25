use std::{
    collections::HashMap, net::TcpListener, os::fd::AsRawFd, process::exit, rc::Rc, time::Duration,
};

use nix::{
    libc::{self, close, siginfo_t},
    sys::{
        signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction},
        socket::{
            setsockopt,
            sockopt::{ReceiveTimeout, ReuseAddr, ReusePort},
        },
        time::TimeVal,
    },
    unistd::getpid,
};

use crate::process::Process;
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
    pub timeout_ms: u64,
    pub listeners: Rc<HashMap<String, TcpListener>>,
    pub host: String,
    pub tcp_process: Rc<dyn Process>,
}

impl Worker for TcpWorker {
    fn run(&self) {
        let tcp_listener_opt = self.listeners.get(&self.host);
        if tcp_listener_opt.is_none() {
            return;
        }

        let tcp_listener = tcp_listener_opt.unwrap();

        while unsafe { RUNNING } {
            let stream_result = tcp_listener.accept();

            match stream_result {
                Ok((stream, client)) => {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(self.timeout_ms)));
                    let process_result = self.tcp_process.process(stream, &client);
                    match process_result {
                        Ok((r, w)) => log::trace!("{} r:{} o:{}", client, r, w),
                        Err(err) => log::warn!("process failed {:?}", err),
                    }
                }
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        _ => {
                            log::error!(target: "TcpWorker::run", "Accept failed: {err}");
                            exit(1);
                        }
                    };
                }
            }
        }
    }

    fn init(&self) {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.init", "TcpWorker start[{pid}:{process}]");
        // close unnecessary sockets
        self.init_sockets();

        register_signal();
    }

    fn cleanup(&self) {
        let pid = nix::unistd::getpid();
        let process = &self.tcp_process.name();
        log::trace!(target: "TcpWorker.cleanup", "TcpWorker stop[{pid}:{process}]");
        if let Some(listener) = self.listeners.get(&self.host) {
            unsafe { close(listener.as_raw_fd()) };
        }
    }
}

impl TcpWorker {
    fn init_sockets(&self) {
        self.listeners
            .iter()
            .filter(|(host, _)| **host != self.host)
            .for_each(|(_, listener)| {
                let fd = listener.as_raw_fd();
                unsafe { close(fd) };
            });

        if let Some(listener) = self.listeners.get(&self.host) {
            let _ = setsockopt(listener, ReceiveTimeout, &TimeVal::new(1, 0));
            let _ = setsockopt(listener, ReuseAddr, &true);
            let _ = setsockopt(listener, ReusePort, &true);
        }
    }
}
