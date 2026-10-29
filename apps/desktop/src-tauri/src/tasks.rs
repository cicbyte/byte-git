//! 长任务取消：全局取消标志 + 运行中子进程注册表。
//!
//! 任务上下文与取消：
//! Rust 侧 v1 简化为：原子取消标志 + 可选的运行中子进程句柄（kill 即取消）。
//! 长命令执行前检查/注册，前端 CancelTask() 置标志并杀进程。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

struct KillableChild {
    pub pid: u32,
    /// 进程还活着时可调用 kill
    pub kill: Box<dyn Fn() + Send + Sync>,
}

static RUNNING: Mutex<Option<KillableChild>> = Mutex::new(None);

/// 注册当前运行中的长任务（幂等：顶替旧任务，不主动杀）。
pub fn register(pid: u32, kill: Box<dyn Fn() + Send + Sync>) {
    let mut g = RUNNING.lock().unwrap();
    *g = Some(KillableChild { pid, kill });
}

/// 任务结束时清除注册。
pub fn release() {
    let mut g = RUNNING.lock().unwrap();
    *g = None;
}

/// 是否已请求取消（长循环内轮询）。
pub fn is_cancelled() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
}

/// 取消当前长任务：置标志 + kill 运行中子进程。返回是否有任务被取消。
pub fn cancel() -> bool {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    let mut g = RUNNING.lock().unwrap();
    match g.take() {
        Some(child) => {
            (child.kill)();
            true
        }
        None => false,
    }
}

/// 新任务开始时复位取消标志。
pub fn reset() {
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}
