//! Finds the eraser end of the pen (REQUIREMENTS.md §5.1).
//!
//! qtfb does not tell if the pen or the eraser touches the screen. The digitizer of the tablet reports
//! `BTN_TOOL_RUBBER` when the eraser is near the screen, before it touches. A thread reads the digitizer
//! and keeps this state. The app does not grab the device, thus the system continues to get the input.

use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const EV_KEY: u16 = 0x01;
const BTN_TOOL_PEN: u16 = 0x140;
const BTN_TOOL_RUBBER: u16 = 0x141;
const KEY_BYTES: usize = 0x300 / 8;

/// EVIOCGBIT(EV_KEY, len): _IOC(_IOC_READ, 'E', 0x20 + EV_KEY, len).
fn eviocgbit_key(len: usize) -> libc::c_ulong {
    ((2u64 << 30) | ((len as u64) << 16) | ((b'E' as u64) << 8) | (0x20 + EV_KEY as u64)) as libc::c_ulong
}

fn has_key(bits: &[u8], code: u16) -> bool {
    bits[(code / 8) as usize] & (1 << (code % 8)) != 0
}

/// Opens the input device that has the pen and the eraser tools.
pub fn find_digitizer() -> Option<File> {
    let mut entries: Vec<_> = std::fs::read_dir("/dev/input").ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if !path.file_name().map(|n| n.to_string_lossy().starts_with("event")).unwrap_or(false) {
            continue;
        }
        let Ok(f) = File::open(&path) else { continue };
        let mut bits = [0u8; KEY_BYTES];
        // SAFETY: the buffer has the size given in the request.
        let r = unsafe { libc::ioctl(f.as_raw_fd(), eviocgbit_key(KEY_BYTES) as _, bits.as_mut_ptr()) };
        if r >= 0 && has_key(&bits, BTN_TOOL_PEN) && has_key(&bits, BTN_TOOL_RUBBER) {
            eprintln!("sudoku: the eraser state comes from {}", path.display());
            return Some(f);
        }
    }
    None
}

/// Reads the digitizer for `seconds` and prints the changes of the tool (pen or eraser). The self
/// test uses it: turn the pen and touch the screen with each end.
pub fn watch_tools(seconds: u64) -> Result<(usize, usize), String> {
    let mut dev = find_digitizer().ok_or("no digitizer with BTN_TOOL_PEN and BTN_TOOL_RUBBER")?;
    let fd = dev.as_raw_fd();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    let (mut pen, mut rubber) = (0, 0);
    let mut buf = [0u8; 24 * 64];
    while std::time::Instant::now() < end {
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd.
        if unsafe { libc::poll(&mut pfd, 1, 200) } <= 0 {
            continue;
        }
        let Ok(n) = dev.read(&mut buf) else { break };
        for ev in buf[..n].chunks_exact(24) {
            let kind = u16::from_ne_bytes([ev[16], ev[17]]);
            let code = u16::from_ne_bytes([ev[18], ev[19]]);
            let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if kind == EV_KEY && (code == BTN_TOOL_PEN || code == BTN_TOOL_RUBBER) && value == 1 {
                let name = if code == BTN_TOOL_PEN { pen += 1; "pen" } else { rubber += 1; "eraser" };
                println!("self-test: tool near the screen: {name}");
            }
        }
    }
    Ok((pen, rubber))
}

/// The eraser state. If the digitizer is not available, the eraser is never active.
#[derive(Clone)]
pub struct Eraser {
    active: Arc<AtomicBool>,
}

impl Eraser {
    pub fn start() -> Eraser {
        let active = Arc::new(AtomicBool::new(false));
        if let Some(mut dev) = find_digitizer() {
            let flag = active.clone();
            std::thread::spawn(move || {
                // struct input_event on 64-bit Linux: timeval (16 bytes), type, code, value.
                let mut buf = [0u8; 24 * 64];
                while let Ok(n) = dev.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    for ev in buf[..n].chunks_exact(24) {
                        let kind = u16::from_ne_bytes([ev[16], ev[17]]);
                        let code = u16::from_ne_bytes([ev[18], ev[19]]);
                        let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                        if kind == EV_KEY && code == BTN_TOOL_RUBBER {
                            flag.store(value != 0, Ordering::SeqCst);
                        }
                    }
                }
            });
        } else {
            eprintln!("sudoku: no digitizer with an eraser found; the eraser is not available");
        }
        Eraser { active }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}
