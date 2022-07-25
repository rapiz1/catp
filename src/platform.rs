use nix::libc::user_regs_struct;

// Implement this trait to add new architectures
pub trait PlatformRegs {
    fn get_args(&self, no: usize) -> usize;
    fn get_sysno(&self) -> usize;
}

pub trait SysWriteArgs {
    fn get_fd(&self) -> i32;

    fn get_len(&self) -> usize;

    fn get_ptr(&self) -> usize;
}

impl<T: PlatformRegs> SysWriteArgs for T {
    fn get_fd(&self) -> i32 {
        self.get_args(0) as i32
    }

    fn get_ptr(&self) -> usize {
        self.get_args(1) as usize
    }

    fn get_len(&self) -> usize {
        self.get_args(2) as usize
    }
}

#[cfg(target_arch = "x86_64")]
impl PlatformRegs for user_regs_struct {
    fn get_args(&self, no: usize) -> usize {
        match no {
            0 => self.rdi as usize,
            1 => self.rsi as usize,
            2 => self.rdx as usize,
            _ => panic!("get args out of range"),
        }
    }

    fn get_sysno(&self) -> usize {
        self.orig_rax as usize
    }
}
