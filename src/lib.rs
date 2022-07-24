use std::io::{IoSliceMut, Write};

use anyhow::{Context, Result};
use clap::Parser;
use nix::errno::Errno;
use nix::libc::SYS_write;
use nix::sys::uio::{process_vm_readv, RemoteIoVec};
use nix::sys::wait::WaitStatus;
use nix::sys::{ptrace, wait};
use nix::unistd::Pid;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct CatpArgs {
    // TODO: Maybe handle multi-threaded programs
    /// PID of the process to print
    pid: u32,

    // TODO: Support multiple fds
    /// File descriptor to print
    #[clap(short, long, default_value_t = 1)]
    fd: u32,

    // TODO:
    // Print child processes as they are created by currently traced processes as
    // a result of the fork(2), vfork(2) and clone(2) system calls.
    // #[clap(short, long, default_value_t = false)]
    // follow_forks: bool,
    /// Print more verbose information to stderr
    #[clap(short, long, action, default_value_t = false)]
    verbose: bool,
}

// fn ptrace_read_data(pid: Pid, ptr: u64, len: u64) -> Result<Vec<u8>> {
//     let word_size = size_of::<c_long>();
//     let mut v: Vec<u8> = vec![];
//     let mut pos = ptr;
//     let end = ptr + len;
//     while pos < end {
//         let word = ptrace::read(pid, pos as AddressType)?;
//         let len = word_size.min((end - pos) as usize);
//         v.extend_from_slice(&word.to_le_bytes()[..len]);
//         pos += word_size as u64;
//     }
//     Ok(v)
// }

fn vm_read_data(pid: Pid, ptr: u64, len: u64) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len as usize];
    let mut local = [IoSliceMut::new(buf.as_mut_slice())];
    let remote = [RemoteIoVec {
        base: ptr as usize,
        len: len as usize,
    }];
    process_vm_readv(pid, &mut local, &remote)?;

    Ok(buf)
}

fn read_data(pid: Pid, ptr: u64, len: u64) -> Result<Vec<u8>> {
    vm_read_data(pid, ptr, len)
}

pub fn catp(args: CatpArgs) -> Result<()> {
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
                    let sysno = regs.orig_rax;
                    if sysno == (SYS_write as u64) {
                        if verbose {
                            eprintln!("{:?}, {:?}", pid, regs);
                        }
                        let fd = regs.rdi as u32;
                        if fd == args.fd {
                            let buf = regs.rsi;
                            let len = regs.rdx;
                            let data = read_data(pid, buf, len).with_context(|| "read_data")?;
                            if verbose {
                                eprintln!("{:?} {:?} {:?}", buf, len, data);
                            }
                            std::io::stdout().write_all(&data)?;
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
