//! A client for the qtfb framebuffer of AppLoad (REQUIREMENTS.md §14.4).
//!
//! AppLoad starts the app with `QTFB_KEY` in the environment. The app connects to `/tmp/qtfb.sock`
//! (a SEQPACKET socket), asks for a framebuffer, writes its pixels into shared memory and sends update
//! messages. AppLoad sends the pen and touch input on the same socket.

use std::io;
use std::os::fd::RawFd;

pub const SOCKET_PATH: &str = "/tmp/qtfb.sock";

const MESSAGE_INITIALIZE: u8 = 0;
const MESSAGE_UPDATE: u8 = 1;
const MESSAGE_USERINPUT: u8 = 4;
const MESSAGE_SET_REFRESH_MODE: u8 = 5;
const MESSAGE_DEVICE_STATE_CHANGED: u8 = 7;
const MESSAGE_DEVICE_STATE_INIT: u8 = 8;
const STATE_CHANGED_REASON_ROTATION: i32 = 0;

/// A message of the server that the app uses.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Input(UserInput),
    /// The rotation of the tablet: 0, 1 (90° left), 2 (90° right) or 3 (180°).
    Rotation(i32),
}

/// RGB888, 1620x2160, 3 bytes for each pixel, 3 * 1620 bytes for each line.
pub const FBFMT_RMPP_RGB888: u8 = 1;

const UPDATE_ALL: i32 = 0;
const UPDATE_PARTIAL: i32 = 1;

pub const INPUT_TOUCH_PRESS: i32 = 0x10;
pub const INPUT_TOUCH_RELEASE: i32 = 0x11;
pub const INPUT_TOUCH_UPDATE: i32 = 0x12;
pub const INPUT_PEN_PRESS: i32 = 0x20;
pub const INPUT_PEN_RELEASE: i32 = 0x21;
pub const INPUT_PEN_UPDATE: i32 = 0x22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum RefreshMode {
    Ui = 4,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InitContents {
    key: i32,
    fb_type: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UpdateContents {
    kind: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[repr(C)]
union ClientContents {
    init: InitContents,
    update: UpdateContents,
    refresh_mode: i32,
}

#[repr(C)]
struct ClientMessage {
    kind: u8,
    contents: ClientContents,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InitResponse {
    shm_key: i32,
    shm_size: usize,
}

/// One input event: the type, the device (the touch point id), x, y and the pressure (0..100).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UserInput {
    pub kind: i32,
    pub dev_id: i32,
    pub x: i32,
    pub y: i32,
    pub d: i32,
}

#[repr(C)]
union ServerContents {
    init: InitResponse,
    input: UserInput,
    raw: [u8; 24],
}

#[repr(C)]
struct ServerMessage {
    kind: u8,
    contents: ServerContents,
}

pub struct Connection {
    fd: RawFd,
    shm: *mut u8,
    shm_size: usize,
}

fn last_error() -> io::Error {
    io::Error::last_os_error()
}

impl Connection {
    pub fn connect(key: i32) -> io::Result<Connection> {
        // SAFETY: plain socket calls with correctly sized structures.
        unsafe {
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
            if fd < 0 {
                return Err(last_error());
            }
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
            for (i, b) in SOCKET_PATH.bytes().enumerate() {
                addr.sun_path[i] = b as libc::c_char;
            }
            let len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
            if libc::connect(fd, &addr as *const _ as *const libc::sockaddr, len) != 0 {
                let e = last_error();
                libc::close(fd);
                return Err(e);
            }
            let mut conn = Connection { fd, shm: std::ptr::null_mut(), shm_size: 0 };
            conn.send(&ClientMessage {
                kind: MESSAGE_INITIALIZE,
                contents: ClientContents { init: InitContents { key, fb_type: FBFMT_RMPP_RGB888 } },
            })?;
            let mut msg: ServerMessage = std::mem::zeroed();
            let n = libc::recv(fd, &mut msg as *mut _ as *mut libc::c_void, std::mem::size_of::<ServerMessage>(), 0);
            if n < std::mem::size_of::<ServerMessage>() as isize || msg.kind != MESSAGE_INITIALIZE {
                libc::close(fd);
                return Err(io::Error::other(format!("qtfb: wrong answer to the init message (type {}, {n} bytes)", msg.kind)));
            }
            let init = msg.contents.init;
            if init.shm_size < 1620 * 2160 * 3 {
                libc::close(fd);
                return Err(io::Error::other(format!("qtfb: the framebuffer is too small ({} bytes)", init.shm_size)));
            }
            let name = std::ffi::CString::new(format!("/qtfb_{}", init.shm_key)).unwrap();
            let shm_fd = libc::shm_open(name.as_ptr(), libc::O_RDWR, 0);
            if shm_fd < 0 {
                return Err(last_error());
            }
            let ptr = libc::mmap(std::ptr::null_mut(), init.shm_size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, shm_fd, 0);
            libc::close(shm_fd);
            if ptr == libc::MAP_FAILED {
                return Err(last_error());
            }
            conn.shm = ptr as *mut u8;
            conn.shm_size = init.shm_size;
            Ok(conn)
        }
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// The shared framebuffer.
    pub fn pixels(&mut self) -> &mut [u8] {
        // SAFETY: the mapping is valid until drop, and only this struct writes to it.
        unsafe { std::slice::from_raw_parts_mut(self.shm, self.shm_size) }
    }

    fn send(&self, msg: &ClientMessage) -> io::Result<()> {
        // SAFETY: msg is a valid repr(C) structure.
        let n = unsafe {
            libc::send(self.fd, msg as *const _ as *const libc::c_void, std::mem::size_of::<ClientMessage>(), libc::MSG_NOSIGNAL)
        };
        if n < 0 {
            Err(last_error())
        } else {
            Ok(())
        }
    }

    pub fn update_all(&self) -> io::Result<()> {
        self.send(&ClientMessage {
            kind: MESSAGE_UPDATE,
            contents: ClientContents { update: UpdateContents { kind: UPDATE_ALL, x: 0, y: 0, w: 0, h: 0 } },
        })
    }

    pub fn update(&self, x: i32, y: i32, w: i32, h: i32) -> io::Result<()> {
        self.send(&ClientMessage {
            kind: MESSAGE_UPDATE,
            contents: ClientContents { update: UpdateContents { kind: UPDATE_PARTIAL, x, y, w, h } },
        })
    }

    pub fn set_refresh_mode(&self, mode: RefreshMode) -> io::Result<()> {
        self.send(&ClientMessage { kind: MESSAGE_SET_REFRESH_MODE, contents: ClientContents { refresh_mode: mode as i32 } })
    }

    /// Reads one message without a wait. Ok(None): no message now. Err: the connection closed.
    pub fn try_recv(&self) -> io::Result<Option<Event>> {
        loop {
            // SAFETY: the buffer is a zeroed ServerMessage of the correct size.
            let mut msg: ServerMessage = unsafe { std::mem::zeroed() };
            let n = unsafe {
                libc::recv(self.fd, &mut msg as *mut _ as *mut libc::c_void, std::mem::size_of::<ServerMessage>(), libc::MSG_DONTWAIT)
            };
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "qtfb: connection closed"));
            }
            if n < 0 {
                let e = last_error();
                return match e.kind() {
                    io::ErrorKind::WouldBlock => Ok(None),
                    io::ErrorKind::Interrupted => continue,
                    _ => Err(e),
                };
            }
            if msg.kind == MESSAGE_USERINPUT {
                // SAFETY: the server sent a user input message.
                return Ok(Some(Event::Input(unsafe { msg.contents.input })));
            }
            if msg.kind == MESSAGE_DEVICE_STATE_INIT || msg.kind == MESSAGE_DEVICE_STATE_CHANGED {
                // DeviceStateChangedContents: int reason, then int rotation.
                // SAFETY: the raw bytes of the union.
                let raw = unsafe { msg.contents.raw };
                let reason = i32::from_ne_bytes(raw[0..4].try_into().unwrap());
                if reason == STATE_CHANGED_REASON_ROTATION {
                    return Ok(Some(Event::Rotation(i32::from_ne_bytes(raw[4..8].try_into().unwrap()))));
                }
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: the mapping and the socket belong to this struct.
        unsafe {
            if !self.shm.is_null() {
                libc::munmap(self.shm as *mut libc::c_void, self.shm_size);
            }
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_sizes_agree_with_the_c_structures() {
        // ClientMessage: uint8_t + a union of 20 bytes (alignment 4). ServerMessage: uint8_t + a union
        // of 24 bytes (alignment 8, size_t).
        assert_eq!(std::mem::size_of::<ClientMessage>(), 24);
        assert_eq!(std::mem::size_of::<ServerMessage>(), 32);
        assert_eq!(std::mem::size_of::<UserInput>(), 20);
    }
}
