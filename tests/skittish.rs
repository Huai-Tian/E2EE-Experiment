//! P21 惊弓之鸟（--skittish）的 CLI 集成测试：真实二进制 + 真实信号。
//! 沙箱无桌面（无 DBUS_SESSION_BUS_ADDRESS、无 /dev/input）→ 两条
//! 触发面的响亮降级路径恰好可测；有桌面的环境跳过对应断言并打印
//! 理由（静默跳过=罪）。合盖/锁屏的桌面功能面由 owner 双平台清单
//! 实测（Linux VM 锁屏、Windows 真机合盖+锁屏+Ctrl-C 清屏）。

#![allow(non_snake_case)]
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CLEAR: &str = "\x1b[2J\x1b[3J\x1b[H";

/// 起一个二进制实例，把 stderr 逐行收进共享账本，等锚点行出现。
/// stdin 保持管道敞开（不 EOF——EOF 也会触发统一退出，别让它抢戏）。
fn spawn_and_wait_for(args: &[&str], anchor: &str) -> (Child, Arc<Mutex<Vec<String>>>) {
    // 临时端口：bind :0 拿号即让（微小竞态，测试容错）
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind 临时端口");
        l.local_addr().expect("local_addr").port()
    };
    let addr = format!("127.0.0.1:{port}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_E2EE-Experiment"))
        .args(args)
        .arg(&addr)
        .arg("-n")
        .arg("测试员")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起二进制");
    let stderr = child.stderr.take().expect("stderr 管道");
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    std::thread::spawn(move || {
        // 进程退出后管道 EOF，线程自然收摊；活着时持续排空防塞
        for line in BufReader::new(stderr).lines() {
            match line {
                Ok(l) => sink.lock().expect("stderr 账本没被毒化").push(l),
                Err(_) => break,
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if lines
            .lock()
            .expect("stderr 账本没被毒化")
            .iter()
            .any(|l| l.contains(anchor))
        {
            return (child, lines);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let seen = lines.lock().expect("stderr 账本没被毒化").join("\n");
            panic!("10 秒内没等到锚点行「{anchor}」。stderr 全文：\n{seen}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 给进程递一个 SIGINT（终端 Ctrl-C 的信号面），等它退出并收回 stdout。
fn interrupt_and_collect(mut child: Child) -> (std::process::ExitStatus, String) {
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                panic!("进程没在 10 秒内响应 SIGINT（Ctrl-C 必须永远灵）");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let mut out = String::new();
    child
        .stdout
        .take()
        .expect("stdout 管道")
        .read_to_string(&mut out)
        .expect("读 stdout");
    (status, out)
}

/// 警惕模式下 Ctrl-C：横幅在场、退出码 0、stdout 恰好是清屏序列
///（CSI 2J + CSI 3J + CSI H——可视区 + 滚回缓冲 + 光标归位）。
#[test]
fn 惊弓之鸟_Ctrl_C_清屏退出() {
    let (child, lines) = spawn_and_wait_for(&["--skittish"], "惊弓之鸟模式已开启");
    let (status, out) = interrupt_and_collect(child);
    assert!(status.success(), "Ctrl-C 应走干净退出（退出码 0），实际 {status}");
    assert!(
        out.contains(CLEAR),
        "警惕模式下一切退出都是蒸发——stdout 必含清屏序列，实际 {out:?}"
    );
    let all = lines.lock().expect("stderr 账本没被毒化").join("\n");
    assert!(all.contains("每次合盖都会杀死全部对话"), "知情横幅要在场：\n{all}");
}

/// 普通模式对照：退出干净但不清屏（回看最后几行是正当需求）。
#[test]
fn 普通模式_Ctrl_C_不清屏() {
    let (child, _lines) = spawn_and_wait_for(&[], "等人来");
    let (status, out) = interrupt_and_collect(child);
    assert!(status.success(), "退出码应为 0，实际 {status}");
    assert!(!out.contains("\x1b[2J"), "普通模式退出不应清屏，stdout {out:?}");
}

/// 无桌面环境（沙箱常态）：两条触发面都必须响亮降级——静默的警惕
/// 比没有警惕更危险。有会话总线/有输入设备的真桌面环境跳过对应
/// 断言并打印理由（跳过要响亮）。
#[test]
fn 惊弓之鸟_无桌面环境_触发面降级响亮() {
    // 会话总线存在的判定：DBUS_SESSION_BUS_ADDRESS 已设，或
    // XDG_RUNTIME_DIR/bus 在盘上。两者皆无 → zbus 必连不上。
    let has_bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
        || std::env::var_os("XDG_RUNTIME_DIR")
            .map(|d| std::path::Path::new(&d).join("bus").exists())
            .unwrap_or(false);
    let has_input_dir = std::path::Path::new("/dev/input").exists();

    let (child, lines) = spawn_and_wait_for(&["--skittish"], "惊弓之鸟");
    // 给降级报告留到达时间（触发面线程各自异步报告）
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let all = lines.lock().expect("stderr 账本没被毒化").join("\n");
        let enough = (!has_bus || all.contains("锁屏触发面不可用"))
            && (!has_input_dir || all.contains("合盖触发面不可用"));
        if enough || Instant::now() > deadline {
            let all = all.clone();
            let (status, _out) = interrupt_and_collect(child);
            assert!(status.success());
            if !has_bus {
                assert!(
                    all.contains("锁屏触发面不可用"),
                    "无会话总线时必须响亮说明锁屏面不可用：\n{all}"
                );
            } else {
                eprintln!("（本机有会话总线——锁屏降级断言跳过，功能面交 owner 桌面实测）");
            }
            if !has_input_dir {
                assert!(
                    all.contains("合盖触发面不可用"),
                    "无 /dev/input 时必须响亮说明合盖面不可用：\n{all}"
                );
            } else {
                eprintln!("（本机有 /dev/input——合盖降级断言跳过；真有无 SW_LID 交 owner 实测）");
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
