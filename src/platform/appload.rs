//! The connection to AppLoad of a program that is the backend of a QML frontend (REQUIREMENTS.md
//! §14.4, INVARIANTS.md §18).
//!
//! AppLoad starts `backend/entry` with the path of a UNIX socket (SOCK_SEQPACKET) as argument 1. Each
//! message is two packets: a header (`launch::header`) and the text. `launch.rs` gives the message
//! types and what the program does for each message. No call of this module waits (LCH-7).

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use crate::launch;

/// The largest text that the program reads. The messages of AppLoad are short (a word or a count);
/// the socket discards the rest of a longer packet.
const MAX_TEXT_BYTES: usize = 4096;

/// AppLoad makes its socket on a thread while it starts the program. If the socket is not there
/// yet, the program tries again for this time.
const CONNECT_RETRY: Duration = Duration::from_secs(1);
const CONNECT_RETRY_STEP: Duration = Duration::from_millis(10);

pub struct Socket {
    fd: RawFd,
    /// The header of a message from AppLoad whose text is not here yet.
    header: Option<(i32, usize)>,
    /// The text of a message to AppLoad whose header is sent. It must go before all other packets.
    unsent_text: Option<Vec<u8>>,
    /// The newest pen area message that is not sent yet. An older message that waits has no more
    /// use: only the last area applies.
    unsent_area: Option<String>,
    /// True after a send failed (not after a full socket buffer): the log gets one line (LCH-7).
    send_failed: bool,
}

fn last_error() -> io::Error {
    io::Error::last_os_error()
}

/// The result of a send that does not wait.
enum Sent {
    Done,
    /// The buffer of the socket is full: send the packet again later.
    Full,
}

impl Socket {
    /// Connects to the socket of AppLoad at `path`.
    pub fn connect(path: &str) -> io::Result<Socket> {
        let start = Instant::now();
        loop {
            match Socket::connect_once(path) {
                Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) && start.elapsed() < CONNECT_RETRY => {
                    std::thread::sleep(CONNECT_RETRY_STEP);
                }
                result => return result,
            }
        }
    }

    fn connect_once(path: &str) -> io::Result<Socket> {
        // SAFETY: plain socket calls with a correctly sized address.
        unsafe {
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            if path.len() >= addr.sun_path.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("the socket path {path:?} is too long")));
            }
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
            if fd < 0 {
                return Err(last_error());
            }
            addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
            for (i, b) in path.bytes().enumerate() {
                addr.sun_path[i] = b as libc::c_char;
            }
            let len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
            if libc::connect(fd, &addr as *const _ as *const libc::sockaddr, len) != 0 {
                let e = last_error();
                libc::close(fd);
                return Err(e);
            }
            Ok(Socket::of(fd))
        }
    }

    fn of(fd: RawFd) -> Socket {
        Socket { fd, header: None, unsent_text: None, unsent_area: None, send_failed: false }
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// True if a packet waits for space in the buffer of the socket: the main loop then also wakes
    /// when the socket can take data.
    pub fn has_unsent(&self) -> bool {
        self.unsent_text.is_some() || self.unsent_area.is_some()
    }

    /// Gives the pen area `text` (`launch::pen_area_text`) to the frontend. The call never waits
    /// (LCH-7): if the socket cannot take the message now, `flush` sends it later, and a newer area
    /// replaces it. If the send fails, the app continues.
    pub fn send_pen_area(&mut self, text: String) {
        self.unsent_area = Some(text);
        self.flush();
    }

    /// Sends the packets that wait, while the socket takes them. The call never waits.
    pub fn flush(&mut self) {
        if let Err(e) = self.try_flush() {
            self.unsent_text = None;
            self.unsent_area = None;
            if !std::mem::replace(&mut self.send_failed, true) {
                eprintln!("sudoku: cannot send the pen area to the frontend: {e}");
            }
        }
    }

    fn try_flush(&mut self) -> io::Result<()> {
        // The text of a message whose header went: AppLoad reads it as the next packet.
        if let Some(text) = &self.unsent_text {
            match self.send_packet(text)? {
                Sent::Done => self.unsent_text = None,
                Sent::Full => return Ok(()),
            }
        }
        let Some(area) = self.unsent_area.take() else { return Ok(()) };
        match self.send_packet(&launch::header(launch::MSG_PEN_AREA, area.len()))? {
            Sent::Full => self.unsent_area = Some(area),
            Sent::Done => {
                if let Sent::Full = self.send_packet(area.as_bytes())? {
                    self.unsent_text = Some(area.into_bytes());
                }
            }
        }
        Ok(())
    }

    fn send_packet(&self, bytes: &[u8]) -> io::Result<Sent> {
        loop {
            // SAFETY: `bytes` is a valid buffer of this length.
            let n = unsafe { libc::send(self.fd, bytes.as_ptr() as *const libc::c_void, bytes.len(), libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL) };
            if n >= 0 {
                return Ok(Sent::Done);
            }
            let e = last_error();
            match e.kind() {
                io::ErrorKind::WouldBlock => return Ok(Sent::Full),
                io::ErrorKind::Interrupted => continue,
                _ => return Err(e),
            }
        }
    }

    /// Reads one message of AppLoad without a wait: its type and its text. Ok(None): no full message
    /// now. Err: the connection ended.
    pub fn try_recv(&mut self) -> io::Result<Option<(i32, String)>> {
        let mut buf = [0u8; MAX_TEXT_BYTES];
        let closed = || io::Error::new(io::ErrorKind::UnexpectedEof, "AppLoad: connection closed");
        let (kind, len) = match self.header {
            Some(h) => h,
            None => {
                let Some(n) = self.recv_packet(&mut buf)? else { return Ok(None) };
                // An empty packet in place of a header is the end of the connection.
                if n == 0 {
                    return Err(closed());
                }
                let h = launch::read_header(&buf[..n]).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("AppLoad: a header of {n} bytes")))?;
                self.header = Some(h);
                h
            }
        };
        // AppLoad sends the text packet also for an empty text. Only then is an empty packet a text.
        let Some(n) = self.recv_packet(&mut buf)? else { return Ok(None) };
        if n == 0 && len > 0 {
            return Err(closed());
        }
        self.header = None;
        Ok(Some((kind, String::from_utf8_lossy(&buf[..n.min(len)]).into_owned())))
    }

    /// Reads one packet without a wait: its length. Ok(None): no packet now. An empty packet and the
    /// end of the connection give the length 0.
    fn recv_packet(&self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        loop {
            // SAFETY: `buf` is a valid buffer of this length.
            let n = unsafe { libc::recv(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), libc::MSG_DONTWAIT) };
            if n >= 0 {
                return Ok(Some(n as usize));
            }
            let e = last_error();
            match e.kind() {
                io::ErrorKind::WouldBlock => return Ok(None),
                io::ErrorKind::Interrupted => continue,
                _ => return Err(e),
            }
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        // SAFETY: the socket belongs to this struct.
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connected pair: the socket of the program, and the end of AppLoad.
    fn pair() -> (Socket, RawFd) {
        let mut fds = [0; 2];
        // SAFETY: `fds` has space for the two descriptors.
        assert_eq!(unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) }, 0);
        (Socket::of(fds[0]), fds[1])
    }

    fn send(fd: RawFd, bytes: &[u8]) {
        // SAFETY: `bytes` is a valid buffer of this length.
        assert_eq!(unsafe { libc::send(fd, bytes.as_ptr() as *const libc::c_void, bytes.len(), 0) }, bytes.len() as isize);
    }

    fn recv(fd: RawFd) -> Vec<u8> {
        let mut buf = [0u8; 64];
        // SAFETY: `buf` is a valid buffer of this length.
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), libc::MSG_DONTWAIT) };
        assert!(n >= 0, "no packet");
        buf[..n as usize].to_vec()
    }

    #[test]
    fn a_message_is_a_header_packet_and_a_text_packet() {
        let (mut socket, appload) = pair();
        socket.send_pen_area(launch::pen_area_text(Some((4, 8, 140, 136))));
        assert_eq!(recv(appload), launch::header(launch::MSG_PEN_AREA, 11));
        assert_eq!(recv(appload), b"4 8 140 136");
        socket.send_pen_area(launch::pen_area_text(None));
        assert_eq!(recv(appload), launch::header(launch::MSG_PEN_AREA, 4));
        assert_eq!(recv(appload), b"none");
        assert!(!socket.has_unsent());
        // SAFETY: the test owns the descriptor.
        unsafe { libc::close(appload) };
    }

    #[test]
    fn a_message_of_appload_is_read_when_its_text_is_here() {
        // LCH-1: the end of the connection is an error, thus the program stops.
        let (mut socket, appload) = pair();
        assert_eq!(socket.try_recv().unwrap(), None);
        send(appload, &launch::header(launch::MSG_FRONTEND_CONNECTED, 1));
        assert_eq!(socket.try_recv().unwrap(), None, "the text is not here yet");
        send(appload, b"1");
        assert_eq!(socket.try_recv().unwrap(), Some((launch::MSG_FRONTEND_CONNECTED, "1".to_string())));
        // An empty text is an empty packet after the header. It is not the end of the connection.
        send(appload, &launch::header(7, 0));
        send(appload, b"");
        assert_eq!(socket.try_recv().unwrap(), Some((7, String::new())));
        send(appload, &launch::header(launch::MSG_STOP, 5));
        send(appload, b"close");
        assert_eq!(socket.try_recv().unwrap(), Some((launch::MSG_STOP, "close".to_string())));
        // SAFETY: the test owns the descriptor.
        unsafe { libc::close(appload) };
        assert!(socket.try_recv().is_err());
        // LCH-7: a send after the end of the connection does not stop the app.
        socket.send_pen_area(launch::pen_area_text(None));
        assert!(!socket.has_unsent());
    }
}
