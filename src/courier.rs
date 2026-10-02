//! 茶馆：同一个二进制的另一份工。
//!
//! 部署在公网机器上，只做一件事：把两个报出同一房号的人接在一起。
//! 它自始至终只见乱码——握手与密谈在它两眼之间端到端进行，
//! 它没有钥匙，也永远拿不到。无身份 · 无存储 · 无明文。
//! 掌柜还兼做介绍人（同门牌的 UDP 半边）：只递地址，不传话。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

use crate::punch::{INTRO1, INTRO2, INTRO_TTL};
use crate::wire::{read_frame, write_frame};

/// 一个人进房后，等另一位的最长时间。等不到就散。
const WAIT_FOR_PAIR: Duration = Duration::from_secs(60);

/// 连上后报房号的时限：占着门厅不吭声的，等不得——
/// 不然一个恶意的空连接就能把整个门厅堵死。
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);

/// 介绍人那边同时等位的标签上限：防人灌一堆一次性标签把记性撑爆。
const MAX_WAITING_TAGS: usize = 1024;

/// 房间到期仍无人落座后的宽限。
const ROOM_LINGER: Duration = Duration::from_secs(5);

/// 开一间茶馆：认房号、接线、忘掉（房号由客人带外自选）。掌柜兼做介绍人（同门牌 UDP）。
/// 返回值只在占不到端口时出错。
pub async fn serve(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    // 同门牌的另一半：介绍人。UDP 与 TCP 各占各的号位，同号不冲突。
    let port = listener.local_addr()?.port();
    match UdpSocket::bind(("0.0.0.0", port)).await {
        Ok(udp) => {
            tokio::spawn(serve_introducer(udp));
        }
        Err(e) => {
            eprintln!("介绍人开不了张（UDP {port} 被占）：{e}；茶馆照常。");
        }
    }
    eprintln!("茶馆开张 {addr} —— 只接线，不识字，不留客。掌柜兼做介绍人（同门牌 UDP）。");

    // 等人的房间：房号 → 想搭线的那一端。
    // 房号由客人自选（双方带外约定，与暗号同路数）——茶馆只认号接线。
    let waiting: Arc<Mutex<HashMap<u32, TcpStream>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, peer) = listener.accept().await?;
        let waiting = waiting.clone();
        // 每位客人单独招呼：一位慢吞吞，不耽误下一位进门。
        // 报房号也限时——不然一个连上不吭声的就能把整个门厅堵死。
        tokio::spawn(async move {
            let mut stream = stream;
            let joined = tokio::time::timeout(JOIN_TIMEOUT, read_room(&mut stream)).await;
            let Ok(Ok(Ok(room))) = joined else {
                eprintln!("{peer} 连上却不报房号，请回。");
                return;
            };
            match check_in(room, stream, &waiting).await {
                PairOutcome::Paired => eprintln!("{peer} 入住房 {room}，接上了。"),
                PairOutcome::Vacated => eprintln!("{peer} 住过房 {room}，另一端没来，散了。"),
                PairOutcome::Broken => eprintln!("{peer} 的房 {room} 出了岔子。"),
            }
        });
    }
}

/// 介绍人：听报名（`INTRO1 标签`），凑成一对就互递对方的公网地址，然后忘掉。
/// 同一个人重复报名算刷新，不算配对；过期未成对的报名按时清位。
/// 只递地址，不传话——谈话永远不经它手。
async fn serve_introducer(udp: UdpSocket) {
    let mut waiting: HashMap<String, (SocketAddr, Instant)> = HashMap::new();
    let mut buf = [0u8; 256];
    loop {
        let Ok((n, from)) = udp.recv_from(&mut buf).await else {
            continue;
        };
        // 顺手扫掉过期的报名
        waiting.retain(|_, (_, t)| t.elapsed() < INTRO_TTL);
        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
        let Some(tag) = text.strip_prefix(&format!("{INTRO1} ")) else {
            continue;
        };
        if tag.is_empty() {
            continue;
        }
        // 同一个人再报一次：刷新等位时间，不算配对
        if waiting.get(tag).is_some_and(|(a, _)| *a == from) {
            if let Some((_, t)) = waiting.get_mut(tag) {
                *t = Instant::now();
            }
            continue;
        }
        match waiting.remove(tag) {
            // 凑成一对：先报名者当应答方，后报名者当发起方（与茶馆分主客一致）
            Some((first, _)) => {
                let _ = udp.send_to(format!("{INTRO2} R {from}").as_bytes(), first).await;
                let _ = udp.send_to(format!("{INTRO2} I {first}").as_bytes(), from).await;
            }
            None => {
                // 等位簿有上限：灌一次性标签的洪水到此为止
                if waiting.len() < MAX_WAITING_TAGS {
                    waiting.insert(tag.to_string(), (from, Instant::now()));
                }
            }
        }
    }
}

/// 进房的结果。
enum PairOutcome {
    /// 两位到齐，线接上了
    Paired,
    /// 等了太久，没等到另一位
    Vacated,
    /// 途中有闪失
    Broken,
}

/// 报房号：进门的头一句话是「JOIN 房号」。
async fn read_room(stream: &mut TcpStream) -> std::io::Result<std::io::Result<u32>> {
    let Some(frame) = read_frame(stream).await? else {
        return Ok(Err(std::io::Error::other("没开口就走了")));
    };
    let text = String::from_utf8_lossy(&frame);
    let Some(num) = text.strip_prefix("JOIN ") else {
        return Ok(Err(std::io::Error::other("不认识的开场白")));
    };
    match num.trim().parse::<u32>() {
        Ok(room) => Ok(Ok(room)),
        Err(_) => Ok(Err(std::io::Error::other("房号念得不对"))),
    }
}

/// 落座：第一位先到，登记在房里等；第二位一到，接线。
async fn check_in(
    room: u32,
    mut newcomer: TcpStream,
    waiting: &Arc<Mutex<HashMap<u32, TcpStream>>>,
) -> PairOutcome {
    let first = {
        let mut rooms = waiting.lock().await;
        rooms.remove(&room) // 有等着的就取出来；没有就自己等
    };

    let (a, b) = match first {
        Some(mut first) => {
            // 两位到齐。告知角色：先来的当应答方，后到的当发起方
            // （Noise 握手必须一先一后，茶馆替他们分好主客）
            let ok = write_frame(&mut first, b"PAIRED R").await.is_ok()
                && write_frame(&mut newcomer, b"PAIRED I").await.is_ok();
            if !ok {
                return PairOutcome::Broken;
            }
            (first, newcomer)
        }
        None => {
            // 自己是第一位：登记入房，等下一位
            {
                let mut rooms = waiting.lock().await;
                // 同号房已有人在等：两对客人撞了号（房号自选，挑得像暗号
                // 一样难猜就不会撞）。稳妥起见拒绝重复等位，免得接错线。
                if rooms.contains_key(&room) {
                    return PairOutcome::Broken;
                }
                rooms.insert(room, newcomer);
            }
            // 等人这口气最长 WAIT_FOR_PAIR；等到就接，等不到就撤
            return match tokio::time::timeout(WAIT_FOR_PAIR, async {
                loop {
                    // 睡一小段，醒来查房：第二位会把我取走
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if !waiting.lock().await.contains_key(&room) {
                        return;
                    }
                }
            })
                .await
            {
                Ok(()) => PairOutcome::Paired, // 被接走了
                Err(_) => {
                    // 没等到人：把自己撤下来，请回
                    let mut rooms = waiting.lock().await;
                    if let Some(mut me) = rooms.remove(&room) {
                        let _ = me.shutdown().await;
                    }
                    PairOutcome::Vacated
                }
            };
        }
    };

    // 接线：把两端的字节原样对倒，直到任何一端离席。
    splice(a, b).await
}

/// 接线：两个方向各一根管子，纯搬运，不识字。
async fn splice(a: TcpStream, b: TcpStream) -> PairOutcome {
    let (mut ar, mut aw) = a.into_split();
    let (mut br, mut bw) = b.into_split();

    let ab = async move {
        let mut buf = [0u8; 8192];
        loop {
            match ar.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if bw.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = bw.shutdown().await;
    };
    let ba = async move {
        let mut buf = [0u8; 8192];
        loop {
            match br.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if aw.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = aw.shutdown().await;
    };
    tokio::join!(ab, ba);
    PairOutcome::Paired // 一端离席、两端收摊，都算这一场圆满结束
}

/// 找茶馆：拨通、报房号、等到齐。返回接好的裸线，和这端该当的角色
/// （后到者为发起方，先到者为应答方）。
/// 这条线上随后跑的仍是 Noise——茶馆只是把它接了起来。
pub async fn enter(
    courier_addr: &str,
    room: u32,
) -> Result<(TcpStream, bool), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(courier_addr).await?;
    write_frame(&mut stream, format!("JOIN {room}").as_bytes()).await?;
    // 等到齐：茶馆回「PAIRED I/R」或断开
    let deadline = tokio::time::Instant::now() + WAIT_FOR_PAIR;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return Err("茶馆里等不到另一位。".into());
        }
        match tokio::time::timeout(left, read_frame(&mut stream)).await {
            Ok(Ok(Some(frame))) => {
                if frame == b"PAIRED I" {
                    return Ok((stream, true));
                }
                if frame == b"PAIRED R" {
                    return Ok((stream, false));
                }
                // 其他帧不该有，继续等
            }
            _ => return Err("茶馆里等不到另一位。".into()),
        }
    }
}

#[allow(dead_code)]
const _: () = {
    // ROOM_LINGER 暂时只作语义占位：房随线生，线断房灭，
    // 当前实现里撤房是即时的，无需宽限。
    let _ = ROOM_LINGER;
};
