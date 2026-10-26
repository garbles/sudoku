//! The real environment: the wall clock and new seeds.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::Env;
use crate::generator::Rng;

pub struct SystemEnv {
    rng: Rng,
}

impl SystemEnv {
    pub fn new() -> SystemEnv {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        SystemEnv { rng: Rng::new(t ^ (u64::from(std::process::id()) << 32)) }
    }
}

impl Default for SystemEnv {
    fn default() -> SystemEnv {
        SystemEnv::new()
    }
}

impl Env for SystemEnv {
    fn unix_time(&mut self) -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    }

    fn new_seed(&mut self) -> u64 {
        self.rng.next_u64()
    }
}

/// The settings file of xochitl. The device keeps the handedness of the user in it.
pub const XOCHITL_CONF: &str = "/home/root/.config/remarkable/xochitl.conf";

/// The handedness in the text of xochitl.conf: `RightHanded` in the section `[General]`. xochitl
/// writes `RightHanded=false` for a left-handed user (examined on the device). If the value is not
/// there, the user is right-handed, as in the default of the device.
pub fn conf_right_handed(conf: &str) -> bool {
    let mut general = false;
    for line in conf.lines().map(str::trim) {
        if line.starts_with('[') {
            general = line == "[General]";
        } else if let Some(v) = line.strip_prefix("RightHanded=").filter(|_| general) {
            return v.trim() != "false";
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handedness_comes_from_the_general_section() {
        assert!(!conf_right_handed("[General]\nA=1\nRightHanded=false\n[Wifi]\nB=2\n"));
        assert!(conf_right_handed("[General]\nRightHanded=true\n"));
        assert!(conf_right_handed("[General]\nA=1\n[Other]\nRightHanded=false\n"));
        assert!(conf_right_handed(""));
    }
}
