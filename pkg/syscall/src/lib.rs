#![no_std]

use num_enum::FromPrimitive;

pub mod macros;

#[repr(usize)]
#[derive(Clone, Debug, FromPrimitive)]
pub enum Syscall {
    Read = 0,
    Write = 1,

    Open = 14,
    Close = 15,
    ListDir = 16,
    GetPid = 39,
    
    VFork = 40,
    Spawn = 59,
    Exit = 60,
    WaitPid = 61,
    Kill = 62,
    Sem = 66,
    Brk = 67,

    ListApp = 65529,
    Stat = 65530,
    Allocate = 65533,
    Deallocate = 65534,

    #[num_enum(default)]
    Unknown = 65535,
}
