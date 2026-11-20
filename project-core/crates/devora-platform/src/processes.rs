//! The process table and signals, for finding and stopping a process tree.

/// One running process. A start time identifies it together with its pid,
/// since pids are reused; it is opaque, and only ever compared for equality on
/// the same machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub started: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Stop,
    Kill,
}

pub trait ProcessTable {
    /// Every process this user can see.
    fn list(&self) -> Result<Vec<ProcessInfo>, String>;
    /// One process, if it is still running. A zombie, dead but not yet reaped
    /// by its parent, isn't.
    fn get(&self, pid: u32) -> Option<ProcessInfo>;
    fn signal(&self, pid: u32, signal: Signal) -> Result<(), String>;
}

pub struct SystemProcessTable;

impl ProcessTable for SystemProcessTable {
    fn list(&self) -> Result<Vec<ProcessInfo>, String> {
        Ok(pids()?.into_iter().filter_map(info).collect())
    }

    fn get(&self, pid: u32) -> Option<ProcessInfo> {
        info(pid)
    }

    fn signal(&self, pid: u32, signal: Signal) -> Result<(), String> {
        let number = match signal {
            Signal::Stop => libc::SIGSTOP,
            Signal::Kill => libc::SIGKILL,
        };
        let pid = libc::pid_t::try_from(pid).map_err(|_| format!("invalid pid {pid}"))?;
        if pid <= 0 {
            return Err(format!("refusing to signal pid {pid}"));
        }
        if unsafe { libc::kill(pid, number) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        // Already gone is what signalling it was for.
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!("signalling process {pid}: {error}"))
        }
    }
}

#[cfg(target_os = "macos")]
fn pids() -> Result<Vec<u32>, String> {
    // The table can grow between sizing and listing, so it is sized with room to spare.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if count < 0 {
        return Err(format!(
            "listing processes: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut pids = vec![0 as libc::pid_t; count as usize + 64];
    let size = (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
    let listed = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
    if listed < 0 {
        return Err(format!(
            "listing processes: {}",
            std::io::Error::last_os_error()
        ));
    }
    pids.truncate(listed as usize);
    Ok(pids
        .into_iter()
        .filter(|pid| *pid > 0)
        .map(|pid| pid as u32)
        .collect())
}

#[cfg(target_os = "macos")]
fn info(pid: u32) -> Option<ProcessInfo> {
    let mut bsd: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut bsd as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (read == size && bsd.pbi_status != libc::SZOMB).then(|| ProcessInfo {
        pid: bsd.pbi_pid,
        ppid: bsd.pbi_ppid,
        started: bsd.pbi_start_tvsec * 1_000_000 + bsd.pbi_start_tvusec,
    })
}

#[cfg(target_os = "linux")]
fn pids() -> Result<Vec<u32>, String> {
    let entries =
        std::fs::read_dir("/proc").map_err(|error| format!("listing processes: {error}"))?;
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect())
}

#[cfg(target_os = "linux")]
fn info(pid: u32) -> Option<ProcessInfo> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name, in parentheses, may itself contain spaces or parentheses.
    let fields: Vec<&str> = stat[stat.rfind(')')? + 2..].split(' ').collect();
    if fields.first() == Some(&"Z") {
        return None;
    }
    Some(ProcessInfo {
        pid,
        // After the name: state, ppid, ..., starttime (the 22nd field of the whole line).
        ppid: fields.get(1)?.parse().ok()?,
        started: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn the_table_finds_this_process_and_a_child_with_its_parent() {
        let table = SystemProcessTable;
        let me = table.get(std::process::id()).expect("this process");
        assert_eq!(
            me,
            table.get(std::process::id()).unwrap(),
            "a start time is stable"
        );
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let listed = table.list().unwrap();
        let found = listed
            .iter()
            .find(|process| process.pid == child.id())
            .expect("the child is listed");
        assert_eq!(found.ppid, std::process::id());
        assert!(listed.contains(&me));
        table.signal(child.id(), Signal::Kill).unwrap();
        while table.get(child.id()).is_some() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Gone before it's reaped: a zombie isn't running.
        child.wait().unwrap();
        assert_eq!(table.get(child.id()), None);
        // Signalling a process that is gone is not an error.
        table.signal(child.id(), Signal::Kill).unwrap();
    }

    #[test]
    fn a_new_process_has_another_start_time() {
        let table = SystemProcessTable;
        let mut first = Command::new("sleep").arg("30").spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut second = Command::new("sleep").arg("30").spawn().unwrap();
        let (a, b) = (
            table.get(first.id()).unwrap(),
            table.get(second.id()).unwrap(),
        );
        assert_ne!(a.started, b.started);
        for child in [&mut first, &mut second] {
            child.kill().unwrap();
            child.wait().unwrap();
        }
    }
}
