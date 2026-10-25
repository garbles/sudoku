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
