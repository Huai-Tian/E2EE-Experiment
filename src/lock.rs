//! 密钥集 mlock——4.0.0 P20-L（裁决：memory_hygiene.mlock_evaluation
//! .reopened-2026-10-31，全文见 README_AGENT）。
//!
//! 原则：**只锁密钥集，不锁全堆**。原否决理由（4KB 页粒度 vs 32B 堆
//! 对象、RLIMIT_MEMLOCK 64KB 默认、分配器无页对齐）对全堆/全缓冲面
//! 继续成立；密钥集是少量页（个位数），接受粒度价格。
//!
//! 失败响亮降级：mlock/VirtualLock 不可用时退回普通 Zeroizing 堆
//!（擦拭照旧，只是可能被换出）——绝不静默假装锁上（loud-failure 纪律）。
//! 库不打印（api_style）：降级事实经两条路响出来——进程级粘滞标志
//! [`degraded`]（CLI 启动探测渲染）+ 每次**降级态武装**时库发
//! `Event::KeyLockDegraded`（体验流说话，CLI 说人话）。
//!
//! 覆盖面（诚实边界，随 P20 landing 入档）：
//! - 本 slice 覆盖：常备门禁钥（stable）与一次性切口（expecting）——
//!   进程寿命级的最长寿命用户秘密。
//! - 不覆盖：Gathering 群钥材料与 snow 会话钥——需要 circle.rs 重构与
//!   snow fork 深化，属后续 owner 裁决（越权即停，不自行扩大手术）。

use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::Zeroizing;

/// 进程级降级标志：任何一次锁定失败都置位（粘滞，只升不降）。
static DEGRADED: AtomicBool = AtomicBool::new(false);

/// 进程是否发生过锁定降级（粘滞）。
pub fn degraded() -> bool {
    DEGRADED.load(Ordering::Relaxed)
}

/// 启动探测：分配一枚最小秘密并立即丢弃，回答『这台机器现在锁得住吗』。
/// CLI 启动时调用并渲染（库不打印）。
pub fn probe() -> bool {
    SecretBytes::new(b"probe").is_locked()
}

/// 锁定内存里的秘密字节。
///
/// - [`SecretBytes::new`] 优先取一页页对齐的私有内存（mmap+mlock /
///   VirtualAlloc+VirtualLock）——swap/pagefile 永不收留这页；
/// - 锁不住（RLIMIT_MEMLOCK、策略限制等）→ 响亮降级为普通 Zeroizing
///   堆：擦拭语义不变，只是换出风险回来了（事件/标志两条路都会响）。
///
/// 一次一枚页（粒度价格，裁决已接受）；Clone = 复制进新的锁定页。
pub struct SecretBytes {
    inner: Inner,
    len: usize,
}

enum Inner {
    /// 页对齐私有页：base..base+region 已 mlock/VirtualLock。
    Locked { base: *mut u8, region: usize },
    /// 降级回退：普通擦除堆。
    Heap(Zeroizing<Vec<u8>>),
}

impl SecretBytes {
    pub fn new(bytes: &[u8]) -> Self {
        let inner = match unsafe { alloc_locked(bytes.len()) } {
            Some((base, region)) => {
                debug_assert!(region >= bytes.len().max(1));
                unsafe { base.copy_from_nonoverlapping(bytes.as_ptr(), bytes.len()) };
                Inner::Locked { base, region }
            }
            None => {
                DEGRADED.store(true, Ordering::Relaxed);
                Inner::Heap(Zeroizing::new(bytes.to_vec()))
            }
        };
        Self { inner, len: bytes.len() }
    }

    /// 秘密字节（只读借用）。
    pub fn as_slice(&self) -> &[u8] {
        match &self.inner {
            // 安全性：base..base+len 在构造时写入、此后只读；借用不逃逸本类型生命周期。
            Inner::Locked { base, .. } => unsafe {
                std::slice::from_raw_parts(*base, self.len)
            },
            Inner::Heap(v) => v,
        }
    }

    /// 是否真锁住了（降级态为 false——调用方可据此响亮告知）。
    pub fn is_locked(&self) -> bool {
        matches!(self.inner, Inner::Locked { .. })
    }
}

impl Clone for SecretBytes {
    fn clone(&self) -> Self {
        Self::new(self.as_slice())
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        match self.inner {
            // 先擦后还：volatile 清零整页（含构造拷贝越界的邻字节），
            // 再解锁、再释放。
            Inner::Locked { base, region } => unsafe {
                for i in 0..region {
                    base.add(i).write_volatile(0);
                }
                free_locked(base, region);
            },
            Inner::Heap(_) => {} // Zeroizing 自擦
        }
    }
}

// 安全性：Locked 页由本类型独占持有——共享访问只经 as_slice()（&[u8]），
// 可变访问只在 Drop（&mut self），无别名逃逸；Heap 变体本就 Send+Sync。
unsafe impl Send for SecretBytes {}
unsafe impl Sync for SecretBytes {}

#[cfg(unix)]
unsafe fn alloc_locked(len: usize) -> Option<(*mut u8, usize)> {
    unsafe {
        let ps = page_size();
        let region = (len.max(1) + ps - 1) / ps * ps;
        let p = libc::mmap(
            std::ptr::null_mut(),
            region,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if p == libc::MAP_FAILED {
            return None;
        }
        if libc::mlock(p, region) != 0 {
            libc::munmap(p, region);
            return None;
        }
        Some((p as *mut u8, region))
    }
}

#[cfg(unix)]
unsafe fn free_locked(base: *mut u8, region: usize) {
    unsafe {
        libc::munlock(base as *mut libc::c_void, region);
        libc::munmap(base as *mut libc::c_void, region);
    }
}

#[cfg(unix)]
fn page_size() -> usize {
    static PAGE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *PAGE.get_or_init(|| unsafe { libc::sysconf(libc::_SC_PAGESIZE).max(1) as usize })
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    pub(super) const MEM_RESERVE: u32 = 0x0000_2000;
    pub(super) const MEM_COMMIT: u32 = 0x0000_1000;
    pub(super) const MEM_RELEASE: u32 = 0x0000_8000;
    pub(super) const PAGE_READWRITE: u32 = 0x04;
    unsafe extern "system" {
        pub(super) fn VirtualAlloc(
            lpAddress: *mut c_void,
            dwSize: usize,
            flAllocationType: u32,
            flProtect: u32,
        ) -> *mut c_void;
        pub(super) fn VirtualLock(lpAddress: *mut c_void, dwSize: usize) -> i32;
        pub(super) fn VirtualUnlock(lpAddress: *mut c_void, dwSize: usize) -> i32;
        pub(super) fn VirtualFree(
            lpAddress: *mut c_void,
            dwSize: usize,
            dwFreeType: u32,
        ) -> i32;
    }
}

#[cfg(windows)]
unsafe fn alloc_locked(len: usize) -> Option<(*mut u8, usize)> {
    unsafe {
        let region = len.max(1);
        let p = win::VirtualAlloc(
            std::ptr::null_mut(),
            region,
            win::MEM_RESERVE | win::MEM_COMMIT,
            win::PAGE_READWRITE,
        );
        if p.is_null() {
            return None;
        }
        if win::VirtualLock(p, region) == 0 {
            win::VirtualFree(p, 0, win::MEM_RELEASE);
            return None;
        }
        Some((p as *mut u8, region))
    }
}

#[cfg(windows)]
unsafe fn free_locked(base: *mut u8, region: usize) {
    unsafe {
        win::VirtualUnlock(base as *mut std::ffi::c_void, region);
        win::VirtualFree(base as *mut std::ffi::c_void, 0, win::MEM_RELEASE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁定往返：字节进、字节出、真锁住、克隆独立。
    #[test]
    fn 锁定往返() {
        let s = SecretBytes::new(b"a door key of 32-ish bytes.......");
        assert!(s.is_locked(), "正常环境应锁定（本测试沙箱 root 亦然）");
        assert_eq!(s.as_slice(), b"a door key of 32-ish bytes.......");
        let c = s.clone();
        assert_eq!(c.as_slice(), s.as_slice());
        drop(s);
        assert_eq!(c.as_slice(), b"a door key of 32-ish bytes.......");
    }

    /// 降级路径：RLIMIT_MEMLOCK 压到 0 → 分配必败 → 响亮回退到擦除堆
    ///（字节语义不变）；恢复预算后新分配重新锁得住（不是粘死）。
    /// root（CAP_IPC_LOCK）不受 rlimit 约束——降级路径对 root 不可达，
    /// 本测试如实跳过。
    #[cfg(unix)]
    #[test]
    fn 降级路径_响亮回退() {
        if unsafe { libc::geteuid() } == 0 {
            return; // root 无预算限制：降级不可达
        }
        unsafe {
            let mut old = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut old), 0);
            let squeezed = libc::rlimit {
                rlim_cur: 0,
                rlim_max: old.rlim_max, // 只压 cur：不动 max（降 max 不可逆）
            };
            assert_eq!(libc::setrlimit(libc::RLIMIT_MEMLOCK, &squeezed), 0);
            let s = SecretBytes::new(b"must degrade");
            assert!(!s.is_locked(), "预算 0 下应降级");
            assert!(degraded(), "粘滞标志应置位");
            assert_eq!(s.as_slice(), b"must degrade");
            drop(s);
            // 恢复预算：新分配应重新锁得住
            assert_eq!(libc::setrlimit(libc::RLIMIT_MEMLOCK, &old), 0);
            let s2 = SecretBytes::new(b"locked again");
            assert!(s2.is_locked(), "预算恢复后应重新锁定");
        }
    }
}
