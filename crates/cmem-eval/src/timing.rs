use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Timer {
    started: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed().as_millis()
    }
}
