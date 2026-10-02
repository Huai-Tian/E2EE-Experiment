//! 会话层：一个「人」的完整生命周期——出生即在听，随时可以开口，转身即忘。
//!
//! 库的核心纪律：这里不打印任何东西、不读任何输入。
//! 动作从方法进，经历从事件出；渲染是外层使用者的事。
//! 这也是将来「围坐一圈」的骨架：每位成员持有多条并行的一对一线。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use snow::{Builder, HandshakeState, TransportState};
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use crate::wire::{read_frame, write_frame};

/// Noise NN：双方都不带旧身份，每次连接各带一副全新的临时面孔。
const NOISE_PARAMS: &str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";

/// 找人时敲门的最长等待。
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);

/// 一个人经历过、需要让外层知道的事。
/// non_exhaustive：往后的经历种类只会更多，外层匹配请留一个兜底臂。
#[non_exhaustive]
#[derive(Debug)]
pub enum Event {
    /// 门外有动静（还没握手成）
    Knocked { peer: String },
    /// 一场对话开始了。focused = 注意力是否落在了它身上
    Met { id: u64, name: String, focused: bool },
    /// 一场相遇没能善始（来客握手失败等）
    MeetFailed { error: String },
    /// 听到一句话
    Heard { id: u64, name: String, text: String },
    /// 一场对话结束了
    Left { id: u64, name: String, reason: LeaveReason },
    /// 收摊之后，注意力挪了地方
    FocusReturned { to: Option<u64> },
}

/// 一场对话是怎么结束的。
#[non_exhaustive]
#[derive(Debug)]
pub enum LeaveReason {
    /// 对方道了别（空明文帧）
    Farewell,
    /// 对方断了线
    Disconnected,
    /// 话解不开 —— 信道有问题
    Undecipherable,
    /// 线路故障
    Wire(String),
}

/// 出错时，库只说语义；把错误说成人话是外层的事。
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PersonError {
    #[error("占不到 {addr}：{source}")]
    Bind { addr: String, source: std::io::Error },
    #[error("拨不通 {addr}：{source}")]
    Dial { addr: String, source: std::io::Error },
    #[error("拨不通 {addr}：太久没人应门。")]
    DialTimeout { addr: String },
    #[error("这场相遇没能善始：{0}")]
    Greet(String),
    #[error("你现在没在跟任何人说话。")]
    NoFocus,
    #[error("没有这样的对话。")]
    NoConvo,
    #[error("这句话没能送达。")]
    Send(#[source] std::io::Error),
    #[error("加密失败。")]
    Encrypt(#[source] snow::Error),
    #[error("话没送到——[{name}] 恐怕已经走了。")]
    ByeFailed { name: String },
}

/// 事件流：这个人的一切经历从这里流出。
pub type Events = mpsc::UnboundedReceiver<Event>;

/// 一场对话：对面是谁、共说什么话、往哪条线说。
struct Convo {
    their_name: String,
    cipher: Arc<Mutex<TransportState>>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
}

/// 一个人藏在心里的账：名字、正在进行的对话、此刻的注意力。
struct Inner {
    my_name: String,
    convos: HashMap<u64, Convo>,
    focus: Option<u64>,
    next_id: u64,
    events: mpsc::UnboundedSender<Event>,
}

/// 一个人。克隆不产生新的人，只是多一只手牵着他。
#[derive(Clone)]
pub struct Person {
    inner: Arc<Mutex<Inner>>,
}

/// 名册上的一行。
#[derive(Debug)]
pub struct RosterEntry {
    pub id: u64,
    pub name: String,
    pub focused: bool,
}

impl Person {
    /// 出生：起个名字，守在一个地址上听。
    /// 返回这个人，和他的事件流。
    pub async fn born(name: &str, listen_addr: &str) -> Result<(Person, Events), PersonError> {
        let listener = TcpListener::bind(listen_addr)
            .await
            .map_err(|source| PersonError::Bind {
                addr: listen_addr.to_string(),
                source,
            })?;
        let (tx, rx) = mpsc::unbounded_channel();
        let person = Person {
            inner: Arc::new(Mutex::new(Inner {
                my_name: name.to_string(),
                convos: HashMap::new(),
                focus: None,
                next_id: 1,
                events: tx,
            })),
        };

        // 接客：耳朵永远开着，来一个接一个，互不耽搁。
        let inner = person.inner.clone();
        tokio::spawn(async move {
            loop {
                let (stream, peer) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => continue, // 接客遇到小磕绊，接着守门
                };
                {
                    let p = inner.lock().await;
                    p.events
                        .send(Event::Knocked {
                            peer: peer.to_string(),
                        })
                        .ok();
                }
                let inner = inner.clone();
                // 每位来客单独接待：一位握手卡住，不耽误下一位
                tokio::spawn(async move {
                    if let Err(e) = greet(&inner, stream, false).await {
                        inner
                            .lock()
                            .await
                            .events
                            .send(Event::MeetFailed {
                                error: e.to_string(),
                            })
                            .ok();
                    }
                });
            }
        });

        Ok((person, rx))
    }

    /// 找人：拨通、握手、成为当前对话。返回对话编号。
    pub async fn dial(&self, addr: &str) -> Result<u64, PersonError> {
        let stream = match tokio::time::timeout(DIAL_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(s)) => s,
            Ok(Err(source)) => {
                return Err(PersonError::Dial {
                    addr: addr.to_string(),
                    source,
                })
            }
            Err(_) => {
                return Err(PersonError::DialTimeout {
                    addr: addr.to_string(),
                })
            }
        };
        greet(&self.inner, stream, true).await
    }

    /// 说一句话给此刻注意力所在的人。
    pub async fn speak(&self, text: &str) -> Result<(), PersonError> {
        let target = {
            let p = self.inner.lock().await;
            p.focus
                .and_then(|id| p.convos.get(&id))
                .map(|c| (c.cipher.clone(), c.writer.clone()))
        };
        let Some((cipher, writer)) = target else {
            return Err(PersonError::NoFocus);
        };
        send_frame(&cipher, &writer, text.as_bytes()).await
    }

    /// 把注意力切到另一场对话。返回对方的名字。
    pub async fn talk_to(&self, id: u64) -> Result<String, PersonError> {
        let name = {
            let p = self.inner.lock().await;
            p.convos.get(&id).map(|c| c.their_name.clone())
        };
        let Some(name) = name else {
            return Err(PersonError::NoConvo);
        };
        self.inner.lock().await.focus = Some(id);
        Ok(name)
    }

    /// 看看在场的人。
    pub async fn roster(&self) -> Vec<RosterEntry> {
        let p = self.inner.lock().await;
        let mut v: Vec<_> = p
            .convos
            .iter()
            .map(|(id, c)| RosterEntry {
                id: *id,
                name: c.their_name.clone(),
                focused: p.focus == Some(*id),
            })
            .collect();
        v.sort_by_key(|r| r.id);
        v
    }

    /// 和当前对话的人道别。收摊由耳朵任务自己完成。
    pub async fn bye(&self) -> Result<String, PersonError> {
        let target = {
            let p = self.inner.lock().await;
            p.focus
                .and_then(|id| p.convos.get(&id))
                .map(|c| (c.cipher.clone(), c.writer.clone(), c.their_name.clone()))
        };
        let Some((cipher, writer, name)) = target else {
            return Err(PersonError::NoFocus);
        };
        send_frame(&cipher, &writer, &[])
            .await
            .map_err(|_| PersonError::ByeFailed {
                name: name.clone(),
            })?;
        Ok(name)
    }

    /// 离场：和所有人道别。
    pub async fn leave(&self) {
        let targets: Vec<_> = {
            let p = self.inner.lock().await;
            p.convos
                .values()
                .map(|c| (c.cipher.clone(), c.writer.clone()))
                .collect()
        };
        for (cipher, writer) in targets {
            let _ = send_frame(&cipher, &writer, &[]).await;
        }
    }
}

/// 相遇：握手、互报家门、登记入册、竖起一只耳朵。
/// 主动去找的人自然成为注意力所在；不速之客不抢注意力。
async fn greet(
    inner: &Arc<Mutex<Inner>>,
    mut stream: TcpStream,
    as_initiator: bool,
) -> Result<u64, PersonError> {
    let my_name = inner.lock().await.my_name.clone();
    let transport = handshake(&mut stream, as_initiator)
        .await
        .map_err(|e| PersonError::Greet(e.to_string()))?;

    // 互报姓名：两边同时先发后收，全双工，不会死锁
    let (mut rh, mut wh) = stream.into_split();
    write_frame(&mut wh, my_name.as_bytes())
        .await
        .map_err(|e| PersonError::Greet(e.to_string()))?;
    let Some(their_raw) = read_frame(&mut rh)
        .await
        .map_err(|e| PersonError::Greet(e.to_string()))?
    else {
        return Err(PersonError::Greet("对面还没开口就走了。".into()));
    };
    let their_name = String::from_utf8_lossy(&their_raw).into_owned();

    let cipher = Arc::new(Mutex::new(transport));
    let writer = Arc::new(Mutex::new(wh));

    let (id, focused) = {
        let mut p = inner.lock().await;
        let id = p.next_id;
        p.next_id += 1;
        p.convos.insert(
            id,
            Convo {
                their_name: their_name.clone(),
                cipher: cipher.clone(),
                writer: writer.clone(),
            },
        );
        let focused = as_initiator || p.focus.is_none();
        if focused {
            p.focus = Some(id);
        }
        (id, focused)
    };
    inner
        .lock()
        .await
        .events
        .send(Event::Met {
            id,
            name: their_name.clone(),
            focused,
        })
        .ok();

    let inner = inner.clone();
    tokio::spawn(async move {
        ear(inner, id, rh, cipher, their_name).await;
    });
    Ok(id)
}

/// 耳朵：收帧 → 解密 → 送出事件。空明文帧 = 对方道别。
/// 这只耳朵合上时顺手收摊：摘对话、关线路、必要时挪回注意力。
async fn ear(
    inner: Arc<Mutex<Inner>>,
    id: u64,
    mut rh: OwnedReadHalf,
    cipher: Arc<Mutex<TransportState>>,
    their_name: String,
) {
    let reason = loop {
        match read_frame(&mut rh).await {
            Ok(Some(ciphertext)) => {
                let mut plain = vec![0u8; ciphertext.len()];
                match cipher.lock().await.read_message(&ciphertext, &mut plain) {
                    Ok(0) => break LeaveReason::Farewell,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&plain[..n]).into_owned();
                        let p = inner.lock().await;
                        p.events
                            .send(Event::Heard {
                                id,
                                name: their_name.clone(),
                                text,
                            })
                            .ok();
                    }
                    Err(_) => break LeaveReason::Undecipherable,
                }
            }
            Ok(None) => break LeaveReason::Disconnected,
            Err(e) => break LeaveReason::Wire(e.to_string()),
        }
    };

    // 收摊
    let mut p = inner.lock().await;
    p.events
        .send(Event::Left {
            id,
            name: their_name,
            reason,
        })
        .ok();
    if let Some(convo) = p.convos.remove(&id) {
        let _ = convo.writer.lock().await.shutdown().await;
    }
    if p.focus == Some(id) {
        p.focus = p.convos.keys().max().copied();
        let to = p.focus;
        p.events.send(Event::FocusReturned { to }).ok();
    }
}

/// 加密并发送一帧明文。空明文帧 = 道别。
async fn send_frame(
    cipher: &Arc<Mutex<TransportState>>,
    writer: &Arc<Mutex<OwnedWriteHalf>>,
    plaintext: &[u8],
) -> Result<(), PersonError> {
    // 密文 = 明文 + 16 字节认证标签，多留一点余量
    let mut ciphertext = vec![0u8; plaintext.len() + 32];
    let n = cipher
        .lock()
        .await
        .write_message(plaintext, &mut ciphertext)
        .map_err(PersonError::Encrypt)?;
    let mut w = writer.lock().await;
    write_frame(&mut *w, &ciphertext[..n])
        .await
        .map_err(PersonError::Send)?;
    Ok(())
}

/// 见面：Noise NN 握手。拨出方先亮临时公钥（e），接听方回 e 和 ee，
/// 双方由此得出只有彼此知道的共享密钥，随后转入加密传输。
async fn handshake(
    stream: &mut TcpStream,
    as_initiator: bool,
) -> Result<TransportState, Box<dyn std::error::Error>> {
    let params: snow::params::NoiseParams = NOISE_PARAMS.parse()?;
    let mut hs: HandshakeState = if as_initiator {
        Builder::new(params).build_initiator()?
    } else {
        Builder::new(params).build_responder()?
    };
    let mut buf = [0u8; 1024];

    if as_initiator {
        let n = hs.write_message(&[], &mut buf)?;
        write_frame(stream, &buf[..n]).await?;
        let Some(msg) = read_frame(stream).await? else {
            return Err("握手途中对方不见了。".into());
        };
        hs.read_message(&msg, &mut buf)?;
    } else {
        let Some(msg) = read_frame(stream).await? else {
            return Err("对方连上就走。".into());
        };
        hs.read_message(&msg, &mut buf)?;
        let n = hs.write_message(&[], &mut buf)?;
        write_frame(stream, &buf[..n]).await?;
    }

    Ok(hs.into_transport_mode()?)
}
