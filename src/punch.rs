//! 介绍人与打洞：让两堵墙后的人直连。
//!
//! 介绍人是茶馆掌柜的兼职（同一个门牌号，UDP 那半边）：只递地址，不传话。
//! 双方各自向介绍人报同一个标签，掌柜把「对方在公网上的样子」分别告诉他们，
//! 随后退场。打洞：双方同时向对方的公网地址递拳头（UDP），各自的门卫都以为
//! 是自己人先出的门，把门开着。洞开之后，Noise 照常端到端握手——
//! 介绍人自始至终没碰到一个字的明文。
//!
//! 这是尽力而为的路：握手帧约 1.6 KB，走 IP 分片；丢了就失败。
//! 失败了换茶馆重碰（/meet，手动另走一条，不自动回退）——那条路永远可用。

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// 报名的暗记：`INTRO1 <标签>`（报名者 → 介绍人）。
pub(crate) const INTRO1: &str = "INTRO1";
/// 引荐的暗记：`INTRO2 <R|I> <对方的公网地址>`（介绍人 → 报名者）。
pub(crate) const INTRO2: &str = "INTRO2";
/// 介绍人那边：一个标签等人的最长时间（过期清位，记忆不落地）。
pub(crate) const INTRO_TTL: Duration = Duration::from_secs(90);

/// 一记拳头的帧载荷。
const PUNCH: &[u8] = b"PUNCH";
/// 一个数据报能装下的上限（帧含 2 字节长度头）。
const MAX_DATAGRAM: usize = 65507;
/// 报名后等引荐的最长时间。
const INTRO_WAIT: Duration = Duration::from_secs(60);
/// 报名重试间隔（UDP 会丢包，隔几秒再喊一声）。
const INTRO_RETRY: Duration = Duration::from_secs(3);
/// 递拳头的窗口：这么久还没握上手，就当打不通。
const PUNCH_WINDOW: Duration = Duration::from_secs(8);
/// 两拳之间的间隔。
const PUNCH_INTERVAL: Duration = Duration::from_millis(150);

/// 打洞的差错。语义交给上层翻译。
#[derive(Debug, thiserror::Error)]
pub enum PunchError {
    #[error("介绍人那边出了问题：{0}")]
    Introducer(String),
    #[error("拳头递完了，门没开——对面的墙太严，回茶馆吧。")]
    NoHole,
    #[error("打洞信道出了毛病：{0}")]
    Io(#[from] io::Error),
}

/// 拳头帧的完整字节（含长度头）：路上残留的余震靠它辨认。
fn punch_frame() -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + PUNCH.len());
    v.extend_from_slice(&(PUNCH.len() as u16).to_be_bytes());
    v.extend_from_slice(PUNCH);
    v
}

/* ── 读写的共用零件：数据报 ↔ 字节流 ── */

fn poll_read_parts(
    rx: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    pending: &mut Vec<u8>,
    punch: &[u8],
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
) -> Poll<io::Result<()>> {
    // 先把上一个数据报的余字节端出去
    if !pending.is_empty() {
        let n = buf.remaining().min(pending.len());
        let chunk: Vec<u8> = pending.drain(..n).collect();
        buf.put_slice(&chunk);
        return Poll::Ready(Ok(()));
    }
    loop {
        match rx.poll_recv(cx) {
            Poll::Ready(Some(datagram)) => {
                // 余震不算话：拳头帧与空报直接吞掉
                if datagram.is_empty() || datagram == punch {
                    continue;
                }
                let n = buf.remaining().min(datagram.len());
                buf.put_slice(&datagram[..n]);
                if n < datagram.len() {
                    *pending = datagram[n..].to_vec();
                }
                return Poll::Ready(Ok(()));
            }
            Poll::Ready(None) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "打洞线断了",
                )));
            }
            Poll::Pending => return Poll::Pending,
        }
    }
}

fn poll_write_parts(
    tx: &mpsc::UnboundedSender<Vec<u8>>,
    out: &mut Vec<u8>,
    buf: &[u8],
) -> Poll<io::Result<usize>> {
    out.extend_from_slice(buf);
    // 凑齐一个整帧就发一个数据报：帧不跨报
    while out.len() >= 2 {
        let len = u16::from_be_bytes([out[0], out[1]]) as usize;
        if out.len() < 2 + len {
            break;
        }
        let end = 2 + len;
        if end > MAX_DATAGRAM {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "这一帧太长，一个数据报装不下",
            )));
        }
        let datagram: Vec<u8> = out.drain(..end).collect();
        if tx.send(datagram).is_err() {
            return Poll::Ready(Err(io::Error::other("发不出去")));
        }
    }
    Poll::Ready(Ok(buf.len()))
}

/// 打洞出来的「线」：一条已连接的 UDP 通路，帧即数据报（帧不跨报）。
/// 拆开落座见 [`PunchReader`] / [`PunchWriter`]。
pub struct UdpPunch {
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    peer: SocketAddr,
    pending: Vec<u8>,
    out: Vec<u8>,
    punch: Vec<u8>,
}

impl UdpPunch {
    /// 把一个已连接的套接字变成一条「线」。后台两个搬运工：
    /// 一个只管收（数据报→管道），一个只管发（管道→数据报）。
    /// 两半都放下时搬运工随之收工，套接字号位归还。
    fn spawn(socket: Arc<UdpSocket>, peer: SocketAddr) -> Self {
        let (tx_in, rx_in) = mpsc::unbounded_channel::<Vec<u8>>();
        let (tx_out, mut rx_out) = mpsc::unbounded_channel::<Vec<u8>>();
        let recv_socket = Arc::clone(&socket);
        tokio::spawn(async move {
            let mut buf = vec![0u8; MAX_DATAGRAM];
            loop {
                match recv_socket.recv(&mut buf).await {
                    Ok(n) if n > 0 => {
                        if tx_in.send(buf[..n].to_vec()).is_err() {
                            break; // 没人读了
                        }
                    }
                    _ => break,
                }
            }
        });
        tokio::spawn(async move {
            while let Some(bytes) = rx_out.recv().await {
                if socket.send(&bytes).await.is_err() {
                    break;
                }
            }
        });
        Self {
            rx: rx_in,
            tx: tx_out,
            peer,
            pending: Vec::new(),
            out: Vec::new(),
            punch: punch_frame(),
        }
    }

    /// 对端地址。
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// 拆成读半与写半（落座后各归各位）。
    pub fn into_parts(self) -> (PunchReader, PunchWriter) {
        (
            PunchReader {
                rx: self.rx,
                pending: self.pending,
                punch: self.punch,
            },
            PunchWriter {
                tx: self.tx,
                out: self.out,
            },
        )
    }

    /// 发一个原始数据报（打洞阶段用）。
    async fn send_raw(&self, bytes: &[u8]) -> io::Result<()> {
        self.tx
            .send(bytes.to_vec())
            .map_err(|_| io::Error::other("发不出去"))
    }

    /// 收一个原始数据报（打洞阶段用；拳头也照收）。
    async fn recv_raw(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().await
    }

    /// 把先到的话塞回去，让上层当第一帧读。
    fn push_back(&mut self, datagram: Vec<u8>) {
        self.pending = datagram;
    }
}

impl AsyncRead for UdpPunch {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        poll_read_parts(&mut this.rx, &mut this.pending, &this.punch, cx, buf)
    }
}

impl AsyncWrite for UdpPunch {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        poll_write_parts(&this.tx, &mut this.out, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(())) // 发送由搬运工异步完成
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// 打洞线的读半：数据报→字节流，顺手吞掉拳头余震。
pub struct PunchReader {
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pending: Vec<u8>,
    punch: Vec<u8>,
}

impl AsyncRead for PunchReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        poll_read_parts(&mut this.rx, &mut this.pending, &this.punch, cx, buf)
    }
}

/// 打洞线的写半：字节流→数据报，凑齐一个整帧才发（帧不跨报）。
pub struct PunchWriter {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    out: Vec<u8>,
}

impl AsyncWrite for PunchWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        poll_write_parts(&this.tx, &mut this.out, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// 经介绍人找同标签的人：报名、等引荐、递拳头、开洞。
/// 返回打洞出来的线和这端该当的角色（后报名者发起，与茶馆分主客一致）。
pub async fn meet_via(
    introducer: SocketAddr,
    tag: &str,
) -> Result<(UdpPunch, bool), PunchError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let register = format!("{INTRO1} {tag}");

    // 报名并等引荐（隔几秒再喊一声——UDP 会丢包）
    let deadline = Instant::now() + INTRO_WAIT;
    let mut next_intro = Instant::now();
    let mut buf = [0u8; 128];
    let (as_initiator, peer) = loop {
        if Instant::now() >= deadline {
            return Err(PunchError::Introducer("等不到引荐：标签没凑成一对".into()));
        }
        if Instant::now() >= next_intro {
            socket
                .send_to(register.as_bytes(), introducer)
                .await
                .map_err(|e| PunchError::Introducer(e.to_string()))?;
            next_intro = Instant::now() + INTRO_RETRY;
        }
        let wait = next_intro
            .saturating_duration_since(Instant::now())
            .min(deadline.saturating_duration_since(Instant::now()));
        match timeout(wait, socket.recv_from(&mut buf)).await {
            Err(_) => continue, // 到点：再喊一声或超时
            Ok(Err(e)) => return Err(PunchError::Introducer(e.to_string())),
            Ok(Ok((n, from))) => {
                // 只听介绍人本人的话：旁人冒名递「引荐」，不认——
                // 否则任何知道标签的第三者都能把线引到自己门口。
                if from != introducer {
                    continue;
                }
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                if let Some(rest) = text.strip_prefix(INTRO2) {
                    let mut parts = rest.split_whitespace();
                    if let (Some(role), Some(addr)) = (parts.next(), parts.next()) {
                        if let Ok(peer) = addr.parse::<SocketAddr>() {
                            break (role == "I", peer);
                        }
                    }
                }
                // 别的包忽略，接着等
            }
        }
    };

    // 连上对方的公网地址：此后所有拳与话都走这一条
    socket.connect(peer).await?;
    let mut hole = UdpPunch::spawn(Arc::new(socket), peer);

    // 递拳头，直到握上手
    let punch = punch_frame();
    let deadline = Instant::now() + PUNCH_WINDOW;
    loop {
        if Instant::now() >= deadline {
            return Err(PunchError::NoHole);
        }
        hole.send_raw(&punch).await?;
        match timeout(PUNCH_INTERVAL, hole.recv_raw()).await {
            // 拿到一记拳头：洞开了
            Ok(Some(d)) if d == punch => break,
            // 对方已经开口（先到的握手帧）：塞回去，交给上层当第一帧读
            Ok(Some(d)) => {
                hole.push_back(d);
                break;
            }
            // 这一阵没回音：再递一拳
            Err(_) => continue,
            // 信道没了
            Ok(None) => return Err(PunchError::NoHole),
        }
    }
    Ok((hole, as_initiator))
}
