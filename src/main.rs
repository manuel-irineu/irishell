use std::env;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsFd, AsRawFd};

use nix::errno::Errno;
use nix::pty::{Winsize, openpty};
use nix::sys::termios::{SetArg, Termios, cfmakeraw, tcgetattr, tcsetattr};
use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, execvp, fork, setsid};

struct TerminalGuard {
    original: Termios,
}

impl TerminalGuard {
    fn enable_raw() -> Self {
        let stdin = io::stdin();

        let original = tcgetattr(stdin.as_fd()).expect("failed to read terminal attributes");

        let mut raw = original.clone();
        cfmakeraw(&mut raw);

        tcsetattr(stdin.as_fd(), SetArg::TCSANOW, &raw).expect("failed to enable raw mode");

        Self { original }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let stdin = io::stdin();

        let _ = tcsetattr(stdin.as_fd(), SetArg::TCSANOW, &self.original);
    }
}

fn terminal_size() -> Winsize {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut size) };

    if result == -1 {
        return Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
    }

    Winsize {
        ws_row: size.ws_row,
        ws_col: size.ws_col,
        ws_xpixel: size.ws_xpixel,
        ws_ypixel: size.ws_ypixel,
    }
}

fn write_all_fd(fd: libc::c_int, mut buffer: &[u8]) -> io::Result<()> {
    while !buffer.is_empty() {
        let result = unsafe { libc::write(fd, buffer.as_ptr().cast(), buffer.len()) };

        if result == -1 {
            let error = io::Error::last_os_error();

            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }

            return Err(error);
        }

        buffer = &buffer[result as usize..];
    }

    Ok(())
}

fn forward_io(master_fd: libc::c_int) -> io::Result<()> {
    let stdin_fd = libc::STDIN_FILENO;
    let stdout_fd = libc::STDOUT_FILENO;

    let mut poll_fds = [
        libc::pollfd {
            fd: stdin_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: master_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];

    let mut buffer = [0_u8; 8192];
    let mut stdin_open = true;

    loop {
        for poll_fd in &mut poll_fds {
            poll_fd.revents = 0;
        }

        let ready =
            unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, -1) };

        if ready == -1 {
            let error = io::Error::last_os_error();

            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }

            return Err(error);
        }

        let stdin_events = poll_fds[0].revents;
        let master_events = poll_fds[1].revents;

        if master_events & libc::POLLIN != 0 {
            let size = unsafe { libc::read(master_fd, buffer.as_mut_ptr().cast(), buffer.len()) };

            if size == 0 {
                break;
            }

            if size == -1 {
                let error = io::Error::last_os_error();

                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }

                if error.raw_os_error() == Some(Errno::EIO as i32) {
                    break;
                }

                return Err(error);
            }

            write_all_fd(stdout_fd, &buffer[..size as usize])?;
        }

        if master_events & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            break;
        }

        if stdin_events & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            stdin_open = false;
            poll_fds[0].fd = -1;
            poll_fds[0].events = 0;
        }

        if stdin_open && stdin_events & libc::POLLIN != 0 {
            let size = unsafe { libc::read(stdin_fd, buffer.as_mut_ptr().cast(), buffer.len()) };

            if size == 0 {
                stdin_open = false;
                poll_fds[0].fd = -1;
                poll_fds[0].events = 0;
                continue;
            }

            if size == -1 {
                let error = io::Error::last_os_error();

                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }

                return Err(error);
            }

            write_all_fd(master_fd, &buffer[..size as usize])?;
        }
    }

    Ok(())
}

fn main() {
    let winsize = terminal_size();

    let pty = openpty(Some(&winsize), None).expect("failed to create PTY");

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            drop(pty.slave);

            let _terminal_guard = TerminalGuard::enable_raw();
            let master_fd = pty.master.as_raw_fd();

            forward_io(master_fd).expect("failed to forward terminal I/O");

            waitpid(child, None).expect("failed to wait for child");
        }

        Ok(ForkResult::Child) => {
            drop(pty.master);

            setsid().expect("failed to create child session");

            let slave_fd = pty.slave.as_raw_fd();

            let result = unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY, 0) };

            if result == -1 {
                panic!("failed to set controlling terminal");
            }

            unsafe {
                if libc::dup2(slave_fd, libc::STDIN_FILENO) == -1 {
                    panic!("failed to redirect stdin");
                }

                if libc::dup2(slave_fd, libc::STDOUT_FILENO) == -1 {
                    panic!("failed to redirect stdout");
                }

                if libc::dup2(slave_fd, libc::STDERR_FILENO) == -1 {
                    panic!("failed to redirect stderr");
                }
            }

            drop(pty.slave);

            let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

            let shell = CString::new(shell).expect("invalid shell path");

            match execvp(&shell, &[&shell]) {
                Ok(_) => unreachable!(),
                Err(error) => panic!("failed to execute shell: {error}"),
            }
        }

        Err(error) => {
            eprintln!("fork failed: {error}");
        }
    }
}
