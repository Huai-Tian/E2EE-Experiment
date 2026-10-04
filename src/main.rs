//! 第一号使用者：终端里的一个人。
//! 库负责语义——动作从方法进，经历从事件出；这里只把事件说成人话。

use e2ee::{Event, LeaveReason, Person};
use std::process::ExitCode;
use tokio::io::{AsyncBufReadExt, BufReader};

/// 耳朵默认守着的地址
const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:7777";

const HELP: &str = "\
E2EE-Experiment —— 一个像人一样的二进制：启动即在场，能听能说，转身即忘。

用法：
  E2EE-Experiment [监听地址] [-n 名字]
      出生。默认在 0.0.0.0:7777 上听，等人来，也随时可以主动去找人。
      同一台机器开两个人时，记得用不同端口。
  E2EE-Experiment --hidden [监听地址] [-n 名字]
      隐身出生：永不应答 /shout 广播探测——同屋喊一嗓子也探不到你在。
      拿确切地址 /dial、/meet、/punch 等精确交往一切照常（TCP 耳朵照常开）。
      注意：隐身者仍可主动 /shout 找别人，但喊这一嗓子会把自己的 IP 暴露给
      所有在听的人——真隐身连喊也不喊。
  E2EE-Experiment --no-chain [监听地址] [-n 名字]
      不续链出生：道别时不预约下一把暗号，也不接受别人的预约。
      适合对陌生人常开的台席（如意见反馈热线）——谁来都行，散场不留锁
      （默认的续链会把下一场留给上一位客人，陌生人反而进不来）。
      可与 --hidden 叠加。
  E2EE-Experiment --guard <机器暗号> [监听地址] [-n 名字]
      常备门禁出生：每位来客都要对上这句暗号才谈得成，且从不消耗——
      瞎拨多少次也锁不住台，真钥永远进得来。这句应是 32 字节级的
      随机机器密钥（不是人类暗号：一次性切口「烧掉」防的是对低熵暗号
      的在线爆破，机器密钥在线爆破本就不可能）。可与 --no-chain、
      --hidden 叠加；对陌生人常开的武装台席推荐 --guard KEY --no-chain。
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
  /shout               同屋喊一圈，看看谁在（局域网发现）
  /circle              围坐一圈：与当前在场的人凑话铸群钥匙（须先互连）
  /gsay <话>           在圈内说一句（加密一次，人人同时收到）
  /bye                 和当前对话的人道别
  /quit 或 Ctrl-D      和所有人道别，离场
  Ctrl-C               直接离场

这条线上只有你们两端能听懂彼此；对面是谁，由你自己判断。
无账户 · 无身份 · 无存储 —— 每次连接都是初次见面。";

#[tokio::main]
async fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut name = String::from("无名氏");
    let mut listen_addr = DEFAULT_LISTEN_ADDR.to_string();
    let mut as_courier = false;
    let mut hidden = false;
    let mut no_chain = false;
    let mut guard_secret: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "help" | "--help" | "-h" => {
                println!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "--courier" => as_courier = true,
            "--hidden" => hidden = true,
            "--no-chain" => no_chain = true,
            "--guard" => match args.next() {
                Some(g) => guard_secret = Some(g),
                None => {
                    eprintln!("--guard 后面要跟机器暗号（建议 32 字节随机串）\n");
                    eprintln!("{HELP}");
                    return Ok(ExitCode::from(2));
                }
            },
            "-n" | "--name" => match args.next() {
                Some(n) => name = n,
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
            _ => positional.push(arg),
        }
    }
    if let Some(addr) = positional.into_iter().next() {
        listen_addr = addr;
    }

    // 另一份工：开茶馆，永不下班
    if as_courier {
        e2ee::courier::serve(&listen_addr).await?;
        return Ok(ExitCode::SUCCESS);
    }

    // 出生；此后一半心思听事件，一半心思读输入。
    // 隐身出生的人不应答同屋喊话（广播探不到存在），其余照常。
    let (person, mut events) = if hidden {
        Person::born_hidden(&name, &listen_addr).await?
    } else {
        Person::born(&name, &listen_addr).await?
    };
    // 不续链：道别不预约、来约不接受（开放台席形态）
    if no_chain {
        person.set_chaining(false).await;
    }
    // 常备门禁：每位来客都要对上这句机器暗号，从不消耗
    if let Some(g) = &guard_secret {
        person.expect_secret_stable(g.as_bytes()).await;
    }
    let mut notes = String::new();
    if hidden {
        notes.push_str("（隐身：不应答 /shout）");
    }
    if no_chain {
        notes.push_str("（不续链：道别不预约，谁来都行）");
    }
    if guard_secret.is_some() {
        notes.push_str("（常备门禁：每位来客都要对上暗号）");
    }
    eprintln!("我是 {name}，在听 {listen_addr}{notes}。等人来，或 /dial 找人（/quit 离场）");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { break };
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
        }
    }

    // 语尽（Ctrl-D / /quit）：和所有人道别，体面离场。
    person.leave().await;
    Ok(ExitCode::SUCCESS)
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
            let (Some(addr), Some(tag)) = (parts.next(), parts.next()) else {
                eprintln!("/punch 需要介绍人地址和标签，例如：/punch 203.0.113.9:8888 huo-guo 暗号");
                return Ok(false);
            };
            let secret = parts.next();
            eprintln!("向 {addr} 报名「{tag}」，等另一头的人…");
            let r = match secret {
                Some(s) => person.punch_secret(addr, tag, Some(s.as_bytes())).await,
                None => person.punch(addr, tag).await,
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
        "shout" => {
            eprintln!("喊一嗓子…");
            let found = person.shout().await;
            if found.is_empty() {
                eprintln!("屋里没人应。");
            } else {
                for d in found {
                    // 应答里的名字是局域网里任何人都能伪造的，照例先消毒
                    eprintln!("{}  {}", safe(&d.name), d.dial_addr);
                }
            }
        }
        "circle" => match person.form_circle().await {
            Ok(n) => eprintln!("凑话发出去了，{n} 人一圈（收齐自动开讲）。"),
            Err(e) => eprintln!("{e}"),
        },
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
            match tokio::fs::read(path).await {
                Ok(data) => {
                    let size = data.len();
                    match person.send_file(&name, &data).await {
                        Ok(()) => eprintln!("文件〔{}〕递出去了（{size} 字节）。", safe(&name)),
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Err(e) => eprintln!("读不了 {path}：{e}"),
            }
        }
        "bye" => match person.bye().await {
            Ok(n) => eprintln!("跟 [{n}] 道别了。"),
            Err(e) => eprintln!("{e}"),
        },
        "quit" => return Ok(true),
        other => {
            eprintln!("不认识的指令：/{other}。可用：/dial /talk /list /bye /quit")
        }
    }
    Ok(false)
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
        Event::SecretChained { with } => {
            eprintln!("和 [{}] 的暗号链续上了：下一场免念。", safe(&with));
        }
        Event::CircleFormed { members } => {
            eprintln!("圈子铸成了：{members} 人，屋里的话开讲（/gsay）。");
        }
        Event::CircleMumble { from } => {
            eprintln!("[{}] 说了句圈里解不开的话（密钥不合？密文被动过？）——通常该重新 /circle。", safe(&from));
        }
        Event::FileArrived { name, data, .. } => {
            // 库永不落盘；CLI 是外层使用者，替人把东西放进 ./received/。
            // 名字过安检：控制字符中和、路径分隔剥掉、只剩点儿的当没名字、重名加序。
            let cleaned = safe(&name).replace(['/', '\\'], "·");
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
            match std::fs::write(&dest, &data) {
                Ok(()) => eprintln!(
                    "收到文件〔{}〕（{} 字节），放在 {}。",
                    cleaned,
                    data.len(),
                    dest.display()
                ),
                Err(e) => eprintln!(
                    "收到文件〔{cleaned}〕但写不进磁盘：{e}（{} 字节随进程离去）",
                    data.len()
                ),
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
