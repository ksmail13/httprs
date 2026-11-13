#[cfg(any(target_os = "macos", target_os = "freebsd"))]
use std::ffi::c_long;
use std::io::ErrorKind::WouldBlock;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
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
    Connected(SocketAddr),
    Ready((TcpStream, SocketAddr)),
    Signal(Signal),
    Timeout,
}

pub trait Acceptor {
    fn accept(&mut self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError>;
    fn register(&mut self, tcp_stream: TcpStream) -> Result<(), Errno>;
}

#[cfg(any(target_os = "linux"))]
pub struct EpollAcceptor<'a> {
    epoll: Epoll,
    tcp_listener: &'a TcpListener,
    signal_fd: SignalFd,
}

#[cfg(any(target_os = "linux"))]
impl Acceptor for EpollAcceptor<'_> {
    fn accept(&mut self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError> {
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
                    self.register(tcp_stream)
                        .map_err(|e| AcceptError::PollError(e))?;
                    Ok(Accepted::Connected(socket_addr))
                } else if e.data() == 1 {
                    let sig = self
                        .signal_fd
                        .read_signal()
                        .map_err(|e| AcceptError::AcceptError(e))?;
                    let signal = Signal::try_from(sig.expect("signal must exist").ssi_signo as i32)
                        .map_err(|e| AcceptError::AcceptError(e))?;
                    Ok(Accepted::Signal(signal))
                } else {
                    let stream_fd = e.data() as i32;
                    let stream = unsafe { TcpStream::from_raw_fd(stream_fd) };
                    let _ = self.epoll.delete(&stream);
                    let peer_addr = stream.peer_addr().map_err(|e| {
                        AcceptError::AcceptError(Errno::from_raw(
                            e.raw_os_error().unwrap_or_default(),
                        ))
                    })?;
                    Ok(Accepted::Ready((stream, peer_addr)))
                }
            })
            .collect()
    }

    fn register(&mut self, tcp_stream: TcpStream) -> Result<(), Errno> {
        let raw_fd = tcp_stream.as_raw_fd() as u64;
        let res = self.epoll.add(
            &tcp_stream,
            EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLONESHOT, raw_fd),
        );
        let _ = tcp_stream.into_raw_fd();
        return res;
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
    new_conn: Vec<TcpStream>,
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
impl Acceptor for KqueueAcceptor<'_> {
    fn accept(&mut self, timeout_ms: u16) -> Result<Vec<Accepted>, AcceptError> {
        let ev: Vec<KEvent> = self
            .new_conn
            .drain(0..)
            .into_iter()
            .map(|stream| {
                let raw_fd = stream.into_raw_fd();
                KEvent::new(
                    raw_fd as usize,
                    EventFilter::EVFILT_READ,
                    EvFlags::EV_ADD | EvFlags::EV_ENABLE | EvFlags::EV_ONESHOT,
                    FilterFlag::empty(),
                    0,
                    0,
                )
            })
            .collect();

        let mut ret_ev = [KEvent::new(
            0,
            EventFilter::EVFILT_AIO,
            EvFlags::empty(),
            FilterFlag::empty(),
            0,
            0,
        ); 1024];

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

        let mut result = Vec::with_capacity(ev_cnt);

        for i in 0..ev_cnt {
            let event = ret_ev[i];
            let event_filter = event.filter().map_err(|e| AcceptError::PollError(e))?;
            if event_filter == EventFilter::EVFILT_SIGNAL {
                let signal = Signal::try_from(event.ident() as i32)
                    .map_err(|e| AcceptError::AcceptError(e))?;
                result.push(Accepted::Signal(signal));
                continue;
            }

            if event.ident() == self.tcp_listener.as_raw_fd().try_into().unwrap() {
                match self.tcp_listener.accept() {
                    Err(e) => {
                        if e.kind() == WouldBlock {
                            result.push(Accepted::Timeout);
                        } else {
                            return Err(AcceptError::AcceptError(Errno::from_raw(
                                e.raw_os_error().unwrap_or_default(),
                            )));
                        }
                    }
                    Ok(accept) => {
                        let _ = self.register(accept.0);
                        result.push(Accepted::Connected(accept.1));
                    }
                }

                continue;
            }

            let stream_fd = event.ident().try_into().expect("must valid fd");
            let stream = unsafe { TcpStream::from_raw_fd(stream_fd) };
            let peer_addr = stream.peer_addr().map_err(|e| {
                AcceptError::AcceptError(Errno::from_raw(e.raw_os_error().unwrap_or_default()))
            })?;
            result.push(Accepted::Ready((stream, peer_addr)));
        }

        return Ok(result);
    }

    fn register(&mut self, tcp_stream: TcpStream) -> Result<(), Errno> {
        self.new_conn.push(tcp_stream);
        return Ok(());
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
            new_conn: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use nix::{
        errno::Errno,
        sys::signal::{SigSet, Signal},
    };
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };

    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    use crate::server::acceptor::KqueueAcceptor;
    use crate::server::acceptor::{AcceptError, Accepted, Acceptor};

    #[cfg(any(target_os = "linux"))]
    type PlatformAcceptor<'a> = EpollAcceptor<'a>;

    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    type PlatformAcceptor<'a> = KqueueAcceptor<'a>;

    fn create_nonblocking_listener() -> TcpListener {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        listener
    }

    fn send_test_signal(sig: Signal) {
        #[cfg(any(target_os = "linux"))]
        nix::sys::signal::raise(sig).unwrap();

        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        nix::sys::signal::kill(nix::unistd::getpid(), sig).unwrap();
    }

    #[test]
    fn test_accept_error_display_and_traits() {
        let poll_err = AcceptError::PollError(Errno::EINVAL);
        assert_eq!(
            format!("{}", poll_err),
            format!("poll error with code: {}", Errno::EINVAL)
        );

        let accept_err = AcceptError::AcceptError(Errno::EAGAIN);
        assert_eq!(
            format!("{}", accept_err),
            format!("accept error with code: {}", Errno::EAGAIN)
        );

        let cloned = poll_err;
        assert_eq!(poll_err, cloned);
        assert_ne!(poll_err, accept_err);

        let err_ref: &dyn std::error::Error = &poll_err;
        assert!(err_ref.source().is_none());
    }

    #[test]
    fn test_acceptor_timeout() {
        let listener = create_nonblocking_listener();
        let sigset = SigSet::empty();
        let mut acceptor = PlatformAcceptor::new(&listener, &sigset).unwrap();

        let events = acceptor.accept(50).unwrap();
        assert!(
            events.is_empty() || events.iter().all(|e| matches!(e, Accepted::Timeout)),
            "Expected empty or Timeout events on timeout"
        );
    }

    #[test]
    fn test_acceptor_single_connection() {
        let listener = create_nonblocking_listener();
        let addr = listener.local_addr().unwrap();
        let sigset = SigSet::empty();
        let mut acceptor = PlatformAcceptor::new(&listener, &sigset).unwrap();

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"ping").unwrap();

        let connected_events = acceptor.accept(500).unwrap();
        assert_eq!(connected_events.len(), 1);
        assert!(matches!(
            &connected_events[0],
            Accepted::Connected(peer_addr) if *peer_addr == client.local_addr().unwrap()
        ));

        let events = acceptor.accept(500).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            Accepted::Ready((stream, peer_addr)) => {
                assert_eq!(*peer_addr, client.local_addr().unwrap());
                let mut stream_clone = stream.try_clone().unwrap();
                let mut buf = [0u8; 4];
                stream_clone.read_exact(&mut buf).unwrap();
                assert_eq!(&buf, b"ping");
            }
            _ => panic!("Expected Accepted::Ready"),
        }
    }

    #[test]
    fn test_acceptor_multiple_connections() {
        let listener = create_nonblocking_listener();
        let addr = listener.local_addr().unwrap();
        let sigset = SigSet::empty();
        let mut acceptor = PlatformAcceptor::new(&listener, &sigset).unwrap();

        let mut client1 = TcpStream::connect(addr).unwrap();
        let mut client2 = TcpStream::connect(addr).unwrap();
        client1.write_all(b"one").unwrap();
        client2.write_all(b"two").unwrap();

        let _ = acceptor.accept(500).unwrap();
        let mut accepted_addrs = Vec::new();
        for _ in 0..2 {
            let events = acceptor.accept(500).unwrap();
            for event in events {
                if let Accepted::Ready((_, peer_addr)) = event {
                    accepted_addrs.push(peer_addr);
                }
            }
            if accepted_addrs.len() == 2 {
                break;
            }
        }

        assert_eq!(accepted_addrs.len(), 2);
        assert!(accepted_addrs.contains(&client1.local_addr().unwrap()));
        assert!(accepted_addrs.contains(&client2.local_addr().unwrap()));
    }

    #[test]
    fn test_acceptor_signal() {
        let listener = create_nonblocking_listener();
        let mut sigset = SigSet::empty();
        sigset.add(Signal::SIGURG);
        sigset.thread_block().unwrap();

        let mut acceptor = PlatformAcceptor::new(&listener, &sigset).unwrap();

        send_test_signal(Signal::SIGURG);

        let events = acceptor.accept(500).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Accepted::Signal(Signal::SIGURG))),
            "Expected Accepted::Signal(SIGURG)"
        );
    }

    #[test]
    fn test_acceptor_connection_and_signal() {
        let listener = create_nonblocking_listener();
        let addr = listener.local_addr().unwrap();
        let mut sigset = SigSet::empty();
        sigset.add(Signal::SIGWINCH);
        sigset.thread_block().unwrap();

        let mut acceptor = PlatformAcceptor::new(&listener, &sigset).unwrap();

        let _client = TcpStream::connect(addr).unwrap();
        send_test_signal(Signal::SIGWINCH);

        let mut got_connection = false;
        let mut got_signal = false;

        for _ in 0..4 {
            let events = acceptor.accept(500).unwrap();
            for event in events {
                match event {
                    Accepted::Connected(_) => got_connection = true,
                    Accepted::Signal(Signal::SIGWINCH) => got_signal = true,
                    _ => {}
                }
            }
            if got_connection && got_signal {
                break;
            }
        }

        assert!(got_connection, "Expected Accepted::Connected");
        assert!(got_signal, "Expected Accepted::Signal(SIGWINCH)");
    }
}
