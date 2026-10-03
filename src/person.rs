//! 会话层：一个「人」的完整生命周期——出生即在听，随时可以开口，转身即忘。
//!
//! 库的核心纪律：这里不打印任何东西、不读任何输入。
//! 动作从方法进，经历从事件出；渲染是外层使用者的事。
//! 这也是将来「围坐一圈」的骨架：每位成员持有多条并行的一对一线。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use crate::discover::{self, Discovered};
use crate::wire::{read_frame, write_frame};

/// 线的读半与写半：TCP 与打洞线落座后走同一种号位。
type BoxedReader = Box<dyn AsyncRead + Send + Unpin>;
type BoxedWriter = Box<dyn AsyncWrite + Send + Unpin>;

/// Noise 混成套件（无暗号）：X25519 ＋ Kyber1024 两道锁。
const NOISE_PARAMS: &str = "Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s";
/// Noise 混成套件（带暗号）：点火钥匙作 PSK，两道锁照上。
const NOISE_PARAMS_PSK: &str = "Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s";

/// 找人时敲门的最长等待。
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);

/// 相遇的总预算：对暗号、混成握手、互报姓名，都各不得超过这么久。
/// 连上却不吭声的（端口扫描之类），不该一直占着门厅与切口。
const MEET_TIMEOUT: Duration = Duration::from_secs(10);

/// 单个文件的大小上限，收发同限。收方在内存里重组（零持久化：库永不
/// 落盘），32MB 对任何现代机器都轻松，又足以拦下恶意巨块。
pub const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
/// 每块数据的大小：2+1+4+32768 = 32775 < 65507——一块加帧头装得进
/// UDP 数据报，于是文件在直连、茶馆、打洞线上都同样能传。
const FILE_CHUNK_LEN: usize = 32 * 1024;

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
    /// 暗号链续上了：对方在告别时预约了下一把暗号
    SecretChained { with: String },
    /// 圈子铸成：凑话收齐，群钥匙在手，屋里的话可以开讲了
    CircleFormed { members: usize },
    /// 圈里收到一句解不开的话：密钥不合（成员集不一致）或密文被动过。
    /// 不中断会话，只提醒——通常意味着该重新围一次。
    CircleMumble { from: String },
    /// 一份文件收齐了：字节原样都在内存里（data），无损。落不落盘、
    /// 落在哪，是外层使用者的事——库永不写盘。
    FileArrived { id: u64, name: String, data: Vec<u8> },
    /// 对方想递一份超过上限的文件，婉拒了（提醒人类，不中断会话）。
    FileDeclined { from: String, name: String, size: u64 },
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
    #[error("茶馆里等不到人：{detail}")]
    Teahouse { detail: String },
    #[error("打洞不成：{0}")]
    Punch(String),
    #[error("这场相遇没能善始：{0}")]
    Greet(String),
    #[error("你现在没在跟任何人说话。")]
    NoFocus,
    #[error("没有这样的对话。")]
    NoConvo,
    #[error("还没围坐成圈。")]
    NoCircle,
    #[error("这句话没能送达。")]
    Send(#[source] std::io::Error),
    #[error("加密失败。")]
    Encrypt(#[source] snow::Error),
    #[error("话没送到——[{name}] 恐怕已经走了。")]
    ByeFailed { name: String },
    #[error("这份文件太大了（{size} 字节）：一次至多 32MB。")]
    FileTooLarge { size: usize },
    #[error("文件名不能用：留了空、或超过 255 字节。")]
    FileNameBad,
    #[error("上一份文件还没递完，一份递完再递下一份。")]
    FileBusy,
}

/// 事件流：这个人的一切经历从这里流出。
pub type Events = mpsc::UnboundedReceiver<Event>;

/// 加密载荷的首字节：这一帧是什么。
mod frame_kind {
    /// 聊天行：其后是 UTF-8 文本
    pub const CHAT: u8 = 0x00;
    /// 暗号预约：其后是 32 字节新暗号（告别前发出，等确认）
    pub const OFFER: u8 = 0x01;
    /// 预约确认：我收到了下一把暗号
    pub const ACK: u8 = 0x02;
    /// 凑话：其后是 32 字节随机贡献（围圈时经各线广播）
    pub const CONTRIB: u8 = 0x03;
    /// 群聊：其后是群密文（nonce ‖ AEAD，同一份密文各线同送）
    pub const CIRCLE: u8 = 0x04;
    /// 自报家门：其后是 UTF-8 名字（握手后第一帧，双方同时先发后收）。
    /// 也走密文——名字是应用数据，不该裸奔在线上给茶馆和路人看热闹。
    pub const NAME: u8 = 0x05;
    /// 文件头：其后是 名字长度u8 ‖ 名字UTF-8 ‖ 总字节数u64（开传前先报家门）
    pub const FILE_HEAD: u8 = 0x06;
    /// 文件块：其后是 序号u32 ‖ 数据（每块 ≤32KB，一块一帧，打洞线也装得下）
    pub const FILE_CHUNK: u8 = 0x07;
    /// 带垫的聊天行：其后是 正文长u16 ‖ 正文 ‖ 随机垫(0..=255)。
    /// 帧长不再紧贴正文长——「看包长猜话长」的关联被垫糊掉；
    /// 收方按长度前缀取正文、垫直接丢弃，于是无需对端任何配合。
    /// 旧版同伴不认识 0x08：沉默跳过（看不见这行，但不显乱码）。
    pub const PADDED_CHAT: u8 = 0x08;
}

/// 握手失败的账：错在哪，以及切口是否已被查验。
/// burned = 双方遮罩都交换过、暗号被真正比对过（对错都算）；
/// 没走到这一步的失败（拨不通、对端半路消失、报文残缺、超时）不烧切口——
/// 否则端口上扫一下就能替你撕掉暗号约定。
struct HandshakeFailure {
    err: Box<dyn std::error::Error + Send + Sync>,
    burned: bool,
}

impl HandshakeFailure {
    /// 还没验到暗号就失败的（断线、报文残缺、套件构造失败）：切口不算消耗。
    fn early(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            err: e.into(),
            burned: false,
        }
    }
    /// 暗号已被查验（遮罩交换完成，对错都算）：这把暗号花掉了。
    fn checked(msg: &str) -> Self {
        Self {
            err: msg.into(),
            burned: true,
        }
    }
}

/// 相遇失败的账：语义错误（说给人听）＋ 切口是否已被查验（决定归还还是作废）。
struct GreetFailure {
    err: PersonError,
    burned: bool,
}

/// 一场围坐：凑话、铸出的群钥匙、发言计数。
struct Gathering {
    /// 我自己的贡献
    my_r: Vec<u8>,
    /// 各条线上收到的贡献：对话编号 → 贡献
    contribs: HashMap<u64, Vec<u8>>,
    /// 收齐后铸出的群钥匙
    key: Option<[u8; 32]>,
    /// 我的发言计数（nonce 唯一性）
    counter: u64,
}

/// 一场对话：对面是谁、共说什么话、往哪条线说。
struct Convo {
    their_name: String,
    /// 这条线挂不挂得住暗号链：直连与打洞线挂得住（地址可作锚）；
    /// 茶馆线挂不住（房间一次性，锚不存在）——道别时不预约，
    /// 免得约出一把用不上、还堵住下一场的暗号。
    chainable: bool,
    cipher: Arc<Mutex<TransportState>>,
    writer: Arc<Mutex<BoxedWriter>>,
}

/// 一个人藏在心里的账：名字、守着的端口、正在进行的对话、此刻的注意力、
/// 以及等下一位来客要对的切口（用后即焚）。
struct Inner {
    my_name: String,
    listen_port: u16,
    /// 实际绑定上的地址（端口填 0 时这里是真端口）。
    listen_addr: String,
    convos: HashMap<u64, Convo>,
    focus: Option<u64>,
    next_id: u64,
    events: mpsc::UnboundedSender<Event>,
    /// 等人的切口：设了之后，下一位来客必须对得上才谈得成。一位一焚。
    expecting_secret: Option<Vec<u8>>,
    /// 常备门禁（机器密钥形态）：每位来客都要对上才谈得成，且从不消耗。
    /// one-shot 的「烧」护的是低熵人类暗号（防在线爆破）；32 字节机器
    /// 密钥在线爆破本就不可能——于是错多少次都锁不住台，真钥永远进得来。
    /// 一次性切口（/await）若在，优先生效；烧掉之后门禁照常。
    stable_secret: Option<Vec<u8>>,
    /// 暗号链（主动侧）：地址 → 上一场告别时预约的下一把。用过即焚。
    chained_secrets: HashMap<String, Vec<u8>>,
    /// 挂起待确认的预约：对话编号 → 新暗号。收到确认才入链。
    pending_offers: HashMap<u64, Vec<u8>>,
    /// 续链总开关（默认开）。关掉后：道别不预约、来约不接受——
    /// 对陌生人常开的台席（意见反馈热线）用它，谁来都行，散场不留锁。
    /// 链是便利不是必需，关掉只丢便利，不丢安全。
    chaining: bool,
    /// 正在进门的文件（每场对话至多一份在途）：内存重组，上限 MAX_FILE_BYTES。
    inbound_files: HashMap<u64, InboundFile>,
    /// 正在出门的文件（每场对话至多一份在途）：防两份并发交错成乱麻。
    outgoing_files: std::collections::HashSet<u64>,
    /// 正在围坐的圈子（至多一个：一个人同时在一个屋里说话）
    gathering: Option<Gathering>,
}

/// 一份正在进门的东西：名字、总长、已收的字节、下一块的序号。
struct InboundFile {
    name: String,
    total: u64,
    buf: Vec<u8>,
    next_seq: u32,
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
        Self::born_impl(name, listen_addr, false).await
    }

    /// 出生（隐身）：同 [`Person::born`]，只是永不应答同屋的「有人吗」——
    /// 广播探测探不到这个人的存在；拿着确切地址（名片、口头约定），
    /// 或经茶馆、介绍人约好的人，才找得到他。
    /// TCP 耳朵照常开着：隐身不是闭门，只是不应门铃之外的那声喊。
    pub async fn born_hidden(
        name: &str,
        listen_addr: &str,
    ) -> Result<(Person, Events), PersonError> {
        Self::born_impl(name, listen_addr, true).await
    }

    async fn born_impl(
        name: &str,
        listen_addr: &str,
        hidden: bool,
    ) -> Result<(Person, Events), PersonError> {
        let listener = TcpListener::bind(listen_addr)
            .await
            .map_err(|source| PersonError::Bind {
                addr: listen_addr.to_string(),
                source,
            })?;
        let local = listener
            .local_addr()
            .map_err(|source| PersonError::Bind {
                addr: listen_addr.to_string(),
                source,
            })?;
        let port = local.port();
        let bound = local.to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let person = Person {
            inner: Arc::new(Mutex::new(Inner {
                my_name: name.to_string(),
                listen_port: port,
                listen_addr: bound,
                convos: HashMap::new(),
                focus: None,
                next_id: 1,
                events: tx,
                expecting_secret: None,
                stable_secret: None,
                chained_secrets: HashMap::new(),
                pending_offers: HashMap::new(),
                chaining: true,
                inbound_files: HashMap::new(),
                outgoing_files: std::collections::HashSet::new(),
                gathering: None,
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
                    // 门禁的两种形态，来客进门先领一份：
                    // - 一次性切口（/await）：取走即焚——一位一验，
                    //   没验到暗号的失败原样归还（扫描烧不掉约定）；
                    // - 常备门禁（--guard）：复制一把、从不取走——
                    //   机器密钥不怕在线爆破，瞎拨多少次也锁不住台。
                    // 两者并存时一次性切口优先（那是人显式备给下一位的）。
                    let (secret, one_shot) = {
                        let mut p = inner.lock().await;
                        match p.expecting_secret.take() {
                            Some(s) => (Some(s), true),
                            None => (p.stable_secret.clone(), false),
                        }
                    };
                    // 来客的来源地址：将来他主动来续链时的锚
                    let peer_addr = stream
                        .peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_default();
                    if let Err(f) =
                        greet(&inner, stream, false, secret.as_deref(), peer_addr).await
                    {
                        // 只有一次性切口存在「归还」；常备的从没取走，无需归还
                        if !f.burned && one_shot {
                            let mut p = inner.lock().await;
                            if p.expecting_secret.is_none() {
                                p.expecting_secret = secret;
                            }
                        }
                        inner
                            .lock()
                            .await
                            .events
                            .send(Event::MeetFailed {
                                error: f.err.to_string(),
                            })
                            .ok();
                    }
                });
            }
        });

        // 同屋的耳朵：听到「有人吗」就应一声（占不到喊话端口就安静放弃）。
        // 隐身的人压根不竖这只耳朵：广播探不到他，精确交往照旧。
        if !hidden {
            tokio::spawn(discover::listen_for_shouts(name.to_string(), port));
        }

        Ok((person, rx))
    }

    /// 这个人守着的地址（实际绑定；监听时端口填 0，这里给出真端口）。
    pub async fn listen_addr(&self) -> String {
        self.inner.lock().await.listen_addr.clone()
    }

    /// 备好切口：下一位来客必须对得上这句暗号才谈得成（一位一焚）。
    pub async fn expect_secret(&self, secret: &[u8]) {
        self.inner.lock().await.expecting_secret = Some(secret.to_vec());
    }

    /// 立起常备门禁：**每一位**来客都要对上这句暗号才谈得成，且从不消耗——
    /// 错多少次、进多少次，门禁都还立在门口，真钥永远进得来。
    /// 这句应是 32 字节级的随机机器密钥（客户端内嵌），不是人类暗号：
    /// 一次性切口的「烧」防的是对低熵暗号的在线爆破，机器密钥在线爆破
    /// 本就不可能，不需要那份保护——于是瞎拨锁不住台（DoS 无效）。
    pub async fn expect_secret_stable(&self, secret: &[u8]) {
        self.inner.lock().await.stable_secret = Some(secret.to_vec());
    }

    /// 续链开关：关掉后这个人道别不预约、来约不接受。
    /// 对陌生人常开的台席用它（`--no-chain` 的库面）——
    /// 不然每个道别的客人都会给下一位陌生来客上门锁。
    pub async fn set_chaining(&self, enabled: bool) {
        self.inner.lock().await.chaining = enabled;
    }

    /// 找人：拨通、握手、成为当前对话。返回对话编号。
    /// 若上一场告别时与这个地址预约过下一把暗号，自动带上（用过才焚）。
    pub async fn dial(&self, addr: &str) -> Result<u64, PersonError> {
        let chained = self.inner.lock().await.chained_secrets.get(addr).cloned();
        match self.dial_inner(addr, chained.as_deref()).await {
            Ok(id) => {
                // 谈成了：链上的暗号花在了这场对话上
                self.inner.lock().await.chained_secrets.remove(addr);
                Ok(id)
            }
            Err(f) => {
                // 用过才焚：暗号被真正查验过（对错都算）才作数；
                // 只是拨不通、或没走到对暗号，链上的暗号留着下次再试。
                if f.burned {
                    self.inner.lock().await.chained_secrets.remove(addr);
                }
                Err(f.err)
            }
        }
    }

    /// 找人的内层：带失败情报（切口是否被查验过），dial 与 dial_secret 共用。
    async fn dial_inner(&self, addr: &str, secret: Option<&[u8]>) -> Result<u64, GreetFailure> {
        let stream = match tokio::time::timeout(DIAL_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(s)) => s,
            Ok(Err(source)) => {
                return Err(GreetFailure {
                    err: PersonError::Dial {
                        addr: addr.to_string(),
                        source,
                    },
                    burned: false,
                })
            }
            Err(_) => {
                return Err(GreetFailure {
                    err: PersonError::DialTimeout {
                        addr: addr.to_string(),
                    },
                    burned: false,
                })
            }
        };
        let peer_addr = addr.to_string();
        greet(&self.inner, stream, true, secret, peer_addr).await
    }

    /// 带暗号找人：暗号对不上，谈不成。
    pub async fn dial_secret(
        &self,
        addr: &str,
        secret: Option<&[u8]>,
    ) -> Result<u64, PersonError> {
        self.dial_inner(addr, secret).await.map_err(|f| f.err)
    }

    /// 经茶馆找事先约好的人：双方拨同一间茶馆、报同一个房号，
    /// 茶馆把两条线接上之后，Noise 照常端到端握手
    /// （角色由茶馆分配：后到者发起，先到者应答）。
    /// 返回对话编号。房里是谁——照旧由你自己判断。
    pub async fn meet_at_teahouse(
        &self,
        courier_addr: &str,
        room: u32,
    ) -> Result<u64, PersonError> {
        self.meet_at_teahouse_secret(courier_addr, room, None).await
    }

    /// 经茶馆、带暗号找人：暗号对不上，谈不成。
    pub async fn meet_at_teahouse_secret(
        &self,
        courier_addr: &str,
        room: u32,
        secret: Option<&[u8]>,
    ) -> Result<u64, PersonError> {
        let (mut stream, as_initiator) = crate::courier::enter(courier_addr, room)
            .await
            .map_err(|e| PersonError::Teahouse {
                detail: e.to_string(),
            })?;
        // 房里先对暗号（若双方都带了），随后混成握手照常
        let transport = handshake(&mut stream, as_initiator, secret)
            .await
            .map_err(|f| PersonError::Greet(f.err.to_string()))?;
        // 茶馆场景的对端地址未知（角色互换、房间一次性），链挂不住：不预约
        let peer_addr = format!("{courier_addr}#{room}");
        let (rh, wh) = stream.into_split();
        register_convo(
            &self.inner,
            Box::new(rh),
            Box::new(wh),
            transport,
            as_initiator,
            peer_addr,
            secret.is_some(),
            false, // 茶馆线不挂链：房间一次性，锚不存在
        )
            .await
            .map_err(|f| f.err)
    }

    /// 经介绍人打洞找人：双方向同一位介绍人报同一个标签，
    /// 介绍人互递公网地址后退场；拳头握上，Noise 照常端到端握手
    /// （角色由介绍人分配：后报名者发起）。打不通（墙太严）就换
    /// 茶馆重碰（/meet，手动另走一条，不自动回退）。
    /// 返回对话编号。洞那头是谁——照旧由你自己判断。
    pub async fn punch(&self, introducer_addr: &str, tag: &str) -> Result<u64, PersonError> {
        self.punch_secret(introducer_addr, tag, None).await
    }

    /// 经介绍人、带暗号打洞找人：暗号对不上，谈不成。
    pub async fn punch_secret(
        &self,
        introducer_addr: &str,
        tag: &str,
        secret: Option<&[u8]>,
    ) -> Result<u64, PersonError> {
        let addr: SocketAddr = introducer_addr.parse().map_err(|_| {
            PersonError::Punch("介绍人地址念得不对（要用 IP:PORT）".into())
        })?;
        let (mut hole, as_initiator) = crate::punch::meet_via(addr, tag)
            .await
            .map_err(|e| PersonError::Punch(e.to_string()))?;
        let peer_addr = hole.peer_addr().to_string();
        let transport = handshake(&mut hole, as_initiator, secret)
            .await
            .map_err(|f| PersonError::Greet(f.err.to_string()))?;
        let (rh, wh) = hole.into_parts();
        register_convo(
            &self.inner,
            Box::new(rh),
            Box::new(wh),
            transport,
            as_initiator,
            peer_addr,
            secret.is_some(),
            true, // 打洞线挂得住链：对端公网端点可作锚（NAT 映射易逝，链可能拨不回去——不出错，只是用不上）
        )
            .await
            .map_err(|f| f.err)
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
        send_chat(&cipher, &writer, text.as_bytes()).await
    }

    /// 递一份文件给此刻注意力所在的人：分块加密发送，每块独立成帧。
    /// 字节原样走密文、原样进门——二进制不欠文字的安检（无损，
    /// 收到的每一个字节都与给出的一毫不差）。上限收发同限（32MB）；
    /// 库不读盘也不写盘：字节从哪来、到哪去，都由外层使用者决定。
    /// 一场对话同一时刻只递一份（并发递第二份会被拒），聊天照常穿插。
    pub async fn send_file(&self, name: &str, data: &[u8]) -> Result<(), PersonError> {
        if data.len() > MAX_FILE_BYTES {
            return Err(PersonError::FileTooLarge { size: data.len() });
        }
        let name = name.trim();
        if name.is_empty() || name.len() > 255 || name.contains('\0') {
            return Err(PersonError::FileNameBad);
        }
        let target = {
            let mut p = self.inner.lock().await;
            let Some(id) = p.focus else {
                return Err(PersonError::NoFocus);
            };
            // 一场对话一份在途：两份交错会让对方的重组乱序
            if p.outgoing_files.contains(&id) {
                return Err(PersonError::FileBusy);
            }
            let Some(c) = p.convos.get(&id) else {
                return Err(PersonError::NoConvo);
            };
            let pair = (c.cipher.clone(), c.writer.clone());
            p.outgoing_files.insert(id);
            (id, pair.0, pair.1)
        };
        let result = self
            .send_file_frames(name, data, &target.1, &target.2)
            .await;
        self.inner.lock().await.outgoing_files.remove(&target.0);
        result
    }

    /// 文件的实际出门：先递家门（名字＋总长），再逐块递字节。
    /// 块与块的边界在 [8KB, 32KB] 里随机——「每块都恰好 32KB」的固定
    /// 形状是指纹，随机化把它抹掉。收方本就不假设块长（来多大收多大），
    /// 于是这纯属发送方的私事，无需协商、旧版照收。总量的形状
    /// （这份文件大约多大）是另一层：要抹它得靠应用层把容器凑整。
    async fn send_file_frames(
        &self,
        name: &str,
        data: &[u8],
        cipher: &Arc<Mutex<TransportState>>,
        writer: &Arc<Mutex<BoxedWriter>>,
    ) -> Result<(), PersonError> {
        let mut head = Vec::with_capacity(2 + name.len());
        head.push(name.len() as u8);
        head.extend_from_slice(name.as_bytes());
        head.extend_from_slice(&(data.len() as u64).to_be_bytes());
        send_typed(cipher, writer, frame_kind::FILE_HEAD, &head).await?;
        let mut rest = data;
        let mut seq: u32 = 0;
        while !rest.is_empty() {
            let take = if rest.len() > FILE_CHUNK_LEN {
                Self::random_in(FILE_CHUNK_LEN / 4, FILE_CHUNK_LEN)
            } else {
                rest.len()
            };
            let (chunk, tail) = rest.split_at(take);
            let mut part = Vec::with_capacity(4 + chunk.len());
            part.extend_from_slice(&seq.to_be_bytes());
            part.extend_from_slice(chunk);
            send_typed(cipher, writer, frame_kind::FILE_CHUNK, &part).await?;
            rest = tail;
            seq += 1;
        }
        Ok(())
    }

    /// [lo, hi] 闭区间里随机取一。垫形用（不是密码学决策点）；
    /// 取不出随机数就回 hi——垫不出来只是形状不糊，功能照常。
    fn random_in(lo: usize, hi: usize) -> usize {
        let span = hi - lo + 1;
        let mut b = [0u8; 2];
        if getrandom::fill(&mut b).is_err() {
            return hi;
        }
        lo + (u16::from_be_bytes(b) as usize) % span
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

    /// 这个人的名片：每个网卡一行「名字  地址」。
    pub async fn card(&self) -> Vec<String> {
        let p = self.inner.lock().await;
        discover::card(&p.my_name, p.listen_port)
    }

    /// 同屋喊一圈「有人吗」，看看谁在。
    pub async fn shout(&self) -> Vec<Discovered> {
        let (name, port) = {
            let p = self.inner.lock().await;
            (p.my_name.clone(), p.listen_port)
        };
        discover::shout(&name, port).await
    }

    /// 和当前对话的人道别。收摊由耳朵任务自己完成。
    /// 道别之前，先把下一把暗号预约进这条已认证的信道——
    /// 对方确认了，链就续上了；没等到确认，预约作废（丢的是便利，不是安全）。
    /// 茶馆线不预约（锚不存在，见 Convo::chainable）。
    pub async fn bye(&self) -> Result<String, PersonError> {
        let target = {
            let p = self.inner.lock().await;
            p.focus.and_then(|id| {
                p.convos.get(&id).map(|c| {
                    (
                        c.cipher.clone(),
                        c.writer.clone(),
                        c.their_name.clone(),
                        c.chainable,
                        p.chaining,
                        id,
                    )
                })
            })
        };
        let Some((cipher, writer, name, chainable, chaining, id)) = target else {
            return Err(PersonError::NoFocus);
        };
        if chainable && chaining {
            offer_next_secret(&self.inner, id, &cipher, &writer).await;
        }
        send_frame(&cipher, &writer, &[])
            .await
            .map_err(|_| PersonError::ByeFailed {
                name: name.clone(),
            })?;
        Ok(name)
    }

    /// 离场：和所有人道别（续链开着且挂得住链的每场都尝试预约下一把）。
    pub async fn leave(&self) {
        let targets: Vec<_> = {
            let p = self.inner.lock().await;
            p.convos
                .iter()
                .map(|(id, c)| {
                    (
                        *id,
                        c.cipher.clone(),
                        c.writer.clone(),
                        c.chainable,
                        p.chaining,
                    )
                })
                .collect()
        };
        for (id, cipher, writer, chainable, chaining) in targets {
            if chainable && chaining {
                offer_next_secret(&self.inner, id, &cipher, &writer).await;
            }
            let _ = send_frame(&cipher, &writer, &[]).await;
        }
    }

    /// 围坐一圈：对当前在场的所有人凑话铸钥。
    /// 前提：圈内每两人已有一条线（全连接）。缺的线请先 /dial 补齐，
    /// 之后再围一次（新成员加入 = 重新凑话 = 换钥，这正是换钥规则）。
    pub async fn form_circle(&self) -> Result<usize, PersonError> {
        let (lines, my_r) = {
            let mut p = self.inner.lock().await;
            if p.convos.is_empty() {
                return Err(PersonError::NoConvo);
            }
            let mut r = vec![0u8; crate::circle::CONTRIB_LEN];
            getrandom::fill(&mut r).map_err(|_| PersonError::Greet("造不出凑话。".into()))?;
            p.gathering = Some(Gathering {
                my_r: r.clone(),
                contribs: HashMap::new(),
                key: None,
                counter: 0,
            });
            (p.convos.len(), r)
        };
        // 我的贡献从每条线广播出去
        broadcast_contribution(&self.inner, &my_r).await;
        Ok(lines + 1)
    }

    /// 在圈内说一句话：子钥加密一次，同一份群密文从每条线扇出。
    pub async fn circle_speak(&self, text: &str) -> Result<(), PersonError> {
        // 锁内：铸出本次的群密文（内层，逐线相同）
        let (blob, targets) = {
            let mut p = self.inner.lock().await;
            let Some(g) = p.gathering.as_mut() else {
                return Err(PersonError::NoCircle);
            };
            let Some(k) = g.key else {
                return Err(PersonError::NoCircle);
            };
            let counter = g.counter;
            g.counter += 1;
            let blob = crate::circle::seal(
                &crate::circle::derive_sender_subkey(&k, &g.my_r),
                counter,
                &g.my_r,
                text.as_bytes(),
            );
            let targets: Vec<_> = p
                .convos
                .values()
                .map(|c| (c.cipher.clone(), c.writer.clone()))
                .collect();
            (blob, targets)
        };
        // 锁外：逐线封 Noise 信封扇出（外层各不同，内层同一份）
        for (cipher, writer) in &targets {
            send_typed(cipher, writer, frame_kind::CIRCLE, &blob).await?;
        }
        Ok(())
    }
}

/// 凑话的「我说」：把贡献从每条线广播出去。
async fn broadcast_contribution(inner: &Arc<Mutex<Inner>>, my_r: &[u8]) {
    let targets: Vec<_> = {
        let p = inner.lock().await;
        p.convos
            .values()
            .map(|c| (c.cipher.clone(), c.writer.clone()))
            .collect()
    };
    for (cipher, writer) in targets {
        let _ = send_typed(&cipher, &writer, frame_kind::CONTRIB, my_r).await;
    }
}

/// 相遇：握手、互报家门、登记入册、竖起一只耳朵。
/// 主动去找的人自然成为注意力所在；不速之客不抢注意力。
async fn greet(
    inner: &Arc<Mutex<Inner>>,
    mut stream: TcpStream,
    as_initiator: bool,
    secret: Option<&[u8]>,
    peer_addr: String,
) -> Result<u64, GreetFailure> {
    let transport = handshake(&mut stream, as_initiator, secret)
        .await
        .map_err(|f| GreetFailure {
            err: PersonError::Greet(f.err.to_string()),
            burned: f.burned,
        })?;
    let (rh, wh) = stream.into_split();
    register_convo(
        inner,
        Box::new(rh),
        Box::new(wh),
        transport,
        as_initiator,
        peer_addr,
        secret.is_some(),
        true, // 直连线挂得住链：地址可作锚
    )
        .await
}

/// 握手之后的落座：互报家门、登记入册、竖起一只耳朵。
/// 读半写半走同一种号位——TCP 与打洞线在此无分彼此。
/// secret_used：这场握手是否花了一把暗号（决定后续失败时切口算不算消耗）；
/// chainable：这条线挂不挂得住暗号链（见 Convo::chainable）。
#[allow(clippy::too_many_arguments)] // 相遇的随身行李，各归各位，不好再并
async fn register_convo(
    inner: &Arc<Mutex<Inner>>,
    mut rh: BoxedReader,
    wh: BoxedWriter,
    transport: TransportState,
    as_initiator: bool,
    peer_addr: String,
    secret_used: bool,
    chainable: bool,
) -> Result<u64, GreetFailure> {
    let my_name = inner.lock().await.my_name.clone();
    let cipher = Arc::new(Mutex::new(transport));
    let writer = Arc::new(Mutex::new(wh));

    // 互报姓名：两边同时先发后收，全双工，不会死锁。
    // 名字也走密文（带类型字节）——它是应用数据，不该裸奔在线上，
    // 让茶馆和线路上的旁观者看热闹。报名字同样限时：握完手就哑巴的，不陪耗。
    let hello = async {
        send_typed(&cipher, &writer, frame_kind::NAME, my_name.as_bytes()).await?;
        let Some(ciphertext) =
            read_frame(&mut rh).await.map_err(|e| PersonError::Greet(e.to_string()))?
        else {
            return Ok(None);
        };
        let mut plain = vec![0u8; ciphertext.len()];
        let n = cipher
            .lock()
            .await
            .read_message(&ciphertext, &mut plain)
            .map_err(|_| PersonError::Greet("名字这帧没解开——信道有问题。".into()))?;
        if n == 0 || plain[0] != frame_kind::NAME {
            return Err(PersonError::Greet("对面没好好报名字。".into()));
        }
        Ok(Some(String::from_utf8_lossy(&plain[1..n]).into_owned()))
    };
    let their_name = match tokio::time::timeout(MEET_TIMEOUT, hello).await {
        Err(_) => {
            return Err(GreetFailure {
                err: PersonError::Greet("对面迟迟不报名字。".into()),
                burned: secret_used,
            })
        }
        Ok(Err(e)) => return Err(GreetFailure { err: e, burned: secret_used }),
        Ok(Ok(None)) => {
            return Err(GreetFailure {
                err: PersonError::Greet("对面还没开口就走了。".into()),
                burned: secret_used,
            })
        }
        Ok(Ok(Some(name))) => name,
    };

    let (id, focused) = {
        let mut p = inner.lock().await;
        let id = p.next_id;
        p.next_id += 1;
        p.convos.insert(
            id,
            Convo {
                their_name: their_name.clone(),
                chainable,
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
        ear(inner, id, rh, cipher, writer, their_name, peer_addr).await;
    });
    Ok(id)
}

/// 耳朵：收帧 → 解密 → 按帧类型处理。空明文帧 = 对方道别。
/// 这只耳朵合上时顺手收摊：摘对话、关线路、必要时挪回注意力。
async fn ear(
    inner: Arc<Mutex<Inner>>,
    id: u64,
    mut rh: BoxedReader,
    cipher: Arc<Mutex<TransportState>>,
    writer: Arc<Mutex<BoxedWriter>>,
    their_name: String,
    peer_addr: String,
) {
    let reason = loop {
        match read_frame(&mut rh).await {
            Ok(Some(ciphertext)) => {
                let mut plain = vec![0u8; ciphertext.len()];
                // 先把读结果取出：锁守卫若留在 match 里，
                // 下面回 ACK 时再锁同一把 cipher 就是自锁
                let read = cipher.lock().await.read_message(&ciphertext, &mut plain);
                match read {
                    Ok(0) => break LeaveReason::Farewell,
                    Ok(n) => match plain[0] {
                        frame_kind::CHAT | frame_kind::PADDED_CHAT => {
                            // 新旧两种排法都认：0x08 带长度前缀（垫直接丢弃），
                            // 0x00 是旧同伴的裸正文。
                            if let Some(text) = chat_text(plain[0], &plain[1..n]) {
                                let p = inner.lock().await;
                                p.events
                                    .send(Event::Heard {
                                        id,
                                        name: their_name.clone(),
                                        text,
                                    })
                                    .ok();
                            }
                        }
                        frame_kind::OFFER => {
                            // 对方在道别前预约下一把暗号：
                            // 收下当「等人的切口」，回一声确认。
                            // 不续链的人（开放台席）对预约装聋：不收、不确认——
                            // 对方的预约没人理会会自动作废，两边都只丢便利不丢安全。
                            // 切口位上已有话的（自己 /await 过）：同样不覆盖、不确认。
                            if n == 1 + 32 {
                                let mut p = inner.lock().await;
                                if p.chaining && p.expecting_secret.is_none() {
                                    p.expecting_secret = Some(plain[1..n].to_vec());
                                    p.events
                                        .send(Event::SecretChained {
                                            with: their_name.clone(),
                                        })
                                        .ok();
                                    drop(p);
                                    let _ = send_typed(&cipher, &writer, frame_kind::ACK, &[])
                                        .await;
                                }
                            }
                        }
                        frame_kind::ACK => {
                            // 对方确认收到了我们预约的暗号：入链，续上了
                            let mut p = inner.lock().await;
                            if let Some(x) = p.pending_offers.remove(&id) {
                                p.chained_secrets.insert(peer_addr.clone(), x);
                            }
                        }
                        frame_kind::CONTRIB => {
                            // 凑话：圈里的人把贡献送来了。
                            // 我自己还没凑过话的，现在凑上并广播（凑话自传染）；
                            // 收齐全部贡献即铸出群钥匙。
                            if n == 1 + crate::circle::CONTRIB_LEN {
                                let their = plain[1..n].to_vec();
                                let mut p = inner.lock().await;
                                let lines = p.convos.len();
                                let mut mine_to_send: Option<Vec<u8>> = None;
                                if p.gathering.is_none() {
                                    let mut r = vec![0u8; crate::circle::CONTRIB_LEN];
                                    if getrandom::fill(&mut r).is_ok() {
                                        mine_to_send = Some(r.clone());
                                        p.gathering = Some(Gathering {
                                            my_r: r,
                                            contribs: HashMap::new(),
                                            key: None,
                                            counter: 0,
                                        });
                                    }
                                }
                                if let Some(g) = p.gathering.as_mut() {
                                    let fresh = !g.contribs.contains_key(&id);
                                    g.contribs.insert(id, their);
                                    let need = lines; // 其余人各一份
                                    if fresh && g.key.is_none() && g.contribs.len() >= need {
                                        let mut all: Vec<Vec<u8>> =
                                            g.contribs.values().cloned().collect();
                                        all.push(g.my_r.clone());
                                        g.key = Some(crate::circle::derive_group_key(&all));
                                        p.events
                                            .send(Event::CircleFormed {
                                                members: all.len(),
                                            })
                                            .ok();
                                    }
                                }
                                // 自己刚凑上话的，把自己的贡献也广播出去（自传染）
                                if let Some(r) = mine_to_send {
                                    drop(p);
                                    broadcast_contribution(&inner, &r).await;
                                }
                            }
                        }
                        frame_kind::CIRCLE => {
                            // 屋里的话：用「这条线对面的贡献」派生的子钥解开。
                            // 解得开 = 说话的正是这条线的主人（线路即署名）。
                            let spoken = {
                                let p = inner.lock().await;
                                let Some(g) = p.gathering.as_ref() else {
                                    continue;
                                };
                                let Some(k) = g.key else {
                                    continue;
                                };
                                let Some(r) = g.contribs.get(&id) else {
                                    continue;
                                };
                                crate::circle::open(
                                    &crate::circle::derive_sender_subkey(&k, r),
                                    &plain[1..n],
                                )
                            };
                            if let Some(plain_text) = spoken {
                                let text = String::from_utf8_lossy(&plain_text).into_owned();
                                let p = inner.lock().await;
                                p.events
                                    .send(Event::Heard {
                                        id,
                                        name: their_name.clone(),
                                        text,
                                    })
                                    .ok();
                            } else {
                                // 解不开：不再无声吞掉——密钥不合（有人
                                // 的成员集和别家不一样）或密文被动过。
                                let p = inner.lock().await;
                                p.events
                                    .send(Event::CircleMumble {
                                        from: their_name.clone(),
                                    })
                                    .ok();
                            }
                        }
                        frame_kind::FILE_HEAD => {
                            // 对方递来一份文件的家门：名字＋总长。
                            // 超上限的婉拒（提醒人类，不中断会话，也不立户）；
                            // 合限的立户等块。已有在途的，让位给新的——
                            // 内存有界，且恶意的半截文件不该堵住正经的。
                            // 家门既要有名字又要有总长，缺一角就当没听见
                            if n > 1 && n >= 2 + plain[1] as usize + 8 {
                                let name_len = plain[1] as usize;
                                let name =
                                    String::from_utf8_lossy(&plain[2..2 + name_len]).into_owned();
                                let total = u64::from_be_bytes(
                                    plain[2 + name_len..2 + name_len + 8]
                                        .try_into()
                                        .expect("定长切８字节"),
                                );
                                let mut p = inner.lock().await;
                                if total > MAX_FILE_BYTES as u64 {
                                    p.inbound_files.remove(&id);
                                    p.events
                                        .send(Event::FileDeclined {
                                            from: their_name.clone(),
                                            name,
                                            size: total,
                                        })
                                        .ok();
                                } else if total == 0 {
                                    // 空文件：家门即全部，无需等块
                                    p.inbound_files.remove(&id);
                                    p.events
                                        .send(Event::FileArrived {
                                            id,
                                            name,
                                            data: Vec::new(),
                                        })
                                        .ok();
                                } else {
                                    p.inbound_files.insert(
                                        id,
                                        InboundFile {
                                            name,
                                            total,
                                            buf: Vec::with_capacity(total as usize),
                                            next_seq: 0,
                                        },
                                    );
                                }
                            }
                        }
                        frame_kind::FILE_CHUNK => {
                            // 一块字节进门：序号对得上才收；对不上（凭空来块、
                            // 乱序、越过报的总长）整份作废——TCP 上不会乱，
                            // 这是给坏实现与恶意对端的铁栏杆。
                            if n > 4 {
                                let seq =
                                    u32::from_be_bytes(plain[1..5].try_into().expect("定长切４字节"));
                                let chunk = plain[5..n].to_vec();
                                let mut arrived: Option<(String, Vec<u8>)> = None;
                                {
                                    let mut p = inner.lock().await;
                                    if let Some(f) = p.inbound_files.get_mut(&id)
                                        && seq == f.next_seq
                                        && f.buf.len() + chunk.len() <= f.total as usize
                                    {
                                        f.buf.extend_from_slice(&chunk);
                                        f.next_seq += 1;
                                        if f.buf.len() as u64 == f.total
                                            && let Some(done) = p.inbound_files.remove(&id)
                                        {
                                            arrived = Some((done.name, done.buf));
                                        }
                                    } else {
                                        // 序号不合或凭空来块：这份作废
                                        //（remove 对不存在的键是无害空操作）
                                        p.inbound_files.remove(&id);
                                    }
                                }
                                if let Some((name, data)) = arrived {
                                    let p = inner.lock().await;
                                    p.events
                                        .send(Event::FileArrived { id, name, data })
                                        .ok();
                                }
                            }
                        }
                        _ => { // 不认识的帧类型：沉默跳过，向前兼容
                        }
                    },
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
    // 线都断了，进出门的文件一并散伙（内存随之归还）
    p.inbound_files.remove(&id);
    p.outgoing_files.remove(&id);
    if p.focus == Some(id) {
        p.focus = p.convos.keys().max().copied();
        let to = p.focus;
        p.events.send(Event::FocusReturned { to }).ok();
    }
}

/// 加密并发送一帧。空明文帧 = 道别。
async fn send_frame(
    cipher: &Arc<Mutex<TransportState>>,
    writer: &Arc<Mutex<BoxedWriter>>,
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

/// 发送带类型前缀的加密帧。
async fn send_typed(
    cipher: &Arc<Mutex<TransportState>>,
    writer: &Arc<Mutex<BoxedWriter>>,
    kind: u8,
    payload: &[u8],
) -> Result<(), PersonError> {
    let mut plain = Vec::with_capacity(1 + payload.len());
    plain.push(kind);
    plain.extend_from_slice(payload);
    send_frame(cipher, writer, &plain).await
}

/// 发送一行聊天：0x08 ‖ 正文长 u16 ‖ 正文 ‖ 随机垫（0..=255 字节）。
/// 帧长不再紧贴正文长——「看包长猜话长」的长度关联被垫糊掉。垫被收方
/// 直接丢弃，无需对端任何配合；旧版同伴不认识 0x08，沉默跳过
/// （看不见这行，但也不显乱码）——两端同版本即无缝。
async fn send_chat(
    cipher: &Arc<Mutex<TransportState>>,
    writer: &Arc<Mutex<BoxedWriter>>,
    text: &[u8],
) -> Result<(), PersonError> {
    // 帧头共 3 字节、垫至多 255 字节——正文放不下 u16 就整帧装不下
    if text.len() > u16::MAX as usize - 258 {
        return Err(PersonError::Send(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "这一句太长，说不完",
        )));
    }
    let mut plain = Vec::with_capacity(3 + text.len() + 255);
    plain.push(frame_kind::PADDED_CHAT);
    plain.extend_from_slice(&(text.len() as u16).to_be_bytes());
    plain.extend_from_slice(text);
    // 随机垫：取不出随机数就发无垫（话照送，只是这一帧的形状不糊）
    let mut how_much = [0u8; 1];
    if getrandom::fill(&mut how_much).is_ok() && how_much[0] > 0 {
        plain.resize(plain.len() + how_much[0] as usize, 0);
    }
    send_frame(cipher, writer, &plain).await
}

/// 从聊天帧正文里取出要上屏的话。认两种排法：
/// 0x08＝长度前缀 u16 ‖ 正文 ‖ 随机垫（垫直接丢弃）；
/// 0x00＝旧同伴的裸正文（整段都是话）。
/// 前缀越界的 0x08 当没听见（坏实现与恶意对端的铁栏杆）。
fn chat_text(kind: u8, body: &[u8]) -> Option<String> {
    match kind {
        frame_kind::PADDED_CHAT => {
            if body.len() < 2 {
                return None;
            }
            let len = u16::from_be_bytes([body[0], body[1]]) as usize;
            body.get(2..2 + len)
                .map(|t| String::from_utf8_lossy(t).into_owned())
        }
        _ => Some(String::from_utf8_lossy(body).into_owned()),
    }
}

/// 告别前的暗号预约：现场造一把新暗号，发进这条已认证的信道。
/// 挂账待确认——收到对方的 ACK 才真正入链；道别后没人确认就作废。
async fn offer_next_secret(
    inner: &Arc<Mutex<Inner>>,
    convo_id: u64,
    cipher: &Arc<Mutex<TransportState>>,
    writer: &Arc<Mutex<BoxedWriter>>,
) {
    let mut x = [0u8; 32];
    if getrandom::fill(&mut x).is_err() {
        return; // 造不出暗号就算了：链是便利，不是必需
    }
    if send_typed(cipher, writer, frame_kind::OFFER, &x).await.is_err() {
        return; // 没送出去：作废
    }
    inner.lock().await.pending_offers.insert(convo_id, x.to_vec());
}

/// 见面三件套：
/// 1. 暗号（可选）——SPAKE2 对暗号：双方各发一条遮罩消息，只有持有
///    同一暗号的人能得出同一把点火钥匙。对不上则当场失败。
/// 2. 点火钥匙 —— 作为 PSK 喂入 Noise 握手（psk0 模式），只护送握手。
/// 3. Noise 混成握手 —— X25519 与 Kyber1024 两道锁同时上（后量子混成，
///    攻击者必须两道都破）。谈话用的钥匙全部来自本场全新的临时密钥。
///
/// TCP 与打洞线在此无分彼此：只要是条 AsyncRead+AsyncWrite 的线就行。
/// 每一件都限时（MEET_TIMEOUT）：连上却不吭声的，不陪他耗着。
async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    as_initiator: bool,
    secret: Option<&[u8]>,
) -> Result<TransportState, HandshakeFailure> {
    match handshake_inner(stream, as_initiator, secret).await {
        Ok(t) => Ok(t),
        // 带了暗号却握手失败，最可能的根因就是暗号对不上——直说，帮人诊断
        // （错误里本就提到暗号的，不重复加头）。
        Err(f) if secret.is_some() && !f.err.to_string().contains("暗号") => {
            Err(HandshakeFailure {
                err: format!("暗号可能对不上：{}", f.err).into(),
                burned: f.burned,
            })
        }
        Err(f) => Err(f),
    }
}

async fn handshake_inner<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    as_initiator: bool,
    secret: Option<&[u8]>,
) -> Result<TransportState, HandshakeFailure> {
    // 第 1 件：暗号。有则先对，得出点火钥匙；无则跳过。
    // 对暗号也限时：占着切口不说话的，等不得（超时不算查验，切口归还）。
    let psk = match secret {
        Some(secret) => {
            match tokio::time::timeout(MEET_TIMEOUT, pake(stream, as_initiator, secret)).await {
                Err(_) => return Err(HandshakeFailure::early("对暗号太慢，不等了。")),
                Ok(r) => Some(r?),
            }
        }
        None => None,
    };
    // 暗号一旦对过，这把就花掉了——无论后面的握手成不成
    let burned = psk.is_some();

    // 第 2、3 件：Noise（混成）。暗号在场用 psk0+hfs，不在场用 hfs。
    let suite = if psk.is_some() {
        NOISE_PARAMS_PSK
    } else {
        NOISE_PARAMS
    };
    let params: snow::params::NoiseParams = suite.parse().map_err(HandshakeFailure::early)?;
    let mut builder = Builder::new(params);
    if let Some(psk) = &psk {
        builder = builder.psk(0, psk).map_err(HandshakeFailure::early)?;
    }
    let hs: HandshakeState = if as_initiator {
        builder
            .build_initiator()
            .map_err(HandshakeFailure::early)?
    } else {
        builder
            .build_responder()
            .map_err(HandshakeFailure::early)?
    };

    match tokio::time::timeout(MEET_TIMEOUT, noise_stage(stream, hs, as_initiator)).await {
        Err(_) => Err(HandshakeFailure {
            err: "握手太慢，不等了。".into(),
            burned,
        }),
        Ok(r) => r.map_err(|e| HandshakeFailure { err: e, burned }),
    }
}

/// Noise 握手的收发（混成消息一往一来）。
/// 每条握手消息带一份 0..=255 字节的随机载荷：混成握手「恰是 1600/1632
/// 字节」的精确形状本身就是流量指纹，随机垫一垫就糊了。载荷对协议毫无
/// 意义（双方只取钥匙，不看载荷）——旧版本照常互通，无需协商。
async fn noise_stage<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    mut hs: HandshakeState,
    as_initiator: bool,
) -> Result<TransportState, Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = [0u8; 4096];

    // 随机垫：取不出随机数就发空垫（握手照常，只是形状不糊了）
    let mut pad = Vec::new();
    let mut len_b = [0u8; 1];
    if getrandom::fill(&mut len_b).is_ok() && len_b[0] > 0 {
        pad.resize(len_b[0] as usize, 0);
        let _ = getrandom::fill(&mut pad);
    }

    if as_initiator {
        let n = hs.write_message(&pad, &mut buf)?;
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
        let n = hs.write_message(&pad, &mut buf)?;
        write_frame(stream, &buf[..n]).await?;
    }

    Ok(hs.into_transport_mode()?)
}

/// 对暗号（SPAKE2）：双方各发一条遮罩消息。
/// 暗号相同 → 双方得到同一把 32 字节点火钥匙；
/// 暗号不同 → 钥匙不同，随后的 Noise 握手在数学上无法通过。
/// 暗号本身从不出现在线上。
/// 半路断线的失败（early）不算查验过暗号：切口可原样归还；
/// 遮罩都交换完、finish 跑过（对错都算）才算（checked）。
async fn pake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    as_initiator: bool,
    secret: &[u8],
) -> Result<[u8; 32], HandshakeFailure> {
    use spake2::{Ed25519Group, Identity, Password, Spake2};

    let password = Password::new(secret);
    // 身份留空：我们本来就不认身份，对上暗号即可
    let nobody = Identity::new(&[]);
    let (state, my_mask) = if as_initiator {
        Spake2::<Ed25519Group>::start_a(&password, &nobody, &nobody)
    } else {
        Spake2::<Ed25519Group>::start_b(&password, &nobody, &nobody)
    };

    // 遮罩消息一往一来：发起方先发，应答方先收
    let their_mask = if as_initiator {
        write_frame(stream, &my_mask)
            .await
            .map_err(HandshakeFailure::early)?;
        read_frame(stream)
            .await
            .map_err(HandshakeFailure::early)?
            .ok_or_else(|| HandshakeFailure::early("对暗号时对方不见了。"))?
    } else {
        let m = read_frame(stream)
            .await
            .map_err(HandshakeFailure::early)?
            .ok_or_else(|| HandshakeFailure::early("对暗号时对方不见了。"))?;
        write_frame(stream, &my_mask)
            .await
            .map_err(HandshakeFailure::early)?;
        m
    };
    let key: Vec<u8> = state
        .finish(&their_mask)
        .map_err(|_| HandshakeFailure::checked("暗号对不上。"))?;
    key.try_into()
        .map_err(|_| HandshakeFailure::checked("暗号钥匙长度不对。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 旧式聊天帧_整段都是话() {
        // 旧同伴的 0x00：裸正文，照常解读
        assert_eq!(
            chat_text(frame_kind::CHAT, "hua".as_bytes()).as_deref(),
            Some("hua")
        );
    }

    #[test]
    fn 带垫聊天帧_只取正文() {
        // 0x08：长度前缀之后、前缀说多少取多少——垫无论多长是什么，都该被丢
        let mut body = Vec::new();
        body.extend_from_slice(&3u16.to_be_bytes());
        body.extend_from_slice(b"hua");
        body.extend_from_slice(&[0xAB; 200]);
        assert_eq!(
            chat_text(frame_kind::PADDED_CHAT, &body).as_deref(),
            Some("hua")
        );
        // 无垫（垫长 0）也照常
        let mut bare = Vec::new();
        bare.extend_from_slice(&3u16.to_be_bytes());
        bare.extend_from_slice(b"hua");
        assert_eq!(
            chat_text(frame_kind::PADDED_CHAT, &bare).as_deref(),
            Some("hua")
        );
    }

    #[test]
    fn 带垫聊天帧_前缀越界_当没听见() {
        // 说有 10 字节正文、实际只有 1——坏帧，静默丢弃
        let mut body = Vec::new();
        body.extend_from_slice(&10u16.to_be_bytes());
        body.push(b'x');
        assert!(chat_text(frame_kind::PADDED_CHAT, &body).is_none());
        // 空头（连前缀都不全）同理
        assert!(chat_text(frame_kind::PADDED_CHAT, &[]).is_none());
        assert!(chat_text(frame_kind::PADDED_CHAT, &[0x00]).is_none());
    }
}
