//! 第一号使用者：终端里的一个人。
//! 库负责语义——动作从方法进，经历从事件出；这里只把事件说成人话。

use e2ee::{Event, LeaveReason, Person};
use std::io::BufRead;
use std::process::ExitCode;

/// 耳朵默认守着的地址
const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:7777";

const HELP: &str = "\
E2EE-Experiment —— 一个像人一样的二进制：启动即在场，能听能说，转身即忘。

用法：
  E2EE-Experiment [监听地址] [-n 名字]
      出生。默认在 0.0.0.0:7777 上听，等人来，也随时可以主动去找人。
      同一台机器开两个人时，记得用不同端口。
  E2EE-Experiment --guard [监听地址] [-n 名字]
      常备门禁出生：每位来客都要对上同一句机器暗号才谈得成，且从不
      消耗——瞎拨多少次也锁不住台，真钥永远进得来。这句应是 32 字节级
      的随机机器密钥（不是人类暗号：一次性切口「烧掉」防的是对低熵暗号
      的在线爆破，机器密钥在线爆破本就不可能）。
      钥匙在启动后按提示悄悄输入（不回显）——钥匙不进命令行：本机任何
      账户都读得到进程启动命令，shell 历史也会记下它。
  E2EE-Experiment --skittish [监听地址] [-n 名字]
      警惕模式出生：合盖或锁屏的瞬间，一切将被遗忘，程序蒸发
      离场——擦净秘密、清空屏幕（含滚回缓冲）、退出，不等网络。
      锁屏含系统自动锁；只收确定性的物理事件，机器推断永不触发。
      警惕模式下一切退出都会清屏，包括 Ctrl-C。无需修改任何系统
      设置；触发面不可用时会响亮说明。详见 README 的「警惕者手册」。
  E2EE-Experiment --courier [监听地址]
      开茶馆：同一个二进制的另一份工。认房号、接线、只搬看不懂的字节
      （房号由客人带外自选，茶馆只认号接线）。
      部署在任意有公网 IP 的机器上；无身份、无存储、无明文。

在场指令：
  /dial <ADDR:PORT> [暗号]
      主动去找人；带暗号则对方必须也对上才谈得成
  /meet <茶馆地址> <房号> [暗号]
      经茶馆找事先约好的人（双方报同一房号；带暗号则双方须一致）
  /punch <介绍人地址> <标签> [暗号]
      经介绍人打洞直连（双方报同一标签；打不通就换 /meet 走茶馆，不自动回退）
  /await <暗号>
      备好切口：下一位来客对得上才谈得成（一位一验，验完即焚）
  /file <文件路径>
      递一份文件给当前对话的人（≤32MB，分块加密，字节无损；
      收到的文件自动放进 ./received/ 目录）
  /talk <编号|名字>     把注意力切到另一场对话
  /list                看看在场的人
  /card                打印自己的名片（名字 + 各网卡地址）
  /circle [封桌|开台]
      开台/围坐/点名：署名邀请名单=当前在场的人；名单上的
      名字自署到齐后铸群钥。在圈中重跑 = 点名重聚（修复路径）。
      多人默认封桌、一人默认开台；可显式覆盖（开台是刻意的宣言）
  /gsay <话>           在圈内说一句（加密一次，人人同时收到）
  /bring [拟名]        领人入座：向当前对话的人（候选人）递看座骨架。
                       带拟名则开期望门（回执名字不符 → 响亮暂停）；
                       不带 = 无门直接自动。封桌警告但照发——
                       拒绝是全员契约行为，不是发起人的自我审查
  /sit [名字]          落座：回应看座邀约（A1：落座署名必须本人触发）。
                       默认用你在这条线上的自报名，可覆盖
                       （「这桌叫我阿花」是自己的嗓音）
  /word                对词：显示当前信道的短词（聚焦 1:1 的会话短词 /
                       在圈的群短词，12 位数字分 4 组）。两边各看各的
                       屏幕，想核对的人口头对一遍——念的不一样
                       = 这条线上有人拼话。短词即时显示、永不落盘
  /bye                 和当前对话的人道别（在圈则先署名告退；程序继续在场）
  Ctrl-C 或 Ctrl-D     立即离场：擦净一切秘密，不告别、不等网络——
                       对面看到的是掉线。想体面地走：先 /bye 再 Ctrl-C。
                       连按两次 Ctrl-C = 强制立即退出（清理卡住时的保险阀）

这条线上只有你们两端能听懂彼此；对面是谁，由你自己判断。
无账户 · 无身份 · 无存储 —— 每次连接都是初次见面。";

#[tokio::main]
async fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    // 内存卫生（T1 通道封堵）：转储核心文件 = 把全部明文秘密写进磁盘。
    // 关掉它；再提醒未加密 swap 的存在（swap 落盘是另一条 T1 通道）。
    // 这是 CLI 这个使用者的进程策略——库本体不碰进程级设置。
    harden_process();

    // P20-L 密钥集锁定探测：锁不住就响亮说（库不打印——api_style；
    // 运行中的降级经 Event::KeyLockDegraded 走体验流）。
    if !e2ee::lock::probe() {
        eprintln!("警告：本机的密钥内存锁定（mlock）不可用——暗号/门禁钥匙将退回普通擦除内存，存在被换出到磁盘的可能。程序照常运行；如需启用，请检查系统的内存锁定限额（ulimit -l）。");
    }

    let mut name = String::from("无名氏");
    let mut listen_addr = DEFAULT_LISTEN_ADDR.to_string();
    let mut as_courier = false;
    let mut guard_prompt = false;
    let mut skittish = false;
    let mut positional: Vec<String> = Vec::new();

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        match arg {
            "help" | "--help" | "-h" => {
                println!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "--courier" => as_courier = true,
            // 警惕模式（P21，rulings.2026_10_31_skittish_spec）：合盖/锁屏
            // 即蒸发。CLI 专属消费端手势——库零改动（kept.skittish_cli_only）。
            "--skittish" => skittish = true,
            // --no-chain 已随暗号链移除而作废（rulings.2026_10_31_realtime_
            // verification_razor）——没有链可关，旗子退役。
            "--guard" => {
                // 耳语式（kept.guard_whisper_input）：钥匙不进命令行。
                // 后面跟的若不是地址形（IP:PORT），按旧式「--guard 钥匙」
                // 理解——响亮拒绝并指路。
                if let Some(next) = argv.get(i + 1) {
                    if !next.starts_with('-')
                        && next.parse::<std::net::SocketAddr>().is_err()
                    {
                        eprintln!(
                            "钥匙不进命令行——本机任何账户都读得到进程启动命令（/proc 公告栏），shell 历史也会记下它。\n请用：--guard 启动后按提示悄悄输入。"
                        );
                        return Ok(ExitCode::from(2));
                    }
                }
                guard_prompt = true;
            }
            "-n" | "--name" => match argv.get(i + 1) {
                Some(n) => {
                    name = n.clone();
                    i += 1;
                }
                None => {
                    eprintln!("-n 后面要跟名字\n");
                    eprintln!("{HELP}");
                    return Ok(ExitCode::from(2));
                }
            },
            other if other.starts_with('-') => {
                eprintln!("不认识的参数：{other}\n");
                eprintln!("{HELP}");
                return Ok(ExitCode::from(2));
            }
            _ => positional.push(arg.to_string()),
        }
        i += 1;
    }
    if let Some(addr) = positional.into_iter().next() {
        listen_addr = addr;
    }

    // 另一份工：开茶馆，永不下班
    if as_courier {
        if skittish {
            eprintln!("注意：--skittish 对茶馆无意义（茶馆无身份、无存储、无明文——无物可忘），已忽略。");
        }
        e2ee::courier::serve(&listen_addr).await?;
        return Ok(ExitCode::SUCCESS);
    }

    // 出生；此后一半心思听事件，一半心思读输入。
    let (person, mut events) = Person::born(&name, &listen_addr).await?;
    let mut notes = String::new();
    // 常备门禁（耳语式，kept.guard_whisper_input）：钥匙不进命令行
    //（/proc 公告栏与 shell 历史），启动后在这里悄悄问你要——无回显。
    if guard_prompt {
        let key = read_line_hidden("常备门禁钥匙（输入不回显，回车确认）：")?;
        if key.is_empty() {
            eprintln!("没给钥匙——门禁未立，继续无门禁运行。");
        } else {
            person.expect_secret_stable(key.as_bytes()).await;
            notes.push_str("（常备门禁：每位来客都要对上暗号）");
        }
    }
    eprintln!("我是 {name}，在听 {listen_addr}{notes}。等人来，或 /dial 找人（Ctrl-C 离场）");
    // P21 警惕模式知情横幅：这不是默认行为，是用户自己选的易惊状态——
    // 姿态必须在启动时响亮说清（rulings.2026_10_31_skittish_spec）。
    if skittish {
        eprintln!("警惕模式已开启：这台机器处于易惊状态——合盖或锁屏的瞬间，一切将被遗忘，程序蒸发离场。每次合盖都会杀死全部对话。");
    }

    // 输入线：独立 std 线程阻塞读 stdin，逐行递给异步世界（P20-E①）。
    // 为什么不用 tokio::io::stdin()：它的读跑在 runtime 的阻塞池上，而
    // Runtime::drop 会等阻塞任务完成——stdin 不 EOF 就永远等下去（实测：
    // 单次 Ctrl-C 后循环正确退出、worker 干净收摊，进程却假死在 runtime
    // 关机上，直到第二次 Ctrl-C 才被强制阀带走）。独立线程不在 drop 路
    // 径上：main 返回即进程退出，线程随进程消亡。EOF（Ctrl-D ≡ Ctrl-C）
    // 经通道关闭传递：线程结束时 tx drop → rx 收到 None。
    let (line_tx, mut line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if line_tx.send(l).is_err() {
                        return; // 主循环已走：别拦着进程退出
                    }
                }
                Err(_) => return,
            }
        }
    });
    // 统一终止信号面（P20-E①，rulings.2026_10_31_exit_taxonomy）：
    // Ctrl-C / SIGTERM（unix）、控制台事件（windows：Ctrl-C/X 按钮/注销，
    // 经 SetConsoleCtrlHandler 桥到静态 Notify）——汇成一条通道。
    let (mut term, term_tx) = terminate_signal();
    // P21 警惕模式：布防触发面——合盖/锁屏（各平台的确定性物理事件）
    // 直通统一终止通道。触发在语义上就是一次终止手势，复用同一条
    // 干净退出路径（擦→清屏→走人，警惕模式下次序见函数尾）。
    if skittish {
        arm_skittish(term_tx.clone());
    }
    loop {
        tokio::select! {
            line = line_rx.recv() => {
                // Ctrl-D/语尽 ≡ Ctrl-C（终端 EOF 惯例，非本项目发明）
                let Some(line) = line else { break };
                let line = line.trim_end();
                if line.is_empty() {
                    continue; // 沉默不必传递
                }
                if let Some(cmd) = line.strip_prefix('/') {
                    if handle_command(cmd, &person).await? {
                        break;
                    }
                } else if let Err(e) = person.speak(line).await {
                    eprintln!("{e}");
                }
            }
            maybe = events.recv() => {
                let Some(ev) = maybe else { break };
                render(ev);
            }
            _ = term.recv() => break,
        }
    }

    // ── 统一退出（P20-E①）：立即、擦净、不告别、不等网络 ─────────────
    // 人的死亡（A1）：不发告别帧、不等任何网络。Drop 链自然跑完——密钥
    // 归零（库的既有语义），耳朵任务随运行时收摊被中止，对端视角=掉线
    //（圈子按 crash 路径见证换气，A4 完整保留）。main 返回即进程退出，
    // 输入线程随进程消亡。
    // 二次 Ctrl-C = 强制阀（exit_taxonomy ④）：独立 std 线程阻塞收信号
    //（blocking_recv，不依赖 runtime 存活——万一清理卡在深处，阀仍在），
    // 第二次不再等，直接走——保住『Ctrl-C 永远灵』的手感。
    std::thread::spawn(move || {
        if term.blocking_recv().is_some() {
            std::process::exit(130);
        }
    });
    drop(person);
    drop(events);
    // P21 警惕模式：警惕模式下一切退出都是蒸发——秘密销毁之后，屏幕上
    // 的影子（含滚回缓冲）一并清空；Ctrl-C 也一样（spec 钉死）。普通
    // 模式不清屏：回看最后几行是正当需求。
    if skittish {
        clear_screen();
    }
    Ok(ExitCode::SUCCESS)
}

/// 统一终止信号面（P20-E① + P21）：把平台上『可捕获的终止手势』汇成
/// 一条通道。每收到一项 = 一次终止手势（Ctrl-C / SIGTERM / Windows
/// 控制台事件；P21 起，警惕模式的合盖/锁屏触发也走这里）。发送端随
/// 接收端一并返回——触发面复用它，触发=终止手势，同一个语义。
fn terminate_signal() -> (
    tokio::sync::mpsc::UnboundedReceiver<()>,
    tokio::sync::mpsc::UnboundedSender<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).expect("注册 SIGINT");
        let tx1 = tx.clone();
        tokio::spawn(async move {
            while sigint.recv().await.is_some() {
                if tx1.send(()).is_err() {
                    break;
                }
            }
        });
        let mut sigterm = signal(SignalKind::terminate()).expect("注册 SIGTERM");
        let tx2 = tx.clone();
        tokio::spawn(async move {
            while sigterm.recv().await.is_some() {
                if tx2.send(()).is_err() {
                    break;
                }
            }
        });
    }
    #[cfg(windows)]
    {
        install_console_ctrl_handler();
        let tx1 = tx.clone();
        tokio::spawn(async move {
            loop {
                CONSOLE_EXIT.notified().await;
                if tx1.send(()).is_err() {
                    break;
                }
            }
        });
    }
    (rx, tx)
}

// ═══════════════════════════════════════════════════════════════
// P21 警惕模式（--skittish，rulings.2026_10_31_skittish_spec）
// 触发=合盖+锁屏（只收确定性物理事件）；威胁模型=触发即当设备已被
// 读型取证接管（写型=T0 边界）；动作=擦→清屏→统一退出；不关机不关窗
//（伪装原则：机器只自动化不可见的事）。以下全是 CLI 消费端进程策略，
// 与 harden_process 同族——库零改动（kept.skittish_cli_only）。
// ═══════════════════════════════════════════════════════════════

/// 清屏并抹掉滚回缓冲（警惕模式的蒸发面）：CSI 2J 清可视区、CSI 3J
/// 清滚回缓冲（真正的历史在那里）、CSI H 光标归位。诚实边界（P20-D
/// 同款入档）：ANSI 清屏是终端的逻辑清除——终端进程自己的内存字节
/// 未必归零（T2/T3 窗口），那一层由人手电源键覆盖（手册四档表）。
fn clear_screen() {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(b"\x1b[2J\x1b[3J\x1b[H");
    let _ = out.flush();
}

/// 触发面布防总入口：把各平台的确定性物理事件接进统一终止通道。每个
/// 触发面各自响亮报状（loud-failure 家族）：连不上就大声说，绝不静默
/// 假装在防——静默的警惕比没有警惕更危险。
fn arm_skittish(fire: tokio::sync::mpsc::UnboundedSender<()>) {
    #[cfg(target_os = "linux")]
    {
        watch_linux_lock_screen(fire.clone());
        std::thread::spawn(move || watch_lid_evdev(fire));
    }
    #[cfg(windows)]
    {
        std::thread::spawn(move || watch_windows_session(fire));
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        // 其他平台（macOS 等）：触发面未实现——响亮声明，不装样子。
        // Ctrl-C 清屏语义仍完整（skittish 退出路径不挑触发源）。
        let _ = fire;
        eprintln!(
            "警惕模式降级：此平台的合盖/锁屏监听未实现——触发面不可用，\
             警惕模式只剩 Ctrl-C 也清屏的语义。"
        );
    }
}

/// Linux 锁屏触发面：会话总线上的屏保广播，两个名字都听——
/// org.freedesktop.ScreenSaver（KDE/xdg 门户面）与 org.gnome.ScreenSaver
/// （GNOME 面），ActiveChanged(true) = 会话已锁（含 OS 自动锁：事件是
/// 『会话已锁』这一事实，不区分手动自动）。连不上会话总线（无桌面/
/// 纯 SSH/无 DBUS_SESSION_BUS_ADDRESS 环境）= 响亮降级。
#[cfg(target_os = "linux")]
fn watch_linux_lock_screen(fire: tokio::sync::mpsc::UnboundedSender<()>) {
    tokio::spawn(async move {
        if let Err(e) = watch_lock_screen_inner(fire).await {
            eprintln!(
                "警惕模式降级：锁屏触发面不可用（{e}）——锁屏时程序不会\
                 自动离场；Ctrl-C 依然立即擦净。"
            );
        }
    });
}

#[cfg(target_os = "linux")]
async fn watch_lock_screen_inner(
    fire: tokio::sync::mpsc::UnboundedSender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = zbus::Connection::session().await?;
    for iface in ["org.freedesktop.ScreenSaver", "org.gnome.ScreenSaver"] {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(iface)
            .member("ActiveChanged")
            .build()?;
        let mut stream = zbus::MessageStream::for_match_rule(rule, &conn, None).await?;
        let fire = fire.clone();
        // 屏保信号是广播，本程序只是旁听者：不向总线注册任何名字、不
        // 发任何东西——总线上零足迹（伪装原则：不可见才自动化）。
        tokio::spawn(async move {
            // zbus 的流是 futures_core::Stream（经 zbus re-export 引入，
            // 不为 next() 再添一颗零件）；std::future::poll_fn 手动拉取。
            use std::future::poll_fn;
            use std::pin::Pin;
            use zbus::export::futures_core::Stream;
            while let Some(Ok(msg)) = poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await {
                // body = (active: bool,)；只在 true（锁上）时触发——
                // false 是解锁，那时我们早已不在了。
                if let Ok((active,)) = msg.body().deserialize::<(bool,)>() {
                    if active && fire.send(()).is_ok() {
                        return; // 蒸发即离场，任务随运行时收摊
                    }
                }
            }
        });
    }
    eprintln!("警惕模式：锁屏触发面已布防（会话总线屏保信号）。");
    Ok(())
}

/// Linux 合盖触发面：evdev 旁听 /dev/input/event* 里的盖子开关
/// （EV_SW/SW_LID，value=1 即合上）。手写 ioctl（libc 无 EVIOCGBIT
/// 常量，_IOC 编码自算）；绝不 EVIOCGRAB 独占——盖子事件属于整个
/// 桌面，我们只旁听。找不到盖子传感器（台式机/虚拟机常态）或无权限
/// 读 /dev/input（需 input 组成员）= 响亮降级。
#[cfg(target_os = "linux")]
fn watch_lid_evdev(fire: tokio::sync::mpsc::UnboundedSender<()>) {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    const EV_SW: u16 = 0x05; // 开关类事件
    const SW_LID: u16 = 0x00; // 盖子开关（合上=1，掀开=0）
    /// EVIOCGBIT(ev, len) = _IOR('E', 0x20 + ev, len)：查设备支持的事件位图。
    /// 返回 libc::Ioctl 而非写死宽度——glibc 面是 c_ulong（u64），musl/
    /// bionic 面是 c_int（i32；bit31 落在符号位，截断转换正是 C 宏在
    /// 32 位 request 域里的原生形态）。
    const fn eviocgbit(ev: u16, len: usize) -> libc::Ioctl {
        // _IOC(dir,type,nr,size) = dir<<30 | size<<16 | type<<8 | nr；_IOC_READ=2
        ((2u64 << 30)
            | ((len as u64) << 16)
            | ((b'E' as u64) << 8)
            | (0x20 + ev as u64)) as libc::Ioctl
    }

    // 1. 扫描 /dev/input/event*，探测哪个口上有盖子开关
    let mut watched: Vec<std::fs::File> = Vec::new();
    let mut saw_denied = false;
    let entries = match std::fs::read_dir("/dev/input") {
        Ok(e) => e,
        Err(_) => {
            eprintln!(
                "警惕模式降级：读不了 /dev/input（目录不存在或无权限）\
                 ——合盖触发面不可用。合盖时程序不会自动离场；Ctrl-C 依然\
                 立即擦净。"
            );
            return;
        }
    };
    for ent in entries.flatten() {
        let Some(name) = ent.file_name().into_string().ok() else { continue };
        if !name.starts_with("event") {
            continue;
        }
        // O_NONBLOCK：探测期不挂死；监听期读空缓冲返回 EAGAIN 收工
        let file = match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(ent.path())
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                saw_denied = true;
                continue;
            }
            Err(_) => continue,
        };
        let fd = file.as_raw_fd();
        unsafe {
            // 事件类型位图：byte[0] 的 bit5 = EV_SW 支持
            let mut evbits = [0u8; 4];
            if libc::ioctl(fd, eviocgbit(0, evbits.len()), evbits.as_mut_ptr()) < 0 {
                continue;
            }
            if evbits[0] & (1 << EV_SW) == 0 {
                continue;
            }
            // 开关位图：byte[0] 的 bit0 = SW_LID 支持
            let mut swbits = [0u8; 8];
            if libc::ioctl(fd, eviocgbit(EV_SW, swbits.len()), swbits.as_mut_ptr()) < 0 {
                continue;
            }
            if swbits[0] & (1 << SW_LID) == 0 {
                continue;
            }
        }
        watched.push(file);
    }
    if watched.is_empty() {
        if saw_denied {
            eprintln!(
                "警惕模式降级：无权限读 /dev/input（把自己加入 input 组\
                 可解）——合盖触发面不可用。合盖时程序不会自动离场；\
                 Ctrl-C 依然立即擦净。"
            );
        } else {
            eprintln!(
                "警惕模式降级：未找到盖子传感器（/dev/input 下没有\
                 SW_LID 设备——台式机/虚拟机常态）——合盖触发面不可用。\
                 合盖时程序不会自动离场；Ctrl-C 依然立即擦净。"
            );
        }
        return;
    }
    eprintln!(
        "警惕模式：合盖触发面已布防（{} 个盖子开关在耳）。",
        watched.len()
    );

    // 2. 阻塞旁听：poll 全体 fd，见到 EV_SW/SW_LID=1 即触发
    let mut fds: Vec<libc::pollfd> = watched
        .iter()
        .map(|f| libc::pollfd {
            fd: f.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    // input_event 布局：timeval + type(2) + code(2) + value(4)
    //（LP64=24 字节，32 位平台=16 字节——按本机 timeval 尺寸自适应）
    let tv = std::mem::size_of::<libc::timeval>();
    let ev_size = tv + 8;
    let mut buf = [0u8; 512];
    loop {
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue; // 被信号打断，重听
            }
            eprintln!("警惕模式：盖子监听退出（poll 失败：{e}）——合盖触发面不再可用。");
            return;
        }
        for fd in &fds {
            if fd.revents & libc::POLLIN == 0 {
                continue;
            }
            loop {
                let n = unsafe {
                    libc::read(fd.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if n <= 0 {
                    break; // EAGAIN（读空）或设备拔除
                }
                let mut off = 0usize;
                while off + ev_size <= n as usize {
                    let ty = u16::from_ne_bytes(buf[off + tv..off + tv + 2].try_into().unwrap());
                    let code =
                        u16::from_ne_bytes(buf[off + tv + 2..off + tv + 4].try_into().unwrap());
                    let val =
                        i32::from_ne_bytes(buf[off + tv + 4..off + tv + 8].try_into().unwrap());
                    if ty == EV_SW && code == SW_LID && val == 1 && fire.send(()).is_ok() {
                        return; // 蒸发即离场，线程随进程消亡
                    }
                    off += ev_size;
                }
            }
        }
    }
}

/// Windows 触发面：一枚 message-only 窗口（不可见、不抢焦点）做消息泵——
/// WTSRegisterSessionNotification 接会话锁（WM_WTSSESSION_CHANGE/
/// WTS_SESSION_LOCK=0x7）；盖子走 RegisterPowerSettingNotification 直听
/// GUID_LIDSWITCH_STATE_CHANGE——硬件状态通知，与电源动作设置无关
///（推翻早先『盖子无控制台通知』的错误入档，2026-11-01 owner 裁决）。
/// 睡眠不是触发器：自动睡眠源自闲置计时器（机器推断出局）；主动入睡
/// 的人有更快的手势——合盖/锁屏/Ctrl-C。
#[cfg(windows)]
fn watch_windows_session(fire: tokio::sync::mpsc::UnboundedSender<()>) {
    use std::cell::RefCell;
    thread_local! {
        /// 窗口过程回调没有闭包舱位——发送端经 thread_local 递进去。
        /// WndProc 永远在本线程上被调（GetMessage/Dispatch 的约定）。
        static FIRE: RefCell<Option<tokio::sync::mpsc::UnboundedSender<()>>> =
            RefCell::new(None);
    }
    FIRE.with(|f| *f.borrow_mut() = Some(fire));

    unsafe extern "system" fn skittish_wnd_proc(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        match msg {
            m if m == win::WM_WTSSESSION_CHANGE && wparam == win::WTS_SESSION_LOCK => {
                FIRE.with(|f| {
                    if let Some(tx) = f.borrow().as_ref() {
                        let _ = tx.send(());
                    }
                });
                0
            }
            m if m == win::WM_POWERBROADCAST && wparam == win::PBT_POWERSETTINGCHANGE => {
                // 载荷 = POWERBROADCAST_SETTING；盖子通知 Data：0x0=合上
                //（微软文档钉死）。状态通知与电源动作两层独立——与用户
                // 的合盖动作设置无关。
                let s = lparam as *const win::PowerBroadcastSetting;
                unsafe {
                    if (*s).power_setting == win::GUID_LIDSWITCH_STATE_CHANGE
                        && (*s).data_length >= 1
                        && (*s).data[0] == 0
                    {
                        FIRE.with(|f| {
                            if let Some(tx) = f.borrow().as_ref() {
                                let _ = tx.send(());
                            }
                        });
                    }
                }
                1 // TRUE：已处理
            }
            _ => unsafe { win::DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    unsafe {
        let class_name: Vec<u16> = "E2EE_SkittishSink\0".encode_utf16().collect();
        let wc = win::WndClassW {
            style: 0,
            lpfn_wnd_proc: Some(skittish_wnd_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: win::GetModuleHandleW(std::ptr::null()),
            h_icon: std::ptr::null_mut(),
            h_cursor: std::ptr::null_mut(),
            h_brush: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
        };
        if win::RegisterClassW(&wc) == 0 {
            eprintln!(
                "警惕模式降级：注册窗口类失败——合盖/锁屏触发面不可用。\
                 Ctrl-C 依然立即擦净。"
            );
            return;
        }
        let hwnd = win::CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            0,
            0, 0, 0, 0,
            win::hwnd_message(),
            std::ptr::null_mut(),
            wc.h_instance,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            eprintln!(
                "警惕模式降级：创建通知窗口失败——合盖/锁屏触发面不可用。\
                 Ctrl-C 依然立即擦净。"
            );
            return;
        }
        // 盖子直听：状态通知与电源动作两层独立——合盖动作怎么设都照发
        if win::RegisterPowerSettingNotification(
            hwnd,
            &win::GUID_LIDSWITCH_STATE_CHANGE,
            0,
        )
        .is_null()
        {
            eprintln!(
                "警惕模式降级：盖子通知注册失败——合盖触发面不可用\
                 （锁屏触发面仍在）。Ctrl-C 依然立即擦净。"
            );
        } else {
            eprintln!("警惕模式：合盖触发面已布防（盖子状态通知）。");
        }
        // NOTIFY_FOR_THIS_SESSION = 0：只听本会话
        if win::WTSRegisterSessionNotification(hwnd, 0) == 0 {
            eprintln!(
                "警惕模式降级：WTSRegisterSessionNotification 失败——锁屏\
                 触发面不可用（合盖触发面仍在）。Ctrl-C 依然立即擦净。"
            );
        } else {
            eprintln!("警惕模式：锁屏触发面已布防（会话通知）。");
        }
        let mut msg: win::Msg = std::mem::zeroed();
        loop {
            let r = win::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r <= 0 {
                // 0 = WM_QUIT（无人投递，不应发生）；-1 = 消息泵出错
                eprintln!("警惕模式：消息泵退出——触发面不再可用。");
                return;
            }
            let _ = win::TranslateMessage(&msg);
            let _ = win::DispatchMessageW(&msg);
        }
    }
}

/// 进程级加固（unix）：两件事，失败都静默——加固是尽力而为，不该挡住出生。
/// 1. RLIMIT_CORE=0 —— 本进程永不转储核心文件（core dump 会把全部
///    明文秘密整页写进磁盘，是 T1 持久化通道里最容易关的一条）。
/// 2. /proc/swaps 检查 —— 有未加密 swap 分区就 stderr 一行提醒。
///    zram（RAM 内压缩）不算落盘，不提醒；swap 加密与否无法从
///    /proc/swaps 看出，只能提醒用户自己确认。
#[cfg(unix)]
fn harden_process() {
    // 1. 拒绝 core dump
    unsafe {
        let lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let _ = libc::setrlimit(libc::RLIMIT_CORE, &lim);
        // P7（P-10 结构解，rulings.2026_10_17_p7_spec_lite_approved）：
        // PR_SET_DUMPABLE=0——RLIMIT_CORE 拦不住 core_pattern='|' 的
        // 管道收集器；DUMPABLE=0 让 core 从一开始就不产生、ptrace 附着
        // 一并被禁（ssh-agent 同款手段）。RLIMIT_CORE 留作腰带背带。
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
    }
    // 2. 未加密 swap 提醒
    if let Ok(text) = std::fs::read_to_string("/proc/swaps") {
        for line in text.lines().skip(1) {
            // 列：Filename Type Size Used Priority
            let mut parts = line.split_whitespace();
            let (Some(_path), Some(kind)) = (parts.next(), parts.next()) else {
                continue;
            };
            if kind == "partition" {
                eprintln!("提醒：检测到 swap 分区。若它未加密，被换出的内存页（可能含明文）会落到磁盘。建议 swap 加密或关闭。");
                break; // 提醒一次就够
            }
        }
    }
    // 3. 休眠画像提醒（4.0.0 P20-D）：/sys/power/state 含 disk ⇒ 本机可休眠。
    //    休眠=整个内存（含全部密钥）写进盘上休眠文件——T1 通道里应用层
    //    唯一无法关闭的一条（挂起是 OS 行为，抢跑=被否决的休眠前预处理族）。
    //    不拦截，只响亮提醒一次：静态配置事实的告知，loud-failure 家族。
    //    应用层的答案是手势遗忘（--skittish：合盖/锁屏即全忘）+ 系统设置。
    if let Ok(states) = std::fs::read_to_string("/sys/power/state") {
        if states.split_whitespace().any(|s| s == "disk") {
            eprintln!("提醒：本机可休眠（hibernate）——休眠会把整个内存（包括密钥）写进磁盘上的休眠文件，应用层无法拦截。建议在系统设置中关闭休眠；高风险用法可加 --skittish（合盖/锁屏即全忘）。");
        }
    }
}

#[cfg(not(unix))]
fn harden_process() {
    // ── 4.0.0 P20-W：Windows 分支不再是空壳 ─────────────────────────
    // 1. 未处理异常 → 立即自灭（TerminateProcess(self)）。原生异常（访问
    //    违例等）到达默认处理器时，Windows 错误报告（WER）会生成内存
    //    转储——整内存快照落盘，即 RLIMIT_CORE 的 Windows 同构 T1 通道。
    //    我们的过滤器抢先自杀：进程干净地死，WER 从未见过未处理异常，
    //    无转储、无『程序已停止工作』弹窗。这是 PR_SET_DUMPABLE=0 的
    //    Windows 同构手段（P-10 结构解：让泄密物从一开始就不产生）。
    //    诚实边界：__fastfail（Rust abort 的底层路径）与栈溢出可能绕过
    //    本过滤器——尽力而为，非保证，入档 known_limitations。
    //    为什么不用注册表 ExcludedApplications 排除 WER：那要写 HKCU
    //    = 在用户机器上留下持久痕迹，违反零持久化红线。
    // 2. SetErrorMode：抑制硬错误弹窗（弹窗会卡住退出路径）。
    // 3. pagefile 提醒（Unix /proc/swaps 警告的 Windows 同构）：用户态
    //    无法可靠查询 pagefile 加密状态，恒定提醒一次——静态配置事实
    //    的响亮告知，loud-failure 家族。
    unsafe {
        win::SetErrorMode(win::SEM_FAILCRITICALERRORS | win::SEM_NOGPFAULTERRORBOX);
        win::SetUnhandledExceptionFilter(Some(win::self_terminate_on_exception));
    }
    eprintln!(
        "提醒：Windows 页面文件（pagefile）若未加密，被换出的内存页（可能含明文）会落到磁盘。建议确认系统盘加密（如 BitLocker）或 pagefile 加密设置。"
    );
}

/// Windows kernel32 手写 FFI——不为三五个函数引入 windows-sys 依赖（单一二进制纪律）。
#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    pub(super) const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    pub(super) const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
    pub(super) const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6; // (DWORD)-10
    pub(super) const ENABLE_ECHO_INPUT: u32 = 0x0004;

    // ── P21 警惕模式触发面（message-only 窗口消息泵） ──────────────
    pub(super) const WM_POWERBROADCAST: u32 = 0x0218; // 电源事件广播
    pub(super) const WM_WTSSESSION_CHANGE: u32 = 0x02B1; // 会话状态变化
    pub(super) const PBT_POWERSETTINGCHANGE: usize = 0x8013; // 电源设置状态变化（盖子通知的容器；微软钉值 32787，owner 实测对账）
    pub(super) const WTS_SESSION_LOCK: usize = 0x7; // 会话已锁

    /// message-only 窗口的父句柄常量（HWND）(HWND)-3：不进 Z 序、
    /// 不收广播、不可见——只为收 WTS/电源通知而存在的影子窗口。
    pub(super) fn hwnd_message() -> *mut c_void {
        (-3isize) as *mut c_void
    }

    /// 窗口类（WNDCLASSW 的手写投影——不为它引 windows-sys，单一
    /// 二进制纪律；repr(C) 布局与 Win32 逐字段一致）
    #[repr(C)]
    pub(super) struct WndClassW {
        pub style: u32,
        pub lpfn_wnd_proc:
            Option<unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize>,
        pub cb_cls_extra: i32,
        pub cb_wnd_extra: i32,
        pub h_instance: *mut c_void,
        pub h_icon: *mut c_void,
        pub h_cursor: *mut c_void,
        pub h_brush: *mut c_void,
        pub lpsz_menu_name: *const u16,
        pub lpsz_class_name: *const u16,
    }

    /// 点坐标（POINT 投影——repr(C) 下 tuple 布局未指明，用显式结构）
    #[repr(C)]
    pub(super) struct Point {
        pub x: i32,
        pub y: i32,
    }

    /// GUID（Win32 repr(C) 投影）
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq)]
    pub(super) struct Guid {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    /// 电源设置通知载荷（POWERBROADCAST_SETTING 投影）
    #[repr(C)]
    pub(super) struct PowerBroadcastSetting {
        pub power_setting: Guid,
        pub data_length: u32,
        pub data: [u8; 1],
    }

    /// 盖子开关状态通知（微软文档：Data 0x0=合上、0x1=掀开——状态
    /// 通知与电源动作决策是两层，不依赖用户的合盖动作设置）
    pub(super) const GUID_LIDSWITCH_STATE_CHANGE: Guid = Guid {
        data1: 0xBA3E_0F4D,
        data2: 0xB817,
        data3: 0x4094,
        data4: [0xA2, 0xD1, 0xD5, 0x63, 0x79, 0xE6, 0xA0, 0xF3],
    };

    /// 消息泵缓冲（MSG 投影：内容不读，只递给系统转发）
    #[repr(C)]
    pub(super) struct Msg {
        pub hwnd: *mut c_void,
        pub message: u32,
        pub wparam: usize,
        pub lparam: isize,
        pub time: u32,
        pub pt: Point,
    }

    /// 未处理异常过滤器：只看『发生了』这件事，不解内容——立即自灭。
    /// 返回值不可达（TerminateProcess 不返回）。
    pub(super) unsafe extern "system" fn self_terminate_on_exception(
        _info: *mut c_void, // EXCEPTION_POINTERS*
    ) -> i32 {
        unsafe {
            TerminateProcess(GetCurrentProcess(), 0xE2EE);
        }
        0 // EXCEPTION_CONTINUE_SEARCH——不可达
    }

    unsafe extern "system" {
        pub(super) fn SetErrorMode(uMode: u32) -> u32;
        pub(super) fn SetUnhandledExceptionFilter(
            lpTopLevelExceptionFilter: Option<unsafe extern "system" fn(*mut c_void) -> i32>,
        ) -> *mut c_void;
        pub(super) fn GetCurrentProcess() -> *mut c_void;
        pub(super) fn TerminateProcess(hProcess: *mut c_void, uExitCode: u32) -> i32;
        pub(super) fn SetConsoleCtrlHandler(
            handler_routine: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
        pub(super) fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
        pub(super) fn GetConsoleMode(hConsoleHandle: *mut c_void, lpMode: *mut u32) -> i32;
        pub(super) fn SetConsoleMode(hConsoleHandle: *mut c_void, dwMode: u32) -> i32;
    }

    // P21：user32 消息泵四件套 + 模块句柄（默认链接不带 user32）
    #[link(name = "user32")]
    unsafe extern "system" {
        pub(super) fn RegisterClassW(lpwc: *const WndClassW) -> u16;
        pub(super) fn CreateWindowExW(
            dwexstyle: u32,
            lpclassname: *const u16,
            lpwindowname: *const u16,
            dwstyle: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            hwndparent: *mut c_void,
            hmenu: *mut c_void,
            hinstance: *mut c_void,
            lpparam: *mut c_void,
        ) -> *mut c_void;
        pub(super) fn DefWindowProcW(
            hwnd: *mut c_void,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> isize;
        pub(super) fn GetMessageW(
            lpmsg: *mut Msg,
            hwnd: *mut c_void,
            min: u32,
            max: u32,
        ) -> i32;
        pub(super) fn TranslateMessage(lpmsg: *const Msg) -> i32;
        pub(super) fn DispatchMessageW(lpmsg: *const Msg) -> isize;
        pub(super) fn GetModuleHandleW(lpmodulename: *const u16) -> *mut c_void;
        pub(super) fn RegisterPowerSettingNotification(
            hwnd: *mut c_void,
            powersettingguid: *const Guid,
            flags: u32,
        ) -> *mut c_void;
    }

    // P21：会话通知（wtsapi32；锁屏事件 WM_WTSSESSION_CHANGE 的订阅端）
    #[link(name = "wtsapi32")]
    unsafe extern "system" {
        pub(super) fn WTSRegisterSessionNotification(hwnd: *mut c_void, flags: u32) -> i32;
    }
}

/// Windows 控制台事件桥（P20-E①）：CTRL_C(0) / CTRL_CLOSE(2，X 按钮) /
/// CTRL_LOGOFF(5) / CTRL_SHUTDOWN(6) → 静态 Notify → 统一终止信号面。
/// notify_one 带许可语义：主循环还没走到 select 分支时事件也不丢。
#[cfg(windows)]
static CONSOLE_EXIT: std::sync::LazyLock<tokio::sync::Notify> =
    std::sync::LazyLock::new(tokio::sync::Notify::new);

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    match ctrl_type {
        0 | 2 | 5 | 6 => {
            CONSOLE_EXIT.notify_one();
            // CTRL_C：TRUE 抑制默认终止，退出交给我们自己（擦净再走）。
            // CLOSE/LOGOFF/SHUTDOWN：handler 返回后 OS 仍会终止进程——
            // 毫秒级擦拭与数秒宽限的赛跑，README_AGENT P21 竞态注记的已知边界。
            1
        }
        _ => 0,
    }
}

#[cfg(windows)]
fn install_console_ctrl_handler() {
    unsafe {
        // 失败静默（尽力而为，同 harden_process 家族语义）
        let _ = win::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
    }
}

/// 悄悄读一行（无回显）——门禁钥匙的耳语式输入（P20-E②，
/// kept.guard_whisper_input）。stdin 不是终端（管道/重定向）时，
/// 关回显失败属正常：照常读（键盘输入的场景里回显由终端负责）。
#[cfg(unix)]
fn read_line_hidden(prompt: &str) -> std::io::Result<String> {
    use std::io::{BufRead, Write};
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut saved: Option<libc::termios> = None;
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut t) == 0 {
            saved = Some(t);
            let mut t2 = t;
            t2.c_lflag &= !libc::ECHO;
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &t2);
        }
    }
    let mut line = String::new();
    let res = std::io::stdin().lock().read_line(&mut line);
    unsafe {
        if let Some(t) = saved {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &t);
        }
    }
    eprintln!(); // 回显被吞了的那个换行补上
    res.map(|_| line.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(windows)]
fn read_line_hidden(prompt: &str) -> std::io::Result<String> {
    use std::io::{BufRead, Write};
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    unsafe {
        let h = win::GetStdHandle(win::STD_INPUT_HANDLE);
        let mut mode: u32 = 0;
        let had = h.is_null() == false && win::GetConsoleMode(h, &mut mode) != 0;
        if had {
            win::SetConsoleMode(h, mode & !win::ENABLE_ECHO_INPUT);
        }
        let mut line = String::new();
        let res = std::io::stdin().lock().read_line(&mut line);
        if had {
            win::SetConsoleMode(h, mode);
        }
        eprintln!();
        res.map(|_| line.trim_end_matches(['\r', '\n']).to_string())
    }
}

#[cfg(all(unix, test))]
mod harden_tests {
    use super::*;

    /// P7（P-10 结构解）：harden_process 之后 TracerPid 必须为 0；
    /// Dumpable 字段（若本内核 status 暴露——部分定制内核无此行）
    /// 必须为 0。prctl 调用返回值在 harden_process 内 debug 断言。
    #[test]
    fn dumpable_is_zero_after_harden() {
        harden_process();
        let status = std::fs::read_to_string("/proc/self/status").expect("linux 有 status");
        if let Some(l) = status.lines().find(|l| l.starts_with("Dumpable:")) {
            let v = l.split_whitespace().nth(1).unwrap_or("?").to_string();
            assert_eq!(v, "0", "PR_SET_DUMPABLE=0 未生效：{l}");
        }
        let tracer = status
            .lines()
            .find(|l| l.starts_with("TracerPid:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("?")
            .to_string();
        assert_eq!(tracer, "0", "TracerPid 必须为 0（无人附着）");
    }
}

/// P7 courier tag 三层规则（rulings punch_tag_tiered +
/// 2026_10_17_p7_spec_lite_approved）：省略标签 → 自动 128bit
///（Crockford base32 分组上屏，转告给介绍人即可）；显式标签照用。
fn crockford_tag() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // 无 I L O U
    let mut raw = [0u8; 16]; // 128bit
    let _ = getrandom::fill(&mut raw);
    let mut acc: u64 = 0;
    let mut bits = 0u32;
    let mut chars = [0u8; 26]; // 128bit → 25×5bit + 3bit（尾组补零）
    let mut n = 0;
    for b in raw {
        acc = (acc << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            chars[n] = ((acc >> bits) & 31) as u8;
            n += 1;
        }
    }
    chars[n] = ((acc << (5 - bits)) & 31) as u8; // 尾组高位补零
    let s: String = chars.iter().map(|&v| ALPHABET[v as usize] as char).collect();
    format!(
        "{}-{}-{}-{}-{}-{}",
        &s[0..5], &s[5..10], &s[10..15], &s[15..20], &s[20..25], &s[25..26]
    )
}

/// punch 标签的三层裁决出口：自动生成（省略层）或照用（显式层）+
/// 短标签响亮警告（tag 猜测无需受害者在场——向介绍人枚举注册即可，
/// 降级必须知情）。
fn punch_tag_or_warn(explicit: Option<String>) -> String {
    match explicit {
        Some(t) => {
            if t.chars().count() < 16 {
                eprintln!(
                    "警告：标签「{t}」短于 16 字符 = 降级使用。标签猜测无需你\
                     在场——第三方向介绍人枚举注册即可冒配对；PAKE 暗号才是\
                     在场补偿。降级是你的自由，风险你知情。"
                );
            }
            t
        }
        None => {
            let t = crockford_tag();
            eprintln!("自动标签（128bit）：{t}——请经介绍人转告另一位。");
            t
        }
    }
}

/// 场内指令。返回 true 表示要离场。
async fn handle_command(cmd: &str, person: &Person) -> Result<bool, Box<dyn std::error::Error>> {
    let mut parts = cmd.split_whitespace();
    match parts.next().unwrap_or("") {
        "dial" => {
            let Some(addr) = parts.next() else {
                eprintln!("/dial 需要对方地址，例如：/dial 127.0.0.1:7777 暗号");
                return Ok(false);
            };
            eprintln!("去找 {addr} …");
            // 显式带暗号走 dial_secret；不带则走 dial——链上的下一把自动生效
            let r = match parts.next() {
                Some(s) => person.dial_secret(addr, Some(s.as_bytes())).await,
                None => person.dial(addr).await,
            };
            if let Err(e) = r {
                eprintln!("{e}");
            }
        }
        "meet" => {
            let (Some(courier), Some(room)) = (parts.next(), parts.next()) else {
                eprintln!("/meet 需要茶馆地址和房号，例如：/meet 203.0.113.9:8888 1001 暗号");
                return Ok(false);
            };
            let Ok(room) = room.parse::<u32>() else {
                eprintln!("房号应是数字。");
                return Ok(false);
            };
            let secret = parts.next();
            eprintln!("进茶馆 {courier} 房间 {room}，等另一位…");
            let r = match secret {
                Some(s) => {
                    person
                        .meet_at_teahouse_secret(courier, room, Some(s.as_bytes()))
                        .await
                }
                None => person.meet_at_teahouse(courier, room).await,
            };
            match r {
                Ok(_) => {}
                Err(e) => eprintln!("{e}"),
            }
        }
        "punch" => {
            // /punch <介绍人地址> [标签] [暗号]——P7 三层规则（裁决②）：
            // 省略标签 = 自动 128bit（Crockford 分组上屏，经介绍人转告）；
            // 显式短标签 = 响亮警告照用（降级自由，PAKE 补偿关系写明）。
            let (Some(addr), tag) = (parts.next(), parts.next()) else {
                eprintln!("/punch 需要介绍人地址；标签可省略（自动生成 128bit），例如：/punch 203.0.113.9:8888 huo-guo 暗号");
                return Ok(false);
            };
            let secret = parts.next();
            let tag = punch_tag_or_warn(tag.map(|s| s.to_string()));
            eprintln!("向 {addr} 报名「{tag}」，等另一头的人…");
            let r = match secret {
                Some(s) => person.punch_secret(addr, &tag, Some(s.as_bytes())).await,
                None => person.punch(addr, &tag).await,
            };
            if let Err(e) = r {
                eprintln!("{e}");
            }
        }
        "await" => {
            let Some(secret) = parts.next() else {
                eprintln!("/await 需要一句暗号，例如：/await 蓝铜七号");
                return Ok(false);
            };
            person.expect_secret(secret.as_bytes()).await;
            // 只确认已备好，不回显暗号本体——它不该出现在任何输出流里
            eprintln!("切口备好了：下一位来客要对上这句暗号才谈得成。");
        }
        "talk" => {
            let Some(key) = parts.next() else {
                eprintln!("/talk 需要对话编号或名字，例如：/talk 1、/talk bob");
                return Ok(false);
            };
            let id: Option<u64> = if let Ok(id) = key.parse::<u64>() {
                Some(id) // 对不对，交给库说了算
            } else {
                let lower = key.to_lowercase();
                let roster = person.roster().await;
                let hits: Vec<_> = roster
                    .iter()
                    .filter(|r| r.name.to_lowercase().starts_with(&lower))
                    .collect();
                match hits.len() {
                    0 => {
                        eprintln!("没有这样的对话。/list 看看在场的人。");
                        return Ok(false);
                    }
                    1 => Some(hits[0].id),
                    _ => {
                        let list = hits
                            .iter()
                            .map(|r| format!("#{} {}", r.id, r.name))
                            .collect::<Vec<_>>()
                            .join("、");
                        eprintln!("不止一个人叫这个：{list}。用编号 /talk N。");
                        return Ok(false);
                    }
                }
            };
            if let Some(id) = id {
                match person.talk_to(id).await {
                    Ok(n) => eprintln!("现在跟 [{n}]（#{id}）说话。"),
                    Err(e) => eprintln!("{e}"),
                }
            }
        }
        "list" => {
            let roster = person.roster().await;
            if roster.is_empty() {
                eprintln!("四下无人。");
                return Ok(false);
            }
            for r in roster {
                let mark = if r.focused {
                    "  ← 此刻在跟 TA 说话"
                } else {
                    ""
                };
                eprintln!("#{} {}{}", r.id, r.name, mark);
            }
        }
        "card" => {
            let lines = person.card().await;
            if lines.is_empty() {
                eprintln!("（一张白板：没找到可用的地址）");
            }
            for l in lines {
                eprintln!("{l}");
            }
        }
        "circle" => {
            // /circle [封桌|开台] [名1 名2 …]：席位可显式覆盖；给名字 =
            // 选择性开圈（P6 subset form——誓言集=这份名单，唯一精确
            // 匹配，歧义/无匹配响亮报错）；空参=全部快照（点名语义）。
            let mut seats: Option<u8> = None;
            let mut names: Vec<String> = Vec::new();
            for a in parts {
                match a {
                    "封桌" | "封" | "closed" => seats = Some(e2ee::person::SEATS_CLOSED),
                    "开台" | "开" | "stage" => seats = Some(e2ee::person::SEATS_OPEN),
                    other => names.push(other.to_string()),
                }
            }
            let r = if names.is_empty() {
                person.form_circle_seats(seats).await
            } else {
                person.form_circle_subset(&names, seats).await
            };
            match r {
                Ok(n) => eprintln!(
                    "开台了：邀请名单 {n} 人（名单上的名字自署到齐后自动开讲）。\
                     在圈中重跑 = 点名重聚（新圈号重新围坐）。{}",
                    if names.is_empty() {
                        "".to_string()
                    } else {
                        format!("（选择性开圈：{:?}——旧桌保留，新桌为当前座位）", names)
                    }
                ),
                Err(e) => eprintln!("{e}"),
            }
        }
        "gsay" => {
            let text = cmd.strip_prefix("gsay").unwrap_or("").trim().to_string();
            if text.is_empty() {
                eprintln!("/gsay 需要一句话，例如：/gsay 今晚吃火锅");
                return Ok(false);
            }
            if let Err(e) = person.circle_speak(&text).await {
                eprintln!("{e}");
            }
        }
        "bring" => {
            // /bring [拟名]：聚焦对端即候选人
            let expected = parts.next().map(|s| s.to_string());
            match person.bring(expected.as_deref()).await {
                Ok(seats) => {
                    if seats == e2ee::person::SEATS_CLOSED {
                        eprintln!(
                            "注意：这是封桌。看座照发，但圈子会响亮拒收这次入席\
                             （席位契约）——你将与圈子分叉（互相 mumble），\
                             直到点名重聚裁决。"
                        );
                    } else {
                        eprintln!("看座递出去了——对方 /sit 后入座自动完成。");
                    }
                }
                Err(e) => eprintln!("{e}"),
            }
        }
        "sit" => {
            // /sit [名字]：落座（默认本人自报名，可覆盖）
            let name_override = parts.next().map(|s| s.to_string());
            match person.sit(name_override.as_deref()).await {
                Ok(()) => eprintln!("落座署名递回去了——等引荐人的座次表。"),
                Err(e) => eprintln!("{e}"),
            }
        }
        "word" => {
            // /word：SAS 对词——即时显示、零落盘；不对账是用户的自由
            match person.sas_word().await {
                Ok(rep) => {
                    if let Some((with, word)) = rep.p2p {
                        eprintln!("和 [{with}] 的会话短词：{word}");
                    }
                    if let Some((members, word)) = rep.circle {
                        eprintln!("圈内群短词（{members} 人）：{word}");
                    }
                    eprintln!(
                        "对词：两边各念各的短词，一样 = 这条线上没人拼话；\
                         不一样 = 有人把线拼成了两半（P-08）。"
                    );
                }
                Err(e) => eprintln!("{e}"),
            }
        }
        "file" => {
            // 路径取整行余文（可含空格）；读盘是 CLI 这个使用者的事——库只收字节
            let path = cmd.strip_prefix("file").unwrap_or("").trim();
            if path.is_empty() {
                eprintln!("/file 需要文件路径，例如：/file ./tu-pian.png");
                return Ok(false);
            }
            let name = std::path::Path::new(path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            // P20-F 流式：边读边发（file_begin / file_chunk / file_end）——
            // 内存里同时只有一块，不再整读 32MB。块边界随机化在库内
            //（file_chunk 再切片），CLI 只管 16KB 地喂。
            {
                use tokio::io::AsyncReadExt;
                let outcome = async {
                    let mut f = tokio::fs::File::open(path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let total = f
                        .metadata()
                        .await
                        .map_err(|e| e.to_string())?
                        .len();
                    person
                        .file_begin(&name, total)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut buf = vec![0u8; 16 * 1024];
                    loop {
                        let n = f.read(&mut buf).await.map_err(|e| e.to_string())?;
                        if n == 0 {
                            break;
                        }
                        person
                            .file_chunk(&buf[..n])
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    person.file_end().await.map_err(|e| e.to_string())
                }
                .await;
                match outcome {
                    Ok(()) => eprintln!("文件〔{}〕递出去了。", safe(&name)),
                    Err(msg) => {
                        let _ = person.file_abort().await;
                        eprintln!("{msg}");
                    }
                }
            }
        }
        "bye" => match person.bye().await {
            Ok(n) => eprintln!("跟 [{n}] 道别了。"),
            Err(e) => eprintln!("{e}"),
        },
        // /quit 已废除（rulings.2026_10_31_exit_taxonomy）：退出面只剩
        // Ctrl-C（立即擦净、不告别）；告别归 /bye。
        other => {
            eprintln!(
                "不认识的指令：/{other}。可用：/dial /meet /punch /await /talk /list /card \
                 /circle /gsay /bring /sit /word /file /bye。退出：Ctrl-C（立即擦净，不告别）"
            )
        }
    }
    Ok(false)
}

/// 收端在途文件的落盘账本（P20-F 流式）：对话编号 →（句柄, 目标路径）。
/// CLI 是外层使用者——字节到哪由它决定（库只交块）；逐块落盘，
/// 内存里不同时存在整份文件。
static INBOUND_FILES: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u64, (std::fs::File, std::path::PathBuf)>>,
> = std::sync::OnceLock::new();

/// 账本的单例取用（HashMap::new 非 const，OnceLock 惰性立）
fn inbound_files() -> &'static std::sync::Mutex<
    std::collections::HashMap<u64, (std::fs::File, std::path::PathBuf)>,
> {
    INBOUND_FILES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 在 ./received/ 开一个新文件（名字过安检：控制字符中和、路径分隔剥掉、
/// 只剩点儿的当没名字、重名加序）。开不出来 = 大声死：收文件这条命就靠盘。
fn open_received_file(name: &str) -> (std::fs::File, std::path::PathBuf) {
    let cleaned = safe(name).replace(['/', '\\'], "·");
    let cleaned = if cleaned.trim_matches(['.', ' ']).is_empty() {
        "file".to_string()
    } else {
        cleaned
    };
    let _ = std::fs::create_dir_all("received");
    let mut dest = std::path::Path::new("received").join(&cleaned);
    let mut n = 1;
    while dest.exists() {
        n += 1;
        dest = std::path::Path::new("received").join(format!("{n}-{cleaned}"));
    }
    let f = std::fs::File::create(&dest)
        .unwrap_or_else(|e| panic!("在 {} 落不了盘：{e}——收不了文件", dest.display()));
    (f, dest)
}

/// 把一句话说成终端里的人话。
/// 远程可控的名字与文本先过这里：控制字符（含 ESC 转义序列）一律中和，
/// 免得对端一句话擦掉你半个屏幕、改你终端标题。
fn safe(s: &str) -> std::borrow::Cow<'_, str> {
    if s.chars().any(|c| c.is_control()) {
        s.chars()
            .map(|c| if c.is_control() { '·' } else { c })
            .collect::<String>()
            .into()
    } else {
        s.into()
    }
}

/// 把一个人的经历说成终端里的人话。
fn render(ev: Event) {
    match ev {
        Event::Knocked { peer } => eprintln!("{}", safe(&peer)),
        Event::Met { id, name, focused } => {
            if focused {
                eprintln!("与 [{}] 的对话 #{id} 开始。", safe(&name));
            } else {
                eprintln!("[{}] 找上门来，对话 #{id} 开始（/talk {id} 切换过去）。", safe(&name));
            }
        }
        Event::MeetFailed { error } => eprintln!("{}", safe(&error)),
        Event::Heard { name, text, .. } => println!("[{}] {}", safe(&name), safe(&text)),
        Event::Left { name, reason, .. } => match reason {
            LeaveReason::Farewell => eprintln!("[{}] 挂断了。", safe(&name)),
            LeaveReason::Disconnected => eprintln!("[{}] 断开了。", safe(&name)),
            LeaveReason::Undecipherable => {
                eprintln!("[{}] 的话解不开，这条信道有问题。", safe(&name))
            }
            LeaveReason::Wire(e) => eprintln!("信道出错：{}", safe(&e)),
            _ => {} // 库将来新增的结局，终端暂时不渲染
        },
        Event::FocusReturned { to } => {
            if let Some(id) = to {
                eprintln!("注意力回到 #{id}。");
            }
        }
        Event::KeyLockDegraded => {
            eprintln!("警告：密钥内存锁定降级——本次设置的暗号/门禁钥匙未能锁进物理内存（可能被换出到磁盘）。程序照常运行，擦拭语义不变；如需关闭，请检查系统的内存锁定限额（ulimit -l）。");
        }
        Event::CircleFormed { circle_id, members } => {
            eprintln!(
                "圈子铸成了（圈 {circle_id:x}）：名单 {members} 人全部自署到场，屋里的话开讲（/gsay）。"
            );
        }
        Event::CircleMumble { circle_id, from } => {
            eprintln!(
                "[{}] 转来一句圈 {circle_id:x} 里解不开的话（响亮分歧：你们的信念分岔了）——\
                 修复路径永远是点名：重跑 /circle。",
                safe(&from)
            );
        }
        Event::CircleRekeyed { circle_id, members, change } => {
            eprintln!("圈 {circle_id:x} 换了新钥匙（{change}），现在 {members} 人在座。");
        }
        Event::CircleRejected { circle_id, detail } => {
            eprintln!("圈 {circle_id:x} 响亮拒绝：{}", safe(&detail));
        }
        Event::CircleConflict { circle_id, detail } => {
            eprintln!("圈 {circle_id:x} 有冲突（人类裁决）：{}", safe(&detail));
        }
        Event::SeatOffered { from, table } => {
            eprintln!(
                "[{}] 给你看座：桌上有 {}。想坐就 /sit（想用别的名字就 /sit 名字）。",
                safe(&from),
                table.iter().map(|n| safe(n).into_owned()).collect::<Vec<_>>().join("、")
            );
        }
        Event::FileChunk { id, name, data, .. } => {
            // P20-F 流式：序号校验已在库内（对不上=FileAborted），CLI 顺序落盘。
            // 库永不落盘；CLI 是外层使用者，替人把东西放进 ./received/。
            use std::io::Write;
            let mut map = inbound_files().lock().expect("在途文件账本没被毒化");
            let entry = map
                .entry(id)
                .or_insert_with(|| open_received_file(&name));
            if let Err(e) = entry.0.write_all(&data) {
                eprintln!("写盘失败：{e}——文件〔{}〕收不全了。", safe(&name));
                map.remove(&id);
            }
        }
        Event::FileComplete { id, name, total } => {
            // 收齐：句柄随条目移除而关闭（空文件在此创建）
            let mut map = inbound_files().lock().expect("在途文件账本没被毒化");
            let dest = map
                .entry(id)
                .or_insert_with(|| open_received_file(&name))
                .1
                .clone();
            map.remove(&id);
            eprintln!("收到文件〔{}〕（{total} 字节），放在 {}。", safe(&name), dest.display());
        }
        Event::FileAborted { id, detail } => {
            // 作废：半份文件没意义，删掉（P20-F loud-failure 的 CLI 面）
            let mut map = inbound_files().lock().expect("在途文件账本没被毒化");
            if let Some((_f, dest)) = map.remove(&id) {
                let _ = std::fs::remove_file(&dest);
                eprintln!("文件作废（{detail}）——半份已删。");
            } else {
                eprintln!("文件作废：{detail}。");
            }
        }
        Event::FileDeclined { from, name, size } => {
            eprintln!(
                "[{}] 想递一份超大的文件〔{}〕（{size} 字节，超过 32MB 上限），婉拒了。",
                safe(&from),
                safe(&name)
            );
        }
        _ => {} // 库将来新增的经历，终端暂时不渲染
    }
}
