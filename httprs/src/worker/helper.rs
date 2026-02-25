use std::process::exit;

use nix::{
    errno::Errno,
    sys::{
        signal::{Signal, kill},
        wait::{WaitStatus, wait},
    },
    unistd::{ForkResult, Pid, fork},
};

use crate::worker::{error::WaitError, group::WorkerGroup};

pub trait ChildManager {
    fn make_child(&self, group: &WorkerGroup) -> Result<Pid, Errno>;
    fn wait(&self) -> Result<Pid, WaitError>;
    fn kill(&self, pid: Pid) -> Result<Pid, Errno>;
}

pub struct ProcessManager;

impl ChildManager for ProcessManager {
    fn make_child(&self, group: &WorkerGroup) -> Result<Pid, Errno> {
        return match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => Ok(child),
            Ok(ForkResult::Child) => {
                group.worker.init();
                group.worker.run();
                group.worker.cleanup();
                exit(0);
            }
            Err(err) => Err(err),
        };
    }

    fn wait(&self) -> Result<Pid, WaitError> {
        let wait_result = wait();
        return match wait_result {
            Ok(WaitStatus::Exited(pid, excode)) => {
                if excode == 0 {
                    Ok(pid)
                } else {
                    Err(WaitError::ErrorExit(pid, excode))
                }
            }
            Ok(ws) => Err(WaitError::NotExited(ws)),
            Err(e) => Err(WaitError::WaitFailed(e)),
        };
    }

    fn kill(&self, pid: Pid) -> Result<Pid, Errno> {
        log::trace!(target: "WorkerCleaner.kill", "kill {pid}");
        return kill(pid, Signal::SIGINT).map(|_| pid);
    }
}
