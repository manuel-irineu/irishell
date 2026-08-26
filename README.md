# Irishell

Irishell is an experimental terminal emulator project written in Rust for
Linux.

The project is being built incrementally from low-level Unix terminal
primitives upward. Its current release is an early foundation: Irishell can
start and manage an interactive shell through a Linux PTY, but it is not yet a
complete graphical terminal emulator.

## Current Status

Current milestone: `v0.0.1` - PTY Foundation.

Irishell currently provides:

- PTY creation with `openpty`;
- initial terminal window size propagation;
- fork-based parent/child process creation;
- parent/child PTY descriptor separation;
- `setsid` in the child process;
- PTY slave setup as the child controlling terminal with `TIOCSCTTY`;
- child stdin, stdout, and stderr redirection to the PTY slave with `dup2`;
- execution of the user's `$SHELL`;
- `/bin/sh` fallback when `$SHELL` is not set;
- raw mode for the host terminal;
- automatic host terminal restoration using RAII;
- bidirectional byte forwarding using `poll(2)`;
- `EINTR` handling in the I/O loop;
- Linux PTY master `EIO` handling when the slave side closes;
- child process cleanup with `waitpid`.

At this stage, Irishell runs an interactive shell through a PTY managed by
Irishell. It does not yet implement full ANSI/VT terminal emulation, terminal
screen state, scrollback, text rendering, or a graphical frontend.

## Architecture

```text
Host Terminal
     |
     | stdin / stdout
     v
+-----------+
| Irishell  |
+-----------+
     |
     | PTY master
     v
+-----------+
| Linux PTY |
+-----------+
     |
     | PTY slave
     v
+-----------+
|   Shell   |
+-----------+
```

Irishell owns the PTY master. The child shell uses the PTY slave, and that
slave is configured as the child session's controlling terminal. Irishell
forwards input from the host terminal to the PTY master and writes PTY output
back to the host terminal.

## Requirements

- Linux
- Rust toolchain
- Cargo
- C toolchain/linker required by the Rust build environment

The project uses the Rust 2024 edition and currently depends on:

- `nix`
- `libc`

No minimum supported Rust version is defined yet.

## Building

Clone the repository and build with Cargo:

```sh
git clone git@gitlab.com:manuel_irineu/irishell.git
cd irishell
cargo build
```

Run Irishell with:

```sh
cargo run
```

`cargo run` currently starts the user's configured shell through the Irishell
PTY layer.

## Development

Useful validation commands:

```sh
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

## Roadmap

The roadmap is intentionally non-binding and may evolve as the implementation
develops. Potential future areas include:

- dynamic terminal resize and `SIGWINCH` propagation;
- ANSI/VT escape sequence parsing;
- terminal screen state;
- scrollback;
- keyboard and input handling;
- text rendering;
- font handling;
- graphical frontend;
- Wayland integration;
- configuration;
- shell integration.

## Project Goals

Irishell is both a terminal emulator project and a systems-programming learning
project. The goal is to understand and implement terminal behavior
incrementally instead of treating the terminal as a black box.

Topics explored by the project include:

- Unix PTYs;
- process management;
- sessions and controlling terminals;
- file descriptors;
- `termios`;
- signals;
- event-driven I/O;
- ANSI/VT protocols;
- terminal state;
- rendering.

## License
Writing license

A license has not yet been selected.
