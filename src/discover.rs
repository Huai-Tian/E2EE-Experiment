//! 名片：本机网卡的地址清单。
//!
//! 只看自己的网卡，不碰任何网络——这是 3.0.0 删除面之后的全部残余：
//! 局域网喊话发现（udp 广播探测/应答）已随 v3 圈子体系退役
//!（removals_3_0_0.v1_discovery_family——它曾是唯一的无邀请暴露源，
//! 找听众的事交给开台）。

use std::net::IpAddr;

/// 本机所有网卡的地址（名片用）。
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
