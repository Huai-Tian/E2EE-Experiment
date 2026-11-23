//! CLI /file 路径保真的集成测试（真实二进制双进程）：待发账本必须存
//! 完整路径。4.2.0 的 bug：账本只存文件名，FileAccepted 时把名字当
//! 路径传给传输任务的二次 open——`/file` 给绝对路径或子目录路径时
//! 「申报成功、点头后递不出」（os error 2）；cwd 纯文件名形态下名字
//! 与路径恰好重合，掩盖了它。修复：账本 insert 改存 path。
//! 红绿自证：临时还原旧实现（名字入账）→ 本测试必须红；恢复修复 → 绿。
//! 跨平台（Windows/Linux CI 双跑）：无信号/DBUS 依赖，管道 stdin +
//! stderr 行账本 + 锚点轮询。

#![allow(non_snake_case)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 占一个临时端口（bind :0 拿号即让——微小竞态，测试容错）。
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind 临时端口");
    l.local_addr().expect("local_addr").port()
}

/// 起一个真实 CLI 实例：stdin 管道敞开、stderr 逐行收进共享账本。
fn spawn_cli(name: &str, port: u16, cwd: &std::path::Path) -> (Child, ChildStdin, Arc<Mutex<Vec<String>>>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_E2EE-Experiment"))
        .arg(format!("127.0.0.1:{port}"))
        .arg("-n")
        .arg(name)
        .current_dir(cwd)
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
    let stdin = child.stdin.take().expect("stdin 管道");
    (child, stdin, lines)
}

/// 轮询 stderr 账本等锚点行（默认 20 秒——拨号+握手+文件传输全程）。
fn wait_anchor(lines: &Arc<Mutex<Vec<String>>>, anchor: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if lines
            .lock()
            .expect("stderr 账本没被毒化")
            .iter()
            .any(|l| l.contains(anchor))
        {
            return;
        }
        if Instant::now() > deadline {
            let seen = lines.lock().expect("stderr 账本没被毒化").join("\n");
            panic!("20 秒内没等到锚点行「{anchor}」。stderr 全文：\n{seen}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 给进程喂一行命令（立即冲刷——滞留缓冲=命令迟到）。
fn send_line(stdin: &mut ChildStdin, line: &str) {
    writeln!(stdin, "{line}").expect("写命令");
    stdin.flush().expect("冲命令");
}

/// 收摊：EOF（统一退出）+ kill 兜底，别让进程漏到下一个测试。
fn reap(child: &mut Child) {
    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait().expect("try_wait") {
            Some(_) => return,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                return;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// 场景：文件放在发送方工作目录的**子目录**里（路径与文件名不重合——
/// 正是 cwd 纯文件名形态掩盖不住的路径形态），`/file` 给完整路径，
/// 收方始终态（receive always）自动收。断言收到的字节一毫不差。
#[test]
fn cli_file_子目录路径照传_账本存的是路径不是名字() {
    let base = std::env::temp_dir().join(format!(
        "e2ee_cli_file_test_{}",
        std::process::id() as u64
    ));
    let sub = base.join("src_dir"); // 子目录：路径 ≠ 文件名
    let recv = base.join("recv_home"); // 甲的工作目录（received/ 落在这）
    let send = base.join("send_home"); // 乙的工作目录
    for d in [&sub, &recv, &send] {
        std::fs::create_dir_all(d).expect("建目录");
    }

    // 源载荷：2KB 确定性字节（非全零——避开稀疏巧合）
    let payload: Vec<u8> = (0..2048u32).map(|i| (i * 31 % 251) as u8).collect();
    let file = sub.join("tu-zhan.bin");
    std::fs::write(&file, &payload).expect("写源文件");

    // 甲先出生（收方，始终态）；乙再出生（发方）
    let port_a = free_port();
    let (mut child_a, mut stdin_a, lines_a) = spawn_cli("jia", port_a, &recv);
    wait_anchor(&lines_a, "在听");
    let port_b = free_port();
    let (mut child_b, mut stdin_b, lines_b) = spawn_cli("yi", port_b, &send);
    wait_anchor(&lines_b, "在听");

    // 甲：这条线今后都直接收（绕开 ask 的应答时序，测试盯的是路径保真；
    // /receive 作用于聚焦线——须在对话建立之后）
    send_line(&mut stdin_b, &format!("/dial 127.0.0.1:{port_a}"));
    wait_anchor(&lines_b, "对话 #1 开始");
    send_line(&mut stdin_a, "/receive always");
    std::thread::sleep(Duration::from_millis(300));

    // 乙：递子目录里的文件（完整路径）
    send_line(&mut stdin_b, &format!("/file {}", file.display()));

    // 收方锚点：收到 + 落盘说明（「放在 received\…」）
    wait_anchor(&lines_a, "收到");

    // 体面收场（bye 双侧）+ 统一退出
    send_line(&mut stdin_a, "/bye");
    send_line(&mut stdin_b, "/bye");
    std::thread::sleep(Duration::from_millis(500));
    reap(&mut child_a);
    reap(&mut child_b);

    // 断言：received/ 下的文件字节一毫不差
    let got = recv.join("received").join("tu-zhan.bin");
    let got_bytes = std::fs::read(&got).unwrap_or_else(|e| {
        let seen_a = lines_a.lock().expect("账本").join("\n");
        let seen_b = lines_b.lock().expect("账本").join("\n");
        panic!("没收到 {got:?}：{e}\n甲 stderr：\n{seen_a}\n乙 stderr：\n{seen_b}");
    });
    assert_eq!(got_bytes, payload, "收到的字节必须与源一毫不差");

    // 清场
    let _ = std::fs::remove_dir_all(&base);
}

/// 场景：绝对路径必须同样成立（与子目录同一修复面，独立锚定的第二
/// 形态——Windows 与 Linux 的绝对路径拼法不同，双平台各自覆盖）。
#[test]
fn cli_file_绝对路径照传() {
    let base = std::env::temp_dir().join(format!(
        "e2ee_cli_file_abs_{}",
        std::process::id() as u64
    ));
    let recv = base.join("recv_home");
    let send = base.join("send_home");
    for d in [&recv, &send] {
        std::fs::create_dir_all(d).expect("建目录");
    }

    let payload: Vec<u8> = (0..1024u32).map(|i| (i * 17 % 253) as u8).collect();
    // 绝对路径直写（不在进程 cwd——名字与路径彻底不重合）
    let file = base.join("kua-mu-lu.bin");
    std::fs::write(&file, &payload).expect("写源文件");

    let port_a = free_port();
    let (mut child_a, mut stdin_a, lines_a) = spawn_cli("jia2", port_a, &recv);
    wait_anchor(&lines_a, "在听");
    let port_b = free_port();
    let (mut child_b, mut stdin_b, lines_b) = spawn_cli("yi2", port_b, &send);
    wait_anchor(&lines_b, "在听");

    send_line(&mut stdin_b, &format!("/dial 127.0.0.1:{port_a}"));
    wait_anchor(&lines_b, "对话 #1 开始");
    // /receive 作用于聚焦线——对话建立之后才设（绝对路径场景同款时序）
    send_line(&mut stdin_a, "/receive always");
    std::thread::sleep(Duration::from_millis(300));
    send_line(&mut stdin_b, &format!("/file {}", file.display()));
    wait_anchor(&lines_a, "收到");

    send_line(&mut stdin_a, "/bye");
    send_line(&mut stdin_b, "/bye");
    std::thread::sleep(Duration::from_millis(500));
    reap(&mut child_a);
    reap(&mut child_b);

    let got = recv.join("received").join("kua-mu-lu.bin");
    let got_bytes = std::fs::read(&got).unwrap_or_else(|e| {
        let seen_a = lines_a.lock().expect("账本").join("\n");
        let seen_b = lines_b.lock().expect("账本").join("\n");
        panic!("没收到 {got:?}：{e}\n甲 stderr：\n{seen_a}\n乙 stderr：\n{seen_b}");
    });
    assert_eq!(got_bytes, payload, "收到的字节必须与源一毫不差");

    let _ = std::fs::remove_dir_all(&base);
}
