//! Background scheduling for app-owned worker threads.
pub trait ThreadPriority {
    fn background(&self) -> Result<(), String>;
}

pub struct SystemThreadPriority;

impl ThreadPriority for SystemThreadPriority {
    fn background(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let result = unsafe {
            libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_BACKGROUND, 0)
        };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::setpriority(
                libc::PRIO_PROCESS,
                libc::syscall(libc::SYS_gettid) as u32,
                19,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let result = 0;
        if result == 0 {
            Ok(())
        } else {
            Err(format!("could not set background priority ({result})"))
        }
    }
}
