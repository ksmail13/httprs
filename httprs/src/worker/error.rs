use nix::{errno::Errno, sys::wait::WaitStatus, unistd::Pid};

pub enum WaitError {
    ErrorExit(Pid, i32),
    WaitFailed(Errno),
    NotExited(WaitStatus),
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return match self {
            WaitError::ErrorExit(pid, exit_code) => {
                f.write_fmt(format_args!("ErrorExit({pid}, {exit_code})"))
            }
            WaitError::WaitFailed(errno) => f.write_fmt(format_args!("WaitFailed({errno})")),
            WaitError::NotExited(wait_status) => {
                f.write_fmt(format_args!("NotExited({wait_status:?})"))
            }
        };
    }
}
