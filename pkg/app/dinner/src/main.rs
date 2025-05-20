#![no_std]
#![no_main]

extern crate lib;
use lib::*;
use core::hint::spin_loop;

// DEMO_MODE 取值：0 - 正常，1 - 死锁示例，2 - 饥饿示例

// 使用 semaphore_array! 宏声明 5 根筷子（每根初始值为 1）
static CHOPSTICKS: [Semaphore; 5] = semaphore_array![0, 1, 2, 3, 4];
// 服务生信号量：正常模式下用于允许最多 4 个哲学家同时拿筷子（避免死锁）
// 注意：在死锁模式下我们不使用 WAITER
static WAITER: Semaphore = Semaphore::new(4);

const PHILOSOPHER_COUNT: usize = 5;
const EAT_TIMES: usize = 3;

//
// 简单的伪随机数生成器，使用进程号作为种子
//
fn simple_rand() -> u64 {
    static mut SEED: u64 = 0;
    unsafe {
        if SEED == 0 {
            SEED = sys_get_pid() as u64;
        }
        SEED = SEED.wrapping_mul(6364136223846793005)
                   .wrapping_add(1);
        SEED
    }
}

// 返回 [min, max) 范围内的随机数
fn rand_range(min: u64, max: u64) -> u64 {
    let r = simple_rand();
    min + (r % (max - min))
}

// 模拟延迟，time 为循环次数
#[inline(never)]
#[unsafe(no_mangle)]
fn delay(time: u64) {
    for _ in 0..time {
        spin_loop();
    }
}

//
// 哲学家行为函数
//
fn philosopher(id: usize, demo: u64) -> ! {
    let pid = sys_get_pid();
    println!("Philosopher {} (PID {}) started.", id, pid);
    for turn in 0..EAT_TIMES {
        // --- 思考阶段 ---
        // 在死锁模式下使用固定、较短的延迟，让所有哲学家近乎同时饥饿
        if demo == 1 {
            delay(0x01000);
        } else {
            if demo == 2 {
                if id == 0 || id == 3 {
                    delay(0x100);
                } else {
                    delay(0x10000);
                }
            } else {
                delay(rand_range(0x30000, 0x60000));
            }
        }
        // --- 就餐阶段 ---
        // 在死锁模式下不使用 WAITER，且所有哲学家均按相同顺序拿筷子
        if demo == 1 {
            // 所有哲学家都先拿左筷子，再拿右筷子
            CHOPSTICKS[id].wait();
            println!("Philosopher {} (PID {}) picked up left chopstick {}.", id, pid, id);
            delay(0x10000);  // 模拟延迟
            CHOPSTICKS[(id + 1) % PHILOSOPHER_COUNT].wait();
            println!("Philosopher {} (PID {}) picked up right chopstick {}.", id, pid, (id + 1) % PHILOSOPHER_COUNT);
        } else {
            // 正常及饥饿模式采用奇偶不同顺序
            if id % 2 == 0 {
                CHOPSTICKS[id].wait();
                println!("Philosopher {} (PID {}) picked up left chopstick {}.", id, pid, id);
                CHOPSTICKS[(id + 1) % PHILOSOPHER_COUNT].wait();
                println!("Philosopher {} (PID {}) picked up right chopstick {}.", id, pid, (id + 1) % PHILOSOPHER_COUNT);
            } else {
                CHOPSTICKS[(id + 1) % PHILOSOPHER_COUNT].wait();
                println!("Philosopher {} (PID {}) picked up right chopstick {}.", id, pid, (id + 1) % PHILOSOPHER_COUNT);
                CHOPSTICKS[id].wait();
                println!("Philosopher {} (PID {}) picked up left chopstick {}.", id, pid, id);
            }
        }

        println!("Philosopher {} (PID {}) is eating (turn {}).", id, pid, turn + 1);
        delay(rand_range(0x10000, 0x20000));  // 吃饭延迟

        // 放下筷子
        CHOPSTICKS[id].signal();
        println!("Philosopher {} (PID {}) put down chopstick {}.", id, pid, id);
        CHOPSTICKS[(id + 1) % PHILOSOPHER_COUNT].signal();
        println!("Philosopher {} (PID {}) put down chopstick {}.", id, pid, (id + 1) % PHILOSOPHER_COUNT);

        // 在死锁模式下不释放 WAITER
        if demo != 1 {
            WAITER.signal();
        }
    }
    println!("Philosopher {} (PID {}) is satisfied and leaves.", id, pid);
    sys_exit(0);
}

//
// 主函数：创建 5 个哲学家进程，并初始化信号量
//
fn main() -> isize {
    let input = lib::stdin().read_line();

    // prase input as u64
    let demo = input.parse::<u64>().unwrap();
    // 初始化 5 根筷子的信号量，初始值 1 表示空闲
    for i in 0..PHILOSOPHER_COUNT {
        CHOPSTICKS[i].init(1);
    }
    // 在正常和饥饿模式中初始化服务生信号量为 4（允许最多 4 个哲学家同时拿筷子）
    if demo != 1 {
        WAITER.init(PHILOSOPHER_COUNT - 1);
    }
    let mut pids = [0u16; PHILOSOPHER_COUNT];
    for id in 0..PHILOSOPHER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            philosopher(id, demo);
        } else {
            pids[id] = pid;
        }
    }
    let parent_pid = sys_get_pid();
    println!("#{}: Created philosophers: {:?}", parent_pid, &pids);
    sys_stat();
    for &child in pids.iter() {
        println!("#{} waiting for philosopher #{}...", parent_pid, child);
        sys_wait_pid(child);
    }
    println!("#{}: All philosophers have finished dinner.", parent_pid);
    0
}

entry!(main);