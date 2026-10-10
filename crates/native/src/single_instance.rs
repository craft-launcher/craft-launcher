//! One launcher at a time: a second start asks the first to show its window, then exits.
//!
//! The first instance listens on a fixed loopback port. The only message is `show`, and the
//! worst another local program can do with it is bring the window forward.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const PORT: u16 = 47_413;
const MESSAGE: &[u8] = b"artcraft-launcher:show";

pub enum Instance {
    /// We're the only one. A message arrives on the receiver whenever another start asks to show
    /// the window; `wake` is called to repaint the (perhaps hidden) window.
    Primary(Receiver<()>),
    /// Another launcher is running and has been asked to show itself.
    Secondary,
}

/// Claims the instance; `wake` is set later (the UI context doesn't exist yet).
pub fn claim(wake: std::sync::Arc<std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>>) -> Instance {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    match TcpListener::bind(addr) {
        Ok(listener) => {
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut buf = [0u8; 64];
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let n = (&stream).read(&mut buf).unwrap_or(0);
                    if buf.get(..n) == Some(MESSAGE) {
                        if tx.send(()).is_err() {
                            break;
                        }
                        if let Some(wake) = wake.get() {
                            wake();
                        }
                    }
                }
            });
            Instance::Primary(rx)
        }
        Err(_) => match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            Ok(mut stream) => {
                let _ = stream.write_all(MESSAGE);
                Instance::Secondary
            }
            // The port is taken by something else: run anyway rather than refuse to start.
            Err(_) => Instance::Primary(channel().1),
        },
    }
}
