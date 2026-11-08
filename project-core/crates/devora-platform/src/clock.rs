//! Monotonic elapsed time that includes system sleep.
use std::time::Duration;

pub trait MonotonicClock: Send + Sync {
    fn now(&self) -> std::io::Result<Duration>;
}

pub struct ContinuousClock;

impl MonotonicClock for ContinuousClock {
    fn now(&self) -> std::io::Result<Duration> {
        #[cfg(target_os = "macos")]
        let clock = libc::CLOCK_MONOTONIC_RAW;
        #[cfg(not(target_os = "macos"))]
        let clock = libc::CLOCK_BOOTTIME;
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if unsafe { libc::clock_gettime(clock, &mut time) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Duration::new(time.tv_sec as u64, time.tv_nsec as u32))
    }
}
