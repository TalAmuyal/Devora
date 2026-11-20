use devora_platform::processes::{ProcessInfo, ProcessTable, Signal};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

/// What identifies a CLI process: its pid, and its start time, since pids are
/// reused. Only a process matching both is ever signalled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliProcess {
    pub pid: u32,
    pub started: u64,
}

impl CliProcess {
    /// The running process with `pid`, if there is one.
    pub fn of(table: &impl ProcessTable, pid: u32) -> Option<Self> {
        table.get(pid).map(Self::from)
    }

    fn alive(&self, table: &impl ProcessTable) -> bool {
        table
            .get(self.pid)
            .is_some_and(|process| Self::from(process) == *self)
    }
}

impl From<ProcessInfo> for CliProcess {
    fn from(process: ProcessInfo) -> Self {
        Self {
            pid: process.pid,
            started: process.started,
        }
    }
}

/// Stops `root` and every process under it, and waits until they're gone.
/// Claude Code makes each background shell a session and process-group leader
/// of its own, so neither a process group nor a session reaches them, but
/// while the CLI runs they are its descendants. Its tree is frozen first
/// (`SIGSTOP`, walking the parent links again until no new process turns up),
/// so nothing forks or exits out of it, then killed.
///
/// The known limit: a descendant whose parent already exited has been
/// reparented to launchd (or init), so the walk no longer finds it. Freezing
/// first keeps that window to processes orphaned before the stop began.
///
/// Returns whether `root` was running; a pid whose start time differs is
/// another process and is left alone.
pub fn stop_tree(
    table: &impl ProcessTable,
    root: CliProcess,
    wait: Duration,
) -> Result<bool, String> {
    if !root.alive(table) {
        return Ok(false);
    }
    table.signal(root.pid, Signal::Stop)?;
    let mut tree = vec![root];
    // Whatever was frozen is killed even if the walk fails part way, since a
    // frozen process would otherwise never run or exit again.
    let walked = freeze_descendants(table, &mut tree);
    let mut killed = Ok(());
    // The CLI last, so a descendant is never left behind with no parent to find it by.
    for process in tree.iter().rev() {
        if process.alive(table) {
            if let Err(error) = table.signal(process.pid, Signal::Kill) {
                killed = killed.and(Err(error));
            }
        }
    }
    walked.and(killed)?;
    let deadline = Instant::now() + wait;
    while tree.iter().any(|process| process.alive(table)) {
        if Instant::now() > deadline {
            let left: Vec<_> = tree
                .iter()
                .filter(|process| process.alive(table))
                .map(|process| process.pid.to_string())
                .collect();
            return Err(format!("processes {} didn't exit", left.join(", ")));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(true)
}

/// Freezes every descendant of the processes in `tree`, adding them to it, and
/// walks the parent links again until no new process turns up.
fn freeze_descendants(table: &impl ProcessTable, tree: &mut Vec<CliProcess>) -> Result<(), String> {
    let mut seen: BTreeSet<u32> = tree.iter().map(|process| process.pid).collect();
    loop {
        let mut children: HashMap<u32, Vec<ProcessInfo>> = HashMap::new();
        for process in table.list()? {
            children.entry(process.ppid).or_default().push(process);
        }
        let found: Vec<CliProcess> = tree
            .iter()
            .flat_map(|parent| children.get(&parent.pid).into_iter().flatten())
            .filter(|process| !seen.contains(&process.pid))
            .map(|process| CliProcess::from(*process))
            .collect();
        if found.is_empty() {
            return Ok(());
        }
        for process in found {
            seen.insert(process.pid);
            // Collected before it's signalled, so a failed signal still gets the kill.
            tree.push(process);
            table.signal(process.pid, Signal::Stop)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_platform::processes::SystemProcessTable;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    /// A parent whose child made itself a session leader, as Claude Code's
    /// background shells do; returns the parent and the child's pid.
    fn tree() -> (Child, u32) {
        let mut parent = Command::new("python3")
            .args([
                "-c",
                "import os, sys, time\n\
                 pid = os.fork()\n\
                 if pid == 0:\n    os.setsid(); time.sleep(60); os._exit(0)\n\
                 print(pid, flush=True); time.sleep(60)",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(parent.stdout.as_mut().unwrap())
            .read_line(&mut line)
            .unwrap();
        (parent, line.trim().parse().unwrap())
    }

    /// A table over a fixed tree, recording signals, whose listing fails after
    /// the first walk.
    struct FailingTable {
        processes: Vec<ProcessInfo>,
        listed: std::cell::Cell<u32>,
        signals: std::cell::RefCell<Vec<(u32, Signal)>>,
    }

    impl ProcessTable for FailingTable {
        fn list(&self) -> Result<Vec<ProcessInfo>, String> {
            self.listed.set(self.listed.get() + 1);
            if self.listed.get() > 1 {
                return Err("listing processes failed".into());
            }
            Ok(self.processes.clone())
        }
        fn get(&self, pid: u32) -> Option<ProcessInfo> {
            let killed = self.signals.borrow().contains(&(pid, Signal::Kill));
            self.processes
                .iter()
                .find(|process| process.pid == pid && !killed)
                .copied()
        }
        fn signal(&self, pid: u32, signal: Signal) -> Result<(), String> {
            self.signals.borrow_mut().push((pid, signal));
            Ok(())
        }
    }

    #[test]
    fn a_walk_that_fails_part_way_still_kills_what_it_froze() {
        let process = |pid, ppid| ProcessInfo {
            pid,
            ppid,
            started: 1,
        };
        let table = FailingTable {
            processes: vec![process(10, 1), process(11, 10), process(12, 11)],
            listed: Default::default(),
            signals: Default::default(),
        };
        let root = CliProcess::of(&table, 10).unwrap();
        let error = stop_tree(&table, root, Duration::from_secs(1)).unwrap_err();
        assert_eq!(error, "listing processes failed");
        assert_eq!(
            *table.signals.borrow(),
            vec![
                (10, Signal::Stop),
                (11, Signal::Stop),
                (11, Signal::Kill),
                (10, Signal::Kill),
            ],
            "nothing it froze is left frozen"
        );
    }

    #[test]
    fn a_tree_is_stopped_with_its_session_leading_descendants() {
        let table = SystemProcessTable;
        let (mut parent, child) = tree();
        let root = CliProcess::of(&table, parent.id()).unwrap();
        assert!(table.get(child).is_some());
        assert!(stop_tree(&table, root, Duration::from_secs(5)).unwrap());
        assert_eq!(table.get(child), None, "the setsid'd child is stopped too");
        assert_eq!(table.get(parent.id()), None);
        parent.wait().unwrap();
        assert!(
            !stop_tree(&table, root, Duration::from_secs(5)).unwrap(),
            "a tree that's gone is no longer running"
        );
    }

    #[test]
    fn a_pid_with_another_start_time_is_another_process_and_left_alone() {
        let table = SystemProcessTable;
        let (mut parent, child) = tree();
        let live = CliProcess::of(&table, parent.id()).unwrap();
        let reused = CliProcess {
            started: live.started + 1,
            ..live
        };
        assert!(!stop_tree(&table, reused, Duration::from_secs(5)).unwrap());
        assert_eq!(table.get(parent.id()).map(CliProcess::from), Some(live));
        assert!(table.get(child).is_some());
        stop_tree(&table, live, Duration::from_secs(5)).unwrap();
        parent.wait().unwrap();
    }
}
