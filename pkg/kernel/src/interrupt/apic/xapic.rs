use super::LocalApic;
use bit_field::BitField;
use bitflags::bitflags;
use core::fmt::{Debug, Error, Formatter};
use core::ptr::{read_volatile, write_volatile};
use x86::cpuid::CpuId;
use crate::interrupt::consts::{Interrupts, Irq};

/// 默认 xAPIC 物理地址
pub const LAPIC_ADDR: u64 = 0xFEE00000;

/// 用枚举表示 LAPIC 寄存器偏移
#[repr(u32)]
pub enum LapicRegister {
    SVR      = 0xF0,  // Spurious Interrupt Vector Register
    TDCR     = 0x3E0, // Timer Divide Configuration Register
    TICR     = 0x380, // Timer Initial Count Register
    LvtTimer = 0x320, // LVT Timer Register
    LvtLInt0 = 0x350, // LVT LINT0 Register
    LvtLInt1 = 0x360, // LVT LINT1 Register
    LvtPCINT = 0x340, // LVT Performance Counter Interrupt Register
    LvtError = 0x370, // LVT Error Register
    ESR      = 0x280, // Error Status Register
    ICRLow   = 0x300, // Interrupt Command Register Low
    ICRHigh  = 0x310, // Interrupt Command Register High
    EOI      = 0x00B0, // End-of-Interrupt Register
    TPR      = 0x80,   // Task Priority Register
}

impl From<LapicRegister> for u32 {
    fn from(reg: LapicRegister) -> Self {
        reg as u32
    }
}

// 使用 bitflags 定义 SVR 寄存器的标志位
bitflags! {
    pub struct SvrFlags: u32 {
        const ENABLE      = 1 << 8;
        const VECTOR_MASK = 0xFF;
    }
}

// 使用 bitflags 定义 LVT Timer 寄存器的标志位
bitflags! {
    pub struct LvtTimerFlags: u32 {
        const MASKED      = 1 << 16;
        const PERIODIC    = 1 << 17;
        const VECTOR_MASK = 0xFF;
    }
}

bitflags! {
    pub struct LvtErrorFlags: u32 {
        const MASKED      = 1 << 16;
        const VECTOR_MASK = 0xFF;
    }
}

pub struct XApic {
    addr: u64,
}

impl XApic {
    /// 构造新 XApic 实例，传入的地址为物理地址。物理地址经过映射后使用。
    pub unsafe fn new(addr: u64) -> Self {
        XApic { addr: addr }
    }

    /// 使用枚举读取寄存器
    unsafe fn read(&self, reg: LapicRegister) -> u32 {
        unsafe{
            read_volatile((self.addr + u32::from(reg) as u64) as *const u32)
        }
    }

    /// 使用枚举写入寄存器
    unsafe fn write(&mut self, reg: LapicRegister, value: u32) {
        unsafe {
            write_volatile((self.addr + u32::from(reg) as u64) as *mut u32, value);
            // 同步：通过读取 ESR 寄存器来确保写入生效
            self.read(LapicRegister::ESR);
        }
    }
}

impl LocalApic for XApic {
    fn support() -> bool {
        CpuId::new().get_feature_info().map(|f| f.has_apic()).unwrap_or(false)
    }

    fn cpu_init(&mut self) {
        unsafe {
            // 设置 Spurious Interrupt Vector Register (SVR)
            let spiv_val = self.read(LapicRegister::SVR);
            let mut svr = SvrFlags::from_bits_truncate(spiv_val);
            svr.insert(SvrFlags::ENABLE);
            svr.remove(SvrFlags::VECTOR_MASK);
            svr.insert(SvrFlags::from_bits_truncate(Interrupts::IrqBase as u32 + Irq::Spurious as u32));
            self.write(LapicRegister::SVR, svr.bits());

            // 配置定时器除法器和初始计数器
            self.write(LapicRegister::TDCR, 0b1000); // 除法器设置为 1
            self.write(LapicRegister::TICR, 0x20000);  // 初始计数器

            // 设置 LVT Timer 寄存器
            let lvt_timer_val = self.read(LapicRegister::LvtTimer);
            let mut lvt_timer = LvtTimerFlags::from_bits_truncate(lvt_timer_val);
            lvt_timer.remove(LvtTimerFlags::VECTOR_MASK);
            lvt_timer.insert(LvtTimerFlags::from_bits_truncate(Interrupts::IrqBase as u32 + Irq::Timer as u32));
            lvt_timer.remove(LvtTimerFlags::MASKED);
            lvt_timer.insert(LvtTimerFlags::PERIODIC);
            self.write(LapicRegister::LvtTimer, lvt_timer.bits());

            // 屏蔽 LINT0, LINT1 以及 PCINT
            self.write(LapicRegister::LvtLInt0, 1 << 16);
            self.write(LapicRegister::LvtLInt1, 1 << 16);
            self.write(LapicRegister::LvtPCINT, 1 << 16);

            // 设置 LVT Error 寄存器
            let lvt_error_val = self.read(LapicRegister::LvtError);
            let mut lvt_error = LvtErrorFlags::from_bits_truncate(lvt_error_val);
            lvt_error.remove(LvtErrorFlags::VECTOR_MASK);
            lvt_error.insert(LvtErrorFlags::from_bits_truncate(Interrupts::IrqBase as u32 + Irq::Error as u32));
            lvt_error.remove(LvtErrorFlags::MASKED);
            self.write(LapicRegister::LvtError, lvt_error.bits());

            // 清除错误状态寄存器两次以清除旧错误
            self.write(LapicRegister::ESR, 0);
            self.write(LapicRegister::ESR, 0);

            // 发出 End-Of-Interrupt
            self.eoi();

            // 发送 INIT Level De-assert 命令（示例代码）
            const BCAST: u32 = 1 << 19;
            const INIT: u32 = 5 << 8;
            const TMLV: u32 = 1 << 15; // TM = 1, LV = 0
            self.set_icr((BCAST | INIT | TMLV) as u64);

            // 设置 Task Priority Register (TPR) 为 0 以允许所有中断
            self.write(LapicRegister::TPR, 0);
        }
    }

    fn id(&self) -> u32 {
        unsafe { self.read(LapicRegister::ICRHigh) >> 24 }
    }

    fn version(&self) -> u32 {
        unsafe { self.read(LapicRegister::ICRLow) } // 示例：请根据实际需要更改寄存器
    }

    fn icr(&self) -> u64 {
        unsafe { (self.read(LapicRegister::ICRHigh) as u64) << 32 | self.read(LapicRegister::ICRLow) as u64 }
    }

    fn set_icr(&mut self, value: u64) {
        unsafe {
            while self.read(LapicRegister::ICRLow).get_bit(12) {}
            self.write(LapicRegister::ICRHigh, (value >> 32) as u32);
            self.write(LapicRegister::ICRLow, value as u32);
            while self.read(LapicRegister::ICRLow).get_bit(12) {}
        }
    }

    fn eoi(&mut self) {
        unsafe {
            self.write(LapicRegister::EOI, 0);
        }
    }
}

impl Debug for XApic {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_struct("XApic")
            .field("id", &self.id())
            .field("version", &self.version())
            .field("icr", &self.icr())
            .finish()
    }
}