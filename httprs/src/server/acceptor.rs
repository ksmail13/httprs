#[cfg(any(target_os = "linux"))]
use std::io::ErrorKind::WouldBlock;
use std::{
    fmt::Display,
    net::{SocketAddr, TcpListener, TcpStream},
};

#[cfg(any(target_os = "linux"))]
use nix::sys::epoll::*;
use nix::{errno::Errno, libc::signalfd_siginfo, sys::signalfd::SignalFd};

#[cfg(any(target_os = "freebsd", target_os = "macos"))]
use nex::sys::kqueue::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptError {
    PollError(Errno),
    AcceptError(Errno),
}

impl std::error::Error for AcceptError {}

impl Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptError::PollError(e) => write!(f, "poll error with code: {}", e),
            AcceptError::AcceptError(e) => write!(f, "accept error with code: {}", e),
        }
    }
}

pub enum Accepted {
    Connection((TcpStream, SocketAddr)),
    Signal(Option<signalfd_siginfo>),
    Timeout,
}

pub trait Acceptor {
    fn accept(&self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError>;
}

#[cfg(any(target_os = "linux"))]
pub struct EpollAcceptor<'a> {
    epoll: Epoll,
    tcp_listener: &'a TcpListener,
    signal_fd: &'a SignalFd,
}

#[cfg(any(target_os = "linux"))]
impl Acceptor for EpollAcceptor<'_> {
    fn accept(&self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError> {
        let mut events = [EpollEvent::empty(); 2];

        let num_events = match self.epoll.wait(&mut events, timeout_ms) {
            Ok(n) => n,
            Err(Errno::EINTR) => return Ok(vec![Accepted::Timeout]),
            Err(e) => return Err(AcceptError::PollError(e)),
        };

        let results: Result<Vec<Accepted>, AcceptError> = events[..num_events]
            .iter()
            .map(|e| {
                if e.data() == 0 {
                    let accept_res = self.tcp_listener.accept();
                    if let Err(e) = accept_res {
                        if e.kind() == WouldBlock {
                            return Ok(Accepted::Timeout);
                        }
                        return Err(AcceptError::AcceptError(Errno::from_raw(
                            e.raw_os_error().unwrap_or(0),
                        )));
                    }
                    let (tcp_stream, socket_addr) = accept_res.unwrap();
                    Ok(Accepted::Connection((tcp_stream, socket_addr)))
                } else {
                    let sig = self
                        .signal_fd
                        .read_signal()
                        .map_err(|e| AcceptError::AcceptError(e))?;
                    Ok(Accepted::Signal(sig))
                }
            })
            .collect();

        results
    }
}

impl<'a> EpollAcceptor<'a> {
    pub fn new(
        tcp_listener: &'a TcpListener,
        signal_fd: &'a SignalFd,
    ) -> Result<Self, &'static str> {
        let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC).map_err(|e| e.desc())?;
        epoll
            .add(&tcp_listener, EpollEvent::new(EpollFlags::EPOLLIN, 0))
            .map_err(|e| e.desc())?;
        epoll
            .add(&signal_fd, EpollEvent::new(EpollFlags::EPOLLIN, 1))
            .map_err(|e| e.desc())?;

        Ok(Self {
            epoll,
            tcp_listener,
            signal_fd,
        })
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub struct KqueueAcceptor {}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
impl Acceptor for KqueueAcceptor {
    fn add<Fd: AsFd>(&self, fd: Fd) -> Result<(), AcceptError> {
        todo!()
    }

    fn accept(&self) -> Result<Accepted, AcceptError> {
        todo!()
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
impl Drop for KqueueAcceptor {
    fn drop(&mut self) {
        todo!()
    }
}
