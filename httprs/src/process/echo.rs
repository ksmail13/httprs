use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream},
};

use crate::process::{Error, Process};

#[derive(Debug)]
#[allow(dead_code)]
pub struct EchoProcess {
    pub prefix: Option<String>,
}

impl Process for EchoProcess {
    fn process(&self, mut stream: TcpStream, client: &SocketAddr) -> Result<(usize, usize), Error> {
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

#[cfg(test)]
mod test {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use crate::process::{Process, echo::EchoProcess};

    #[test]
    fn success() {
        let process = EchoProcess {
            prefix: Some("test".to_string()),
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local_addr = listener.local_addr().unwrap();

        let t = thread::spawn(move || {
            let (stream, remote_addr) = listener.accept().unwrap();
            return process.process(stream, &remote_addr);
        });

        let mut client = TcpStream::connect(local_addr).unwrap();
        let _ = client.set_read_timeout(Some(Duration::from_secs(1)));

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
}
