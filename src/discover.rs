//! 同屋喊一嗓子与名片：局域网里的「在场」与「找得到」。
//!
//! 这条 UDP 侧信道只携带「名字＋地址」，永不携带谈话内容，也永不携带任何秘密。
//! 喊话是主动查询而非常开信标：没人应，就当屋里没人。

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::time::timeout;

/// 喊话端口：同屋的人守在这里听「有人吗」。
pub const SHOUT_PORT: u16 = 37777;

/// 探子的暗记（版本化：将来格式变了，好分新旧）。
const PROBE: &[u8] = b"E2EEPROBE1";
/// 应答的暗记头。
const REPLY_HEAD: &str = "E2EEREPLY1";

/// 喊一圈后发现的一个在场的人。
pub struct Discovered {
    pub name: String,
    pub dial_addr: String,
}

/// 本机所有网卡的地址（名片用，也用来认出自己的回声）。
pub fn local_ips() -> Vec<IpAddr> {
    let mut ips: Vec<IpAddr> = if_addrs::get_if_addrs()
        .map(|ifs| ifs.into_iter().map(|i| i.ip()).collect())
        .unwrap_or_default();
    ips.sort_unstable();
    ips.dedup();
    ips
}

/// 这个人的名片：每个网卡一行「名字  地址」。
pub fn card(name: &str, tcp_port: u16) -> Vec<String> {
    local_ips()
        .into_iter()
        .map(|ip| format!("{name}  {ip}:{tcp_port}"))
        .collect()
}

/// 守着喊话端口过日子：听到「有人吗」，应一声「名字＋地址」。
/// 纯反射，不惊动这个人的意识，也不产生事件。
/// 端口被占（同机第二人）时安静放弃——喊话发现本就服务于不同机器之间。
pub async fn listen_for_shouts(name: String, tcp_port: u16) {
    let Ok(sock) = UdpSocket::bind(("0.0.0.0", SHOUT_PORT)).await else {
        return;
    };
    let mut buf = [0u8; 64];
    loop {
        let Ok((n, from)) = sock.recv_from(&mut buf).await else {
            continue;
        };
        if &buf[..n] == PROBE {
            let reply = format!("{REPLY_HEAD}\n{name}\n{tcp_port}");
            let _ = sock.send_to(reply.as_bytes(), from).await;
        }
    }
}

/// 同屋喊一圈「有人吗」。在听的人应「名字＋地址」，听一小会儿就收声。
pub async fn shout(my_name: &str, my_tcp_port: u16) -> Vec<Discovered> {
    let my_ips = local_ips();
    let Ok(sock) = UdpSocket::bind(("0.0.0.0", 0)).await else {
        return Vec::new();
    };
    let _ = sock.set_broadcast(true);

    // 喊两处：全网广播，外加本机回环（同机调试也听得见）
    let targets = [
        SocketAddr::from(([255, 255, 255, 255], SHOUT_PORT)),
        SocketAddr::from(([127, 0, 0, 1], SHOUT_PORT)),
    ];
    for t in targets {
        let _ = sock.send_to(PROBE, t).await;
    }

    let deadline = Instant::now() + Duration::from_millis(1200);
    let mut found: Vec<Discovered> = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            break;
        }
        let Ok(Ok((n, from))) = timeout(remain, sock.recv_from(&mut buf)).await else {
            break; // 到点收声
        };
        let Ok(text) = std::str::from_utf8(&buf[..n]) else {
            continue;
        };
        let mut parts = text.split('\n');
        if parts.next() != Some(REPLY_HEAD) {
            continue; // 不是同屋的应答
        }
        let (Some(name), Some(port)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        // 认出自己的回声：同名、同端口、又来自自己的网卡
        if name == my_name && port == my_tcp_port && my_ips.contains(&from.ip()) {
            continue;
        }
        let dial_addr = format!("{}:{port}", from.ip());
        if !found.iter().any(|d| d.dial_addr == dial_addr) {
            found.push(Discovered {
                name: name.to_string(),
                dial_addr,
            });
        }
    }
    found
}
