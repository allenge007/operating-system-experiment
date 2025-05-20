#![no_std]
#![no_main]

extern crate lib;
use lib::*;
use core::hint::spin_loop;

// 定义 3 个信号量，用于分别唤醒输出 ">", "<" 和 "_" 的子进程
static SEM_GT: Semaphore = Semaphore::new(0x3001);
static SEM_LT: Semaphore = Semaphore::new(0x3002);
static SEM_US: Semaphore = Semaphore::new(0x3003);
// 用于保护全局状态的互斥信号量
static MUTEX: Semaphore = Semaphore::new(0x3004);

// 全局状态：STEP 表示当前 4 字符块内的位置（0~3），PATTERN 表示当前块用哪种模式
// 模式 0：顺序 [SEM_LT, SEM_GT, SEM_LT, SEM_US] 对应输出 "<><_"
// 模式 1：顺序 [SEM_GT, SEM_LT, SEM_GT, SEM_US] 对应输出 "><>_"
static mut STEP: usize = 0;
static mut PATTERN: u8 = 0; // 0 或 1

// 增加全局打印计数器，用于判断何时结束
static mut PRINT_COUNT: usize = 0;
// 总共要打印的字符数（例如 100 个 4字符块，共 400 个字符）
const TOTAL_PRINTS: usize = 400;

// 根据当前 PATTERN 和 STEP 返回下一个应唤醒的信号量
fn next_sem() -> &'static Semaphore {
    unsafe {
        if PATTERN == 0 {
            match STEP {
                0 => &SEM_LT, // 第 1 个输出："<"
                1 => &SEM_GT, // 第 2 个输出：">"
                2 => &SEM_LT, // 第 3 个输出："<"
                3 => &SEM_US, // 第 4 个输出："_"
                _ => unreachable!(),
            }
        } else {
            match STEP {
                0 => &SEM_GT, // 第 1 个输出：">"
                1 => &SEM_LT, // 第 2 个输出："<"
                2 => &SEM_GT, // 第 3 个输出：">"
                3 => &SEM_US, // 第 4 个输出："_"
                _ => unreachable!(),
            }
        }
    }
}

// 辅助函数：当打印完毕时，广播所有信号，以便所有等待的子进程能退出
fn broadcast_exit() {
    // 同时唤醒所有三个信号量
    SEM_GT.signal();
    SEM_LT.signal();
    SEM_US.signal();
}

// 输出 ">" 的子进程
fn fish_gt() -> ! {
    loop {
        SEM_GT.wait();
        MUTEX.wait();
        unsafe {
            if PRINT_COUNT >= TOTAL_PRINTS {
                broadcast_exit();
                MUTEX.signal();
                sys_exit(0);
            }
        }
        print!(">");
        unsafe {
            PRINT_COUNT += 1;
            STEP += 1;
            if STEP < 4 {
                next_sem().signal();
            } else {
                STEP = 0;
                PATTERN = (PATTERN + 1) % 2;
                next_sem().signal();
            }
        }
        MUTEX.signal();
    }
}

// 输出 "<" 的子进程
fn fish_lt() -> ! {
    loop {
        SEM_LT.wait();
        MUTEX.wait();
        unsafe {
            if PRINT_COUNT >= TOTAL_PRINTS {
                broadcast_exit();
                MUTEX.signal();
                sys_exit(0);
            }
        }
        print!("<");
        unsafe {
            PRINT_COUNT += 1;
            STEP += 1;
            if STEP < 4 {
                next_sem().signal();
            } else {
                STEP = 0;
                PATTERN = (PATTERN + 1) % 2;
                next_sem().signal();
            }
        }
        MUTEX.signal();
    }
}

// 输出 "_" 的子进程
fn fish_us() -> ! {
    loop {
        SEM_US.wait();
        MUTEX.wait();
        unsafe {
            if PRINT_COUNT >= TOTAL_PRINTS {
                broadcast_exit();
                MUTEX.signal();
                sys_exit(0);
            }
        }
        print!("_");
        unsafe {
            PRINT_COUNT += 1;
            STEP += 1;
            if STEP < 4 {
                next_sem().signal();
            } else {
                STEP = 0;
                PATTERN = (PATTERN + 1) % 2;
                next_sem().signal();
            }
        }
        MUTEX.signal();
    }
}

// 简单延迟函数，模拟忙等待
#[inline(never)]
#[unsafe(no_mangle)]
fn delay(time: u64) {
    for _ in 0..time {
        spin_loop();
    }
}

fn main() -> isize {
    // 初始化信号量：初始值为 0 表示各子进程先阻塞
    SEM_GT.init(0);
    SEM_LT.init(0);
    SEM_US.init(0);
    // 互斥信号量初始值为 1
    MUTEX.init(1);
    unsafe {
        STEP = 0;
        PATTERN = 0;
        PRINT_COUNT = 0;
    }
    // 根据初始模式 0 (<><_),首个输出应为 "<"，故唤醒 SEM_LT
    SEM_LT.signal();
    
    // 创建 3 个子进程分别用于输出 ">", "<" 和 "_"
    let pid1 = sys_fork();
    if pid1 == 0 {
        fish_gt();
    }
    let pid2 = sys_fork();
    if pid2 == 0 {
        fish_lt();
    }
    let pid3 = sys_fork();
    if pid3 == 0 {
        fish_us();
    }
    
    // 父进程等待所有子进程结束
    sys_wait_pid(pid1);
    sys_wait_pid(pid2);
    sys_wait_pid(pid3);
    println!("\nAll child processes have exited.");
    sys_exit(0);
}

entry!(main);