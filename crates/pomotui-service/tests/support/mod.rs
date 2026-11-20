use std::io;
use std::sync::{Arc, Mutex};

use pomotui_platform::Clock;

#[derive(Clone)]
pub struct ManualClock(Arc<Mutex<(u64, i64)>>);

impl ManualClock {
    pub fn new(unix_seconds: i64) -> Self {
        Self(Arc::new(Mutex::new((0, unix_seconds))))
    }

    pub fn advance(&self, seconds: u64) {
        let mut time = self.0.lock().unwrap();
        time.0 += seconds;
        time.1 += i64::try_from(seconds).unwrap();
    }
}

impl Clock for ManualClock {
    type Error = io::Error;

    fn monotonic_seconds(&self) -> io::Result<u64> {
        Ok(self.0.lock().unwrap().0)
    }

    fn wall_seconds(&self) -> io::Result<i64> {
        Ok(self.0.lock().unwrap().1)
    }

    fn boot_id(&self) -> io::Result<String> {
        Ok("manual-clock".into())
    }
}
