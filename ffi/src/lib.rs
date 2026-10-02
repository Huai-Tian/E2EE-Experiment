//! E2EE-Experiment 的 C ABI 门面（cdylib：libe2ee.so / e2ee.dll）。
//!
//! 纪律（ABI 冻结规则，README_AGENT 有机器版）：
//! - 头文件 include/e2ee.h 与本文件**必须**同步修改；
//! - **只加不改**：新增函数可以，改动既有签名或结构布局不行；
//! - 返回值：0 = 成功，-1 = 失败（人话在 e2ee_last_error 里）；
//! - 句柄生灭请在同一线程序行管理；别的线程还在 poll 时不要 destroy。

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::timeout;

use e2ee::person::{Event, Events, LeaveReason, Person};

/* ── 事件标签：与 e2ee.h 的 enum 一一对应（冻结） ── */
const EVENT_UNKNOWN: c_int = 0;
const EVENT_KNOCKED: c_int = 1;
const EVENT_MET: c_int = 2;
const EVENT_MEET_FAILED: c_int = 3;
const EVENT_HEARD: c_int = 4;
const EVENT_LEFT: c_int = 5;
const EVENT_FOCUS_RETURNED: c_int = 6;
const EVENT_SECRET_CHAINED: c_int = 7;
const EVENT_CIRCLE_FORMED: c_int = 8;
const EVENT_CIRCLE_MUMBLE: c_int = 9;

/* ── 事件结构：与 e2ee.h 的 struct 一一对应（布局冻结） ── */
/// 超出容量的字符串会被截断（name 63 字节、text 511 字节，UTF-8 可能截半——
/// 需要完整文本的接入方请走 Rust API）。
#[repr(C)]
pub struct E2eeEvent {
    pub tag: c_int,
    pub id: u64,
    pub focused: c_int,
    pub members: c_int,
    pub name: [c_char; 64],
    pub text: [c_char; 512],
}

/// 一个人 + 他的私有运转时 + 事件流。
/// 每个句柄自带一个 tokio 运行时：destroy 时整个世界随之熄灭，
/// 守门的、听声的后台任务一并撤下，端口即刻释放。
pub struct FfiPerson {
    person: Person,
    events: Mutex<Events>,
    rt: tokio::runtime::Runtime,
    local_addr: CString,
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_err(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.into());
}

/// 拷贝字符串进 C 缓冲：截断到容量，永远补 \0。
fn copy_str(dst: &mut [c_char], s: &str) {
    let n = s.as_bytes().len().min(dst.len() - 1);
    for (i, b) in s.as_bytes()[..n].iter().enumerate() {
        dst[i] = *b as c_char;
    }
    dst[n] = 0;
}

/// 读入 C 字符串：非空指针、合法 UTF-8（错误信息为中文，与 CLI 一致）。
unsafe fn cstr_in(p: *const c_char, what: &str) -> Result<String, String> {
    if p.is_null() {
        return Err(format!("{what} 是空指针"));
    }
    let c = unsafe { CStr::from_ptr(p) };
    c.to_str()
        .map(|s| s.to_string())
        .map_err(|_| format!("{what} 不是 UTF-8"))
}

/// 库版本（ABI 排查用）。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_version() -> *const c_char {
    concat!("e2ee ", env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// 出生：起名并守在一个地址上听。端口填 0 则由系统分配
/// （真地址用 e2ee_local_addr 取）。失败返回 NULL，人话在 last_error。
///
/// # Safety
/// name / listen_addr 必须是合法的 NUL 结尾 UTF-8 C 字符串。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_person_create(
    name: *const c_char,
    listen_addr: *const c_char,
) -> *mut FfiPerson {
    let work = || -> Result<FfiPerson, String> {
        let name = unsafe { cstr_in(name, "name") }?;
        let addr = unsafe { cstr_in(listen_addr, "listen_addr") }?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("起不动运转时：{e}"))?;
        let (person, events) = rt
            .block_on(Person::born(&name, &addr))
            .map_err(|e| e.to_string())?;
        let la = rt.block_on(person.listen_addr());
        Ok(FfiPerson {
            person,
            events: Mutex::new(events),
            rt,
            local_addr: CString::new(la).map_err(|_| "地址里混了空字节".to_string())?,
        })
    };
    match work() {
        Ok(p) => Box::into_raw(Box::new(p)),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

/// 离场并销毁：向所有人道别，然后整个世界熄灭。
/// 别的线程还在 poll 这个句柄时不要调用。
///
/// # Safety
/// p 必须来自 e2ee_person_create，且不得二次销毁。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_person_destroy(p: *mut FfiPerson) {
    if p.is_null() {
        return;
    }
    let boxed = unsafe { Box::from_raw(p) };
    let FfiPerson {
        person,
        events,
        rt,
        local_addr,
    } = *boxed;
    let _ = local_addr;
    // 体面道别，随后 drop 运转时：后台任务全部撤下
    let _ = rt.block_on(person.leave());
    drop(events);
    drop(person);
    drop(rt);
}

/// 取最近一次错误的人话（本线程）。返回 0 成功，-1 参数不对。
///
/// # Safety
/// buf 指向 buflen 字节的可写内存。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_last_error(buf: *mut c_char, buflen: c_int) -> c_int {
    if buf.is_null() || buflen <= 0 {
        return -1;
    }
    let dst = unsafe { std::slice::from_raw_parts_mut(buf, buflen as usize) };
    LAST_ERROR.with(|e| copy_str(dst, &e.borrow()));
    0
}

/// 这个人实际守着的地址（端口填 0 时给出真端口）。
///
/// # Safety
/// p 合法句柄；buf 指向 buflen 字节可写内存。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_local_addr(p: *const FfiPerson, buf: *mut c_char, buflen: c_int) -> c_int {
    if p.is_null() || buf.is_null() || buflen <= 0 {
        set_err("参数为空");
        return -1;
    }
    let fp = unsafe { &*p };
    let dst = unsafe { std::slice::from_raw_parts_mut(buf, buflen as usize) };
    copy_str(dst, fp.local_addr.to_str().unwrap_or(""));
    0
}

/// 备好切口：下一位来客对得上这句暗号才谈得成（一位一焚）。
///
/// # Safety
/// p 合法句柄；secret 是合法 C 字符串。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_await_secret(p: *mut FfiPerson, secret: *const c_char) -> c_int {
    if p.is_null() {
        set_err("句柄为空");
        return -1;
    }
    let sec = match unsafe { cstr_in(secret, "secret") } {
        Ok(s) => s,
        Err(e) => {
            set_err(e);
            return -1;
        }
    };
    let fp = unsafe { &*p };
    fp.rt.block_on(fp.person.expect_secret(sec.as_bytes()));
    0
}

/// 找人：拨通、握手、成为当前对话。
/// secret 为 NULL 时走普通握手（若有暗号链则自动用上）；
/// 非 NULL 则先对暗号，对不上响亮失败。out_convo_id 可为 NULL。
///
/// # Safety
/// p 合法句柄；addr 必填、secret 可空，均为合法 C 字符串。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_dial(
    p: *mut FfiPerson,
    addr: *const c_char,
    secret: *const c_char,
    out_convo_id: *mut u64,
) -> c_int {
    if p.is_null() {
        set_err("句柄为空");
        return -1;
    }
    let a = match unsafe { cstr_in(addr, "addr") } {
        Ok(s) => s,
        Err(e) => {
            set_err(e);
            return -1;
        }
    };
    let fp = unsafe { &*p };
    let r = if secret.is_null() {
        fp.rt.block_on(fp.person.dial(&a))
    } else {
        match unsafe { cstr_in(secret, "secret") } {
            Ok(s) => fp.rt.block_on(fp.person.dial_secret(&a, Some(s.as_bytes()))),
            Err(e) => {
                set_err(e);
                return -1;
            }
        }
    };
    match r {
        Ok(id) => {
            if !out_convo_id.is_null() {
                unsafe { *out_convo_id = id };
            }
            0
        }
        Err(e) => {
            set_err(e.to_string());
            -1
        }
    }
}

/// 说一句话给此刻注意力所在的人。
///
/// # Safety
/// p 合法句柄；text 是合法 C 字符串。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_speak(p: *mut FfiPerson, text: *const c_char) -> c_int {
    if p.is_null() {
        set_err("句柄为空");
        return -1;
    }
    let t = match unsafe { cstr_in(text, "text") } {
        Ok(s) => s,
        Err(e) => {
            set_err(e);
            return -1;
        }
    };
    let fp = unsafe { &*p };
    match fp.rt.block_on(fp.person.speak(&t)) {
        Ok(()) => 0,
        Err(e) => {
            set_err(e.to_string());
            -1
        }
    }
}

/// 和当前对话的人道别（道别前自动预约下一把暗号）。
///
/// # Safety
/// p 合法句柄。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_bye(p: *mut FfiPerson) -> c_int {
    if p.is_null() {
        set_err("句柄为空");
        return -1;
    }
    let fp = unsafe { &*p };
    match fp.rt.block_on(fp.person.bye()) {
        Ok(_) => 0,
        Err(e) => {
            set_err(e.to_string());
            -1
        }
    }
}

/// 和所有人道别（每场都尝试预约下一把）。用于 destroy 前的体面收场。
///
/// # Safety
/// p 合法句柄。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_leave(p: *mut FfiPerson) -> c_int {
    if p.is_null() {
        set_err("句柄为空");
        return -1;
    }
    let fp = unsafe { &*p };
    fp.rt.block_on(fp.person.leave());
    0
}

/// 收一件事：返回 1 = 拿到事件，0 = 这一阵没有（超时），-1 = 出错。
/// timeout_ms < 0 表示死等，0 表示看一眼就走。
/// 不关心的事件会被**丢弃**（跳过即取下一件）。
///
/// # Safety
/// p 合法句柄；out_event 指向可写的 e2ee_event。
#[unsafe(no_mangle)]
pub extern "C" fn e2ee_poll(p: *mut FfiPerson, timeout_ms: c_int, out_event: *mut E2eeEvent) -> c_int {
    if p.is_null() || out_event.is_null() {
        set_err("参数为空");
        return -1;
    }
    let fp = unsafe { &*p };

    enum Outcome {
        Got(Event),
        TimedOut,
        Closed,
    }
    let fut = async {
        let mut ev = fp.events.lock().await;
        if timeout_ms < 0 {
            match ev.recv().await {
                Some(e) => Outcome::Got(e),
                None => Outcome::Closed,
            }
        } else {
            match timeout(Duration::from_millis(timeout_ms as u64), ev.recv()).await {
                Ok(Some(e)) => Outcome::Got(e),
                Ok(None) => Outcome::Closed,
                Err(_) => Outcome::TimedOut,
            }
        }
    };
    match fp.rt.block_on(fut) {
        Outcome::Got(e) => {
            unsafe { *out_event = to_c_event(e) };
            1
        }
        Outcome::TimedOut => 0,
        Outcome::Closed => {
            set_err("事件流已关闭");
            -1
        }
    }
}

/// 事件 → C 结构。
fn to_c_event(ev: Event) -> E2eeEvent {
    let mut e = E2eeEvent {
        tag: EVENT_UNKNOWN,
        id: 0,
        focused: 0,
        members: 0,
        name: [0; 64],
        text: [0; 512],
    };
    match ev {
        Event::Knocked { peer } => {
            e.tag = EVENT_KNOCKED;
            copy_str(&mut e.name, &peer);
        }
        Event::Met {
            id,
            name,
            focused,
        } => {
            e.tag = EVENT_MET;
            e.id = id;
            e.focused = c_int::from(focused);
            copy_str(&mut e.name, &name);
        }
        Event::MeetFailed { error } => {
            e.tag = EVENT_MEET_FAILED;
            copy_str(&mut e.text, &error);
        }
        Event::Heard { id, name, text } => {
            e.tag = EVENT_HEARD;
            e.id = id;
            copy_str(&mut e.name, &name);
            copy_str(&mut e.text, &text);
        }
        Event::Left { id, name, reason } => {
            e.tag = EVENT_LEFT;
            e.id = id;
            copy_str(&mut e.name, &name);
            let r: String = match reason {
                LeaveReason::Farewell => "farewell".into(),
                LeaveReason::Disconnected => "disconnected".into(),
                LeaveReason::Undecipherable => "undecipherable".into(),
                LeaveReason::Wire(w) => w,
                _ => "unknown".into(),
            };
            copy_str(&mut e.text, &r);
        }
        Event::FocusReturned { to } => {
            e.tag = EVENT_FOCUS_RETURNED;
            e.id = to.unwrap_or(0);
        }
        Event::SecretChained { with } => {
            e.tag = EVENT_SECRET_CHAINED;
            copy_str(&mut e.name, &with);
        }
        Event::CircleFormed { members } => {
            e.tag = EVENT_CIRCLE_FORMED;
            e.members = members as c_int;
        }
        Event::CircleMumble { from } => {
            e.tag = EVENT_CIRCLE_MUMBLE;
            copy_str(&mut e.name, &from);
        }
        _ => {} // 库将来新增的经历：tag 留 0，向前兼容
    }
    e
}
