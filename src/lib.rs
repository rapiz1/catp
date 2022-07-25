mod platform;

use std::io::{self, IoSliceMut};

use anyhow::{Context, Result};
use clap::Parser;
use nix::errno::Errno;
use nix::libc::{SYS_write, STDERR_FILENO, STDOUT_FILENO};
use nix::sys::uio::{process_vm_readv, RemoteIoVec};
use nix::sys::wait::WaitStatus;
use nix::sys::{ptrace, wait};
use nix::unistd::Pid;
use platform::{PlatformRegs, SysWriteArgs};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct CatpArgs {
    // TODO: Maybe handle multi-threaded programs. Implement -f --follow-forks
    // Trace child processes as they are created by currently traced processes as
    // a result of the fork(2), vfork(2) and clone(2) system calls.
    /// PID of the process to print
    pub pid: u32,

    /// Print more verbose information to stderr
    #[clap(short, long, action, default_value_t = false)]
    pub verbose: bool,
}

fn vm_read_data(pid: Pid, ptr: usize, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let mut local = [IoSliceMut::new(buf.as_mut_slice())];
    let remote = [RemoteIoVec {
        base: ptr as usize,
        len: len as usize,
    }];
    process_vm_readv(pid, &mut local, &remote)?;

    Ok(buf)
}

fn read_data(pid: Pid, ptr: usize, len: usize) -> Result<Vec<u8>> {
    vm_read_data(pid, ptr, len)
}

pub fn catp<T: io::Write, S: io::Write>(
    args: CatpArgs,
    stdout: &mut T,
    stderr: &mut S,
) -> Result<()> {
    let verbose = args.verbose;
    if verbose {
        eprintln!("{:#?}", args);
    }

    // if args.follow_forks {
    //     unimplemented!();
    // }

    let pid = Pid::from_raw(args.pid as i32);
    let opts = ptrace::Options::PTRACE_O_TRACESYSGOOD;
    ptrace::attach(pid).with_context(|| "attach")?;

    let mut first_ptrace_stop = true;
    let mut in_syscall = false;
    loop {
        match wait::wait() {
            Ok(WaitStatus::PtraceSyscall(pid)) => {
                if in_syscall {
                    in_syscall = false;
                } else {
                    let regs = ptrace::getregs(pid).with_context(|| "getregs")?;
                    let sysno = regs.get_sysno();
                    if sysno == SYS_write as usize {
                        if verbose {
                            eprintln!("{:?}, {:?}", pid, regs);
                        }
                        let fd: i32 = regs.get_fd();
                        if fd == STDOUT_FILENO || fd == STDERR_FILENO {
                            let ptr = regs.get_ptr();
                            let len = regs.get_len();
                            let data = read_data(pid, ptr, len).with_context(|| "read_data")?;
                            if verbose {
                                eprintln!("{}, {:?} {:?} {:?}", fd, ptr, len, data);
                            }
                            if fd == STDOUT_FILENO {
                                stdout.write_all(&data)?;
                            } else {
                                stderr.write_all(&data)?;
                            }
                        }
                    }
                    in_syscall = true;
                }
                ptrace::syscall(pid, None).with_context(|| "syscall")?;
            }
            Ok(WaitStatus::Stopped(pid, sig)) => {
                if first_ptrace_stop
                    && ptrace::setoptions(pid, opts)
                        .with_context(|| "setoptions")
                        .is_ok()
                {
                    first_ptrace_stop = false;
                    // This should be a SIGSTP sent by attach. Suppress it
                    ptrace::syscall(pid, None).with_context(|| "syscall")?
                } else {
                    ptrace::syscall(pid, sig).with_context(|| "syscall")?
                }
            }
            Ok(_) => break,
            Err(Errno::ECHILD) => break,
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    Ok(())
}
