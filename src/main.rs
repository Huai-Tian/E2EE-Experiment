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

在场指令：
  /dial <ADDR:PORT>    主动去找人（找上门即成为当前对话）
  /talk <编号|名字>     把注意力切到另一场对话
  /list                看看在场的人
  /bye                 和当前对话的人道别
  /quit 或 Ctrl-D      和所有人道别，离场
  Ctrl-C               直接离场

这条线上只有你们两端能听懂彼此；对面是谁，由你自己判断。
无账户 · 无身份 · 无存储 —— 每次连接都是初次见面。";

#[tokio::main]
async fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut name = String::from("无名氏");
    let mut listen_addr = DEFAULT_LISTEN_ADDR.to_string();
    let mut positional: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "help" | "--help" | "-h" => {
                println!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
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
                eprintln!("/dial 需要对方地址，例如：/dial 127.0.0.1:7777");
                return Ok(false);
            };
            eprintln!("去找 {addr} …");
            if let Err(e) = person.dial(addr).await {
                eprintln!("{e}");
            }
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

/// 把一个人的经历说成终端里的人话。
fn render(ev: Event) {
    match ev {
        Event::Knocked { peer } => eprintln!("{peer} 来了。"),
        Event::Met { id, name, focused } => {
            if focused {
                eprintln!("与 [{name}] 的对话 #{id} 开始。");
            } else {
                eprintln!("[{name}] 找上门来，对话 #{id} 开始（/talk {id} 切换过去）。");
            }
        }
        Event::MeetFailed { error } => eprintln!("{error}"),
        Event::Heard { name, text, .. } => println!("[{name}] {text}"),
        Event::Left { name, reason, .. } => match reason {
            LeaveReason::Farewell => eprintln!("[{name}] 挂断了。"),
            LeaveReason::Disconnected => eprintln!("[{name}] 断开了。"),
            LeaveReason::Undecipherable => eprintln!("[{name}] 的话解不开，这条信道有问题。"),
            LeaveReason::Wire(e) => eprintln!("信道出错：{e}"),
            _ => {} // 库将来新增的结局，终端暂时不渲染
        },
        Event::FocusReturned { to } => {
            if let Some(id) = to {
                eprintln!("注意力回到 #{id}。");
            }
        }
        _ => {} // 库将来新增的经历，终端暂时不渲染
    }
}
