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
    let mut positional: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "help" | "--help" | "-h" => {
                println!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "--courier" => as_courier = true,
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
    let (person, mut events) = Person::born(&name, &listen_addr).await?;
    eprintln!("我是 {name}，在听 {listen_addr}。等人来，或 /dial 找人（/quit 离场）");

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
            eprintln!("切口备好了：下一位来客要对上「{secret}」才谈得成。");
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
        _ => {} // 库将来新增的经历，终端暂时不渲染
    }
}
