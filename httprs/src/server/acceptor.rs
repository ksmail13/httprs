use std::io::ErrorKind::WouldBlock;
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
use std::{ffi::c_long, os::fd::AsRawFd};
use std::{
    fmt::Display,
    net::{SocketAddr, TcpListener, TcpStream},
};

#[cfg(any(target_os = "linux"))]
use nix::sys::{
    epoll::*,
    signalfd::{SfdFlags, SignalFd},
};
use nix::{
    errno::Errno,
    sys::signal::{SigSet, Signal},
};
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
use nix::{
    libc::timespec,
    sys::event::{EvFlags, EventFilter, FilterFlag, KEvent, Kqueue},
};

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
    Signal(Signal),
    Timeout,
}

pub trait Acceptor {
    fn accept(&self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError>;
}

#[cfg(any(target_os = "linux"))]
pub struct EpollAcceptor<'a> {
    epoll: Epoll,
    tcp_listener: &'a TcpListener,
    signal_fd: SignalFd,
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

        events[..num_events]
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
                    let signal = Signal::try_from(sig.expect("signal must exist").ssi_signo as i32)
                        .map_err(|e| AcceptError::AcceptError(e))?;
                    Ok(Accepted::Signal(signal))
                }
            })
            .collect()
    }
}

#[cfg(any(target_os = "linux"))]
impl<'a> EpollAcceptor<'a> {
    pub fn new(tcp_listener: &'a TcpListener, sigset: &'a SigSet) -> Result<Self, &'static str> {
        let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC).map_err(|e| e.desc())?;
        epoll
            .add(&tcp_listener, EpollEvent::new(EpollFlags::EPOLLIN, 0))
            .map_err(|e| e.desc())?;

        sigset.thread_block().map_err(|e| e.desc())?;
        let signal_fd =
            SignalFd::with_flags(&sigset, SfdFlags::SFD_NONBLOCK).map_err(|e| e.desc())?;

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
pub struct KqueueAcceptor<'a> {
    kqueue: Kqueue,
    tcp_listener: &'a TcpListener,
    sigset: &'a SigSet,
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
impl Acceptor for KqueueAcceptor<'_> {
    fn accept(&self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError> {
        let ev = [];
        let mut ret_ev = [
            KEvent::new(
                0,
                EventFilter::EVFILT_AIO,
                EvFlags::empty(),
                FilterFlag::empty(),
                0,
                0,
            ),
            KEvent::new(
                0,
                EventFilter::EVFILT_AIO,
                EvFlags::empty(),
                FilterFlag::empty(),
                0,
                0,
            ),
        ];

        let ev_cnt = self
            .kqueue
            .kevent(
                &ev,
                &mut ret_ev,
                Some(timespec {
                    tv_sec: 0,
                    tv_nsec: timeout_ms as c_long * 1000000,
                }),
            )
            .map_err(|e| AcceptError::PollError(e))?;

        return ret_ev[..ev_cnt]
            .iter()
            .map(|event| {
                let event_filter = event.filter().map_err(|e| AcceptError::PollError(e))?;
                if event_filter == EventFilter::EVFILT_SIGNAL {
                    let signal = Signal::try_from(event.ident() as i32)
                        .map_err(|e| AcceptError::AcceptError(e))?;
                    return Ok(Accepted::Signal(signal));
                }

                let accept_res = self.tcp_listener.accept();
                if let Err(e) = accept_res {
                    if e.kind() == WouldBlock {
                        return Ok(Accepted::Timeout);
                    } else {
                        return Err(AcceptError::AcceptError(Errno::from_raw(
                            e.raw_os_error().unwrap_or_default(),
                        )));
                    }
                }
                return Ok(Accepted::Connection(accept_res.unwrap()));
            })
            .collect();
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
impl<'a> KqueueAcceptor<'a> {
    pub fn new(tcp_listener: &'a TcpListener, sigset: &'a SigSet) -> Result<Self, &'static str> {
        let kqueue = Kqueue::new().map_err(|e| e.desc())?;

        let mut event = vec![KEvent::new(
            tcp_listener.as_raw_fd() as usize,
            EventFilter::EVFILT_READ,
            EvFlags::EV_ADD | EvFlags::EV_ENABLE | EvFlags::EV_RECEIPT,
            FilterFlag::empty(),
            0,
            0,
        )];

        let mut signal_events = sigset
            .iter()
            .map(|s| {
                KEvent::new(
                    s as usize,
                    EventFilter::EVFILT_SIGNAL,
                    EvFlags::EV_ADD | EvFlags::EV_ENABLE | EvFlags::EV_RECEIPT,
                    FilterFlag::empty(),
                    0,
                    0,
                )
            })
            .collect();

        event.append(&mut signal_events);

        let mut res_ev = [];
        kqueue
            .kevent(
                &event,
                &mut res_ev,
                Some(timespec {
                    tv_sec: 0,
                    tv_nsec: 1000000,
                }),
            )
            .map_err(|e| e.desc())?;
        return Ok(Self {
            kqueue,
            tcp_listener,
            sigset,
        });
    }
}
