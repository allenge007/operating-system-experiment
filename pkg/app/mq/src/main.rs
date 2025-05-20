#![no_std]
#![no_main]

extern crate lib;
use lib::*;
use core::hint::spin_loop;

// 杯子架容量：最多同时存放的咖啡杯数
const CUP_CAPACITY: usize = 4;
// 顾客数量（仅顾客会消费咖啡）
const TOTAL_CUSTOMERS: usize = 10;
// 每个顾客消费杯数
const CONSUME_PER_CUSTOMER: usize = 5;
// 咖啡师需要生产的总杯数 = 顾客数 × 每个顾客的消费数
const TOTAL_PRODUCTION: usize = TOTAL_CUSTOMERS * CONSUME_PER_CUSTOMER;

// 全局共享资源：当前咖啡杯数量（生产后未被消费的杯数）
static mut COFFEE_CUPS: usize = 0;

// 定义全局信号量：
// FULL_CAPACITY：杯子架未满，咖啡师可生产（初值：CUP_CAPACITY）
// NON_EMPTY：杯子架不空，顾客可消费（初值：0）
// MUTEX：互斥信号量，用于保护共享资源 COFFEE_CUPS（初值：1）
static FULL_CAPACITY: Semaphore = Semaphore::new(0x1000);
static NON_EMPTY: Semaphore = Semaphore::new(0x2000);
static MUTEX: Semaphore = Semaphore::new(0x6666);

fn main() -> isize {
    // 初始化信号量
    NON_EMPTY.init(0);           // 一开始没有咖啡杯可供顾客消费
    FULL_CAPACITY.init(CUP_CAPACITY); // 杯子架空，最多容纳 CUP_CAPACITY 杯咖啡
    MUTEX.init(1);               // 互斥信号量保护 COFFEE_CUPS

    // 创建子进程：1 个咖啡师 + TOTAL_CUSTOMERS 个顾客
    let total_children = 1 + TOTAL_CUSTOMERS;
    let mut pids = [0u16; 1 + TOTAL_CUSTOMERS];
    for (i, pid_ref) in pids.iter_mut().enumerate() {
        let pid = sys_fork();
        if pid == 0 {
            if i == 0 {
                // 第一个子进程作为咖啡师
                coffee_maker();
            } else {
                // 其余子进程作为顾客
                customer(i);
            }
        } else {
            *pid_ref = pid;
        }
    }

    let parent_pid = sys_get_pid();
    println!("#{}: Created children: {:?}", parent_pid, &pids);
    sys_stat();

    // 父进程等待所有子进程退出
    for &child in pids.iter() {
        println!("#{} waiting for child #{}...", parent_pid, child);
        sys_wait_pid(child);
    }

    // 释放信号量资源（模拟清理）
    MUTEX.free();
    NON_EMPTY.free();
    FULL_CAPACITY.free();

    println!("#{}: All children finished. Final COFFEE_CUPS = {}", parent_pid, unsafe { COFFEE_CUPS });
    0
}

fn coffee_maker() -> ! {
    let pid = sys_get_pid();
    println!("Coffee Maker (PID {}) started. Will produce {} cups.", pid, TOTAL_PRODUCTION);
    // 咖啡师持续生产咖啡，直到生产足够总数
    for i in 0..TOTAL_PRODUCTION {
        delay(0x1000);
        // 等待：若杯子架未满才能生产
        FULL_CAPACITY.wait();
        // 互斥访问共享变量
        MUTEX.wait();
        unsafe { COFFEE_CUPS += 1; }
        println!(
            "Coffee Maker (PID {}): Produced cup {:<3}/{}. Total cups = {}",
            pid,
            i + 1,
            TOTAL_PRODUCTION,
            unsafe { COFFEE_CUPS }
        );
        MUTEX.signal();
        // 通知顾客：至少有一杯咖啡可供消费
        NON_EMPTY.signal();
    }
    println!("Coffee Maker (PID {}) finished producing.", pid);
    sys_exit(0);
}

fn customer(id: usize) -> ! {
    let pid = sys_get_pid();
    println!("Customer #{} (PID {}) arrived.", id, pid);
    // 每个顾客消费 CONSUME_PER_CUSTOMER 杯咖啡
    for i in 0..CONSUME_PER_CUSTOMER {
        delay(0x10000);
        // 等待：确保至少有一杯咖啡
        NON_EMPTY.wait();
        MUTEX.wait();
        unsafe { COFFEE_CUPS -= 1; }
        println!(
            "Customer #{} (PID {}): Drank cup {:<3}/{}. Remaining cups = {}",
            id,
            pid,
            i + 1,
            CONSUME_PER_CUSTOMER,
            unsafe { COFFEE_CUPS }
        );
        MUTEX.signal();
        // 通知咖啡师：有空位可以生产
        FULL_CAPACITY.signal();
    }
    println!("Customer #{} (PID {}) finished drinking.", id, pid);
    sys_exit(0);
}

#[inline(never)]
#[unsafe(no_mangle)]
fn delay(time: usize) {
    // 模拟延迟，便于观察生产与消费的交替效果
    for _ in 0..time {
        spin_loop();
    }
}

entry!(main);