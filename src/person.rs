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
use zeroize::Zeroizing;
use ed25519_dalek::Signer;

use crate::discover;
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
    /// 圈子铸成：名单上的名字全部自署到场，群钥匙在手，屋里的话开讲。
    /// circle_id = 哪一圈（P6 嵌套：一人可同时在多圈，事件按圈路由）。
    CircleFormed { circle_id: u64, members: usize },
    /// 圈里收到一句解不开的话（T1 响亮分歧：信念不同 ⇒ 钥匙不同）。
    /// v3 升格为核心感知：听见失败不是错误，是圈在告诉你你们的信念
    /// 分岔了——修复路径永远是点名（重跑 /circle）。
    CircleMumble { circle_id: u64, from: String },
    /// 圈子名单变了（有人离席/入席），新钥匙已就位。
    /// members = 变更后的圈内人数；change = 人话描述（谁进/谁走）。
    CircleRekeyed { circle_id: u64, members: usize, change: String },
    /// 响亮拒绝（静默=罪）：名单外的凑话、坏签名的告别/补话、
    /// r 碰撞……一切拒收都要被听见，detail 说人话。
    CircleRejected { circle_id: u64, detail: String },
    /// 圈内冲突（人类裁决）：在场辩护扣下的补话（还在说话的人不可
    /// 被判离开）、同名抢注对峙、期望门拟名不符……协议不裁决，
    /// 只响亮呈现。
    CircleConflict { circle_id: u64, detail: String },
    /// 有人给你看座（0x10 到达）：from = 引荐人（到达线对端名），
    /// table = 桌上的名字（席位骨架，无 r）。想坐就 /sit。
    SeatOffered { from: String, table: Vec<String> },
    /// 一份文件收齐了：字节原样都在内存里（data），无损。落不落盘、
    /// 落在哪，是外层使用者的事——库永不写盘。data 是 Zeroizing：
    /// 用后即焚（外层取走字节后本缓冲随 drop 清零）。
    FileArrived { id: u64, name: String, data: Zeroizing<Vec<u8>> },
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
    #[error("圈里已经有叫这个名字的人了。")]
    MemberNameTaken,
    #[error("这帧圈子里的话署名不对——有人拿别人的名义开口。")]
    CircleBadSignature,
    #[error("还没有人给你看座——等引荐人 /bring。")]
    NoSeatOffer,
    #[error("不止一份看座邀约：{detail}——先 /talk 切到那条线，再 /sit。")]
    AmbiguousSeatOffer { detail: String },
    #[error("没有可对词的信道：不在圈里，也没聚焦着的对话。")]
    NoSasChannel,
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
    /// 凑话（v1 圈）：其后是 32 字节随机贡献。v2 泛洪圈改用 0x09；
    /// 解析保留给旧同伴，本版不再发出。
    pub const CONTRIB: u8 = 0x03;
    /// 群聊（v1 圈）：其后是群密文。v2 改用 0x0A；解析已随 v1 圈语义
    /// 一并退役（泛洪圈不接待无署名群聊帧）——常量留档帧编号不复用。
    #[allow(dead_code)]
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
    /// 凑话（v3·自署行）：圈号 u64 ‖ 名单行（名长 u8 ‖ 名字 ‖ r 32B
    /// ‖ 嗓音公钥 32B ‖ 自署 64B）。行是自证的（contrib_sig 签
    /// circle_id‖名‖r‖sig_pk）——任何人伪造不了任何一行（P-04/P-05）。
    /// 计数字段已死：凑话完成由名单说了算（P-03 fix）。信念路由转发。
    pub const CONTRIB_V2: u8 = 0x09;
    /// 群聊（v3 不变）：圈号 u64 ‖ 发话者标签 4B ‖ 计数 u64
    /// ‖ 群密文 ‖ Ed25519 署名 64B。署名覆盖「圈号‖标签‖计数‖密文」；
    /// 验签用名单行里的嗓音公钥——子钥解密 + 公钥验签双闸（T3 嗓音完整性）。
    pub const CIRCLE_SIGNED: u8 = 0x0A;
    /// 补话（v3·见证人制）：圈号 u64 ‖ 'RM' ‖ 离场者（名长 u8‖名字）
    /// ‖ 见证人新行（自署整行，r 已旋转）‖ 见证人署名 64B（签
    /// circle_id‖'RM'‖离场者‖新行）。纪元死、选举死、级联死——
    /// 见证人 = 离开的直接感知者（A2），她换自己的 r、署自己的名。
    /// 幂等应用：条件删除（告退者在则删，不在则跳过）+ 无条件行替换。
    /// （ADD 变更留给入席流 0x10/0x11，布局镜像同构。）
    pub const TOPUP: u8 = 0x0B;
    /// 告退（v3·署名）：圈号 u64 ‖ 名长 u8 ‖ 名字 ‖ 告退署名 64B
    /// （签 circle_id‖名长‖名字）。告别是身体的行为（A1），必须署名
    /// ——无签名的告退一律拒收（P-01 fix）。一跳直接接收者是见证人，
    /// 转发收到者只记账不发起。
    pub const CIRCLE_BYE: u8 = 0x0C;
    /// 开台（v3·创世公告）：圈号 u64 ‖ 席位 u8（0 封桌 / 1 开台）
    /// ‖ 发起人（名长 u8‖名字）‖ 人数 u8 ‖ 邀请名单（每行 名长 u8‖名字）
    /// ‖ 发起人署名 64B（签除署名外全帧）。名单 = 发起者感知快照
    /// （事实陈述，非权力凭证）；凑话完成 = 名单上的名字全部自署到场
    /// （P-03 fix：计数死）。只走一跳（名单=发起者直连对端）。
    /// 点名 = 新圈号的重新 0x0F——与创世完全同构（A4）。
    pub const GENESIS: u8 = 0x0F;
    /// 看座（0x10）：圈号 u64 ‖ 席位 u8 ‖ 人数 u8 ‖（名长 u8 ‖ 名字 ‖
    /// 嗓音公钥 32B）×。1:1 帧（非圈帧）：引荐人把席位骨架递给候选人。
    /// 红线：绝不携带任何 r——拒入者拿到 r 即可推导群钥（P-09）。
    pub const SEAT_OFFER: u8 = 0x10;
    /// 落座署名（0x11）：一行（名长 u8 ‖ 名字 ‖ r 32B ‖ 嗓音公钥 32B
    /// ‖ 自署 64B，与 0x09 行同构）。候选人自选 r、自签整行，经 1:1
    /// 递回引荐人——落座署名必须是候选人本人的手势（A1：递话不代笔，
    /// P-04）。r 自选 + 收端唯一性响亮检查（P-12）。
    pub const SEAT_ACCEPT: u8 = 0x11;
    /// 座次表（0x12）：圈号 u64 ‖ 席位 u8 ‖ 人数 u8 ‖ 全部名单行
    /// （每行同 0x09 行布局，含 r）。1:1 帧：ADD 应用后引荐人把全表
    /// 递给新人（新人由此持全部 r 计算群钥）。只递已成成员（P-09）。
    pub const SEAT_ROSTER: u8 = 0x12;
    /// 线证明（0x13·P16，rulings.2026_10_17_p16_spec_approved）：圈号
    /// u64 ‖ 署名 64B，签名载荷 = circle_id ‖ 本线 transcript（见
    /// circle::proof_payload）。1:1 定向帧（非泛洪）：成员向自己的
    /// 每条绑定线发（凑话后/成键后/点名后，幂等三发）；收端按名单
    /// 行 sig_pk 逐个试验签，恰一行通过 ⇒ 该线锚升级 proven_pk
    ///（或未绑定线被授予 Proven——裁决点②）。per-line transcript
    /// 绑死本线，截获重放跨线必败。验签失败/无行可对 = 当没听见
    ///（无后果行为，静默语义与 bye-sig 去重同哲学）。
    pub const LINE_PROOF: u8 = 0x13;
}

/// 席位标志：多人成圈默认封桌（私人圆桌）；一人成圈默认开台（舞台）。
pub const SEATS_CLOSED: u8 = 0;
pub const SEATS_OPEN: u8 = 1;

/// 在场辩护窗口（挂钟 30 秒）：RM 点名者在窗口内有已验签发言
/// ⇒ 与 RM 矛盾 ⇒ 扣下补话 + 响亮冲突。「还在说话即不可被判离开」。
/// 帧无时间戳——到达时刻本地记账，不落盘、不进协议。
const PRESENCE_WINDOW: Duration = Duration::from_secs(30);

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

/// 一场围坐（v3·名单即圈）：圈不是对象，是 N 个局部信念的对齐。
///
/// 名单（roster）是圈的唯一状态：每行自署（身体的印，A1），名字归属
/// 由行内签名自证；我的名单是我的局部信念（A2）——没有权威副本，
/// 名单即路由表即世界观。凑话完成 = 创世名单上的名字全部自署到场
/// （计数死，P-03 fix）。
///
/// 信念路由：圈帧只走『对方名字在我当前信念名单上』的线 − 来源线 −
/// 除名者的线（P-02/P-04 fix）。连通性约束 = 成员导出子图连通。
///
/// 补话（0x0B·RM）：见证人 = 离开的直接感知者（A2：死线持有者 /
/// 署名告退的一跳接收者），她自己旋转 r、自署新行、签整帧——无纪元、
/// 无选举、无级联。幂等应用（条件删除 + 无条件行替换）保证多见证人
/// 收敛（T2 交换代数）；在场辩护（30 秒挂钟窗口）拦下对还在说话的
/// 人的驱逐（『还在说话即不可被判离开』）。
///
/// 线绑定（P-15 方向 a，rulings 2026_10_17_p15_line_binding_spec）：
/// 线→成员的资格映射只由协议行为建立，their_name（0x05 自报名）降为
/// 纯显示——路由/裁决/见证三类判定一律读绑定。
enum LineBinding {
    /// 名字绑定（窗口态）：该线对端=这名成员。仅两个可信写入者：
    /// form_circle 快照（发起者自己的线扫描——自洽感知宣誓，与署名
    /// genesis 同责）；complete_bring（0x11 自署行名，帧内可验）。
    /// P16（rulings.2026_10_17_p16_spec_approved）：proven_pk=Some 时
    /// 一切判定走 pk 锚（对端 0x13 已把她的身体印在这条线上）；
    /// None = 名字锚窗口态——仅成键前路由（凑话泛洪依赖它），
    /// **成键后严格档熄灭**（key.is_some() ⇒ 名字锚线不再承载圈帧，
    /// 谎报名线收创世+0x09 后一帧密封圈载荷也拿不到——窗口围栏）。
    Name {
        name: String,
        proven_pk: Option<ed25519_dalek::VerifyingKey>,
    },
    /// 圈线资格（窗口态）：0x0F/0x10/0x12 送达授予。双键（P-15 peer
    /// 键修复）+ P16 pk 锚（proven_pk 语义同 Name 变体）。
    Qualified {
        via: String,
        peer: String,
        proven_pk: Option<ed25519_dalek::VerifyingKey>,
    },
    /// pk 锚（P16 裁决点②）：0x13 到达**未绑定线**且验签通过 ⇒ 直接
    /// 授予（授予家族第三员：快照/送达/证明）。relay_asymmetry 的
    /// 半开边由此自然消解（B 主动发证明 ⇒ M 侧拿到 B 的 pk 锚）。
    Proven { pk: ed25519_dalek::VerifyingKey },
}

impl LineBinding {
    /// 线的 pk 锚（P16）：Some = 已锚定（判定一律按 pk——行在信念
    /// 即路由，行删即熄，名字复用/谎报名结构性不可混淆）；
    /// None = 名字锚窗口态（成键前名字判定，成键后严格档熄灭）。
    fn anchor_pk(&self) -> Option<&ed25519_dalek::VerifyingKey> {
        match self {
            LineBinding::Name { proven_pk, .. } => proven_pk.as_ref(),
            LineBinding::Qualified { proven_pk, .. } => proven_pk.as_ref(),
            LineBinding::Proven { pk } => Some(pk),
        }
    }

    /// 该绑定是否把线指向这名成员（窗口态判定：P-05 直连声明与
    /// 迟到告别的**名字锚回退路径**——pk 锚存在时判定方应优先
    /// anchor_pk 比较，此函数只服务锚缺席的窗口）。
    fn points_to(&self, name: &str) -> bool {
        match self {
            LineBinding::Name { name: n, .. } => n == name,
            LineBinding::Qualified { peer, .. } => peer == name,
            LineBinding::Proven { .. } => false, // pk 锚无名字语义
        }
    }

    /// 线是否取得圈路由资格（P16 统一判定）：
    /// - pk 锚：锚 pk 对应行在信念（members ∪ my_row 的 sig_pk 集）
    ///   ——行删即熄，单一机制；
    /// - 名字锚窗口态：成键前按名字过信念（凑话泛洪依赖）；成键后
    ///   （keyed=true）严格档熄灭——谎报名线的窗口围栏落地点。
    fn routes(
        &self,
        belief_names: &std::collections::HashSet<String>,
        belief_pks: &std::collections::HashSet<[u8; 32]>,
        keyed: bool,
    ) -> bool {
        match self.anchor_pk() {
            Some(pk) => belief_pks.contains(pk.as_bytes()),
            None => {
                if keyed {
                    return false; // 严格档：成键后名字锚线路由熄灭
                }
                match self {
                    LineBinding::Name { name, .. } => belief_names.contains(name),
                    LineBinding::Qualified { peer, .. } => belief_names.contains(peer),
                    LineBinding::Proven { .. } => unreachable!("Proven 恒有锚"),
                }
            }
        }
    }
}

/// 一人成圈：默认开台（舞台）——「想要观众就开个台」。
struct Gathering {
    /// 圈号（开台时随机 8B）——泛洪帧路由与去重的第一维；点名 = 新圈号
    circle_id: u64,
    /// 席位：0 封桌（多人圈默认）/ 1 开台（一人圈默认；开台时任何
    /// 成员的引荐即背书——入席流 slice-4 落地后生效，本版随帧记档）
    #[allow(dead_code)] // slice-4（入席流）读它；M1 只随帧传递
    seats: u8,
    /// 我的嗓音钥（一次性 Ed25519，私钥仅内存，随进程熄灭）
    sig: ed25519_dalek::SigningKey,
    /// 我自己的名单行（自署：名 ‖ r ‖ 公钥 ‖ 自署签名）
    my_row: crate::circle::RosterRow,
    /// 信念名单：名字 → 自署行（不含我；我的行是 my_row）
    members: HashMap<String, crate::circle::RosterRow>,
    /// 创世邀请名单（铸钥前 = 信念的一部分；铸钥后置 None，信念转为
    /// 成员）。名单上的名字全部自署到场才铸钥。
    invited: Option<std::collections::HashSet<String>>,
    /// 待验的开台公告（0x0F 除署名外的载荷）——发起人行到达时用她的
    /// 嗓音公钥补验。发起者天生已验（genesis_payload = None）。
    genesis_payload: Option<(String, Vec<u8>)>, // (发起人名, 除署名外全帧)
    /// 开台公告已验（发起者天生 true；被邀请者等发起人行到达时补验）
    genesis_verified: bool,
    /// 铸出后的群钥匙（信念一致时人人同钥；T1：分歧 ⇒ 互验必失败）
    key: Option<Zeroizing<[u8; 32]>>,
    /// 我的发言计数（nonce 唯一性；跨聚首不重置，防重放的唯一时间性来源）
    counter: u64,
    /// 在场辩护记账：名字 → 最近一次已验签发言的本地到达时刻。
    /// 本地挂钟、不落盘、不进协议（判定输入 = 本地证据 ∪ 任何线
    /// 到达的已验签发言——泛洪天然把在场证明送上门）。
    presence: HashMap<String, std::time::Instant>,
    /// 已见其署名告别的成员——RM 在场辩护的豁免条款（两条路径共用）
    seen_bye: std::collections::HashSet<String>,
    /// 嗓音墓碑（2026-10-17 裁决）：名字 → 公钥。apply_rm 除名时写入，
    /// 随圈生灭（点名重置、进程死亡）——A2 直接推导：我的信念记得她
    /// 走过，墓碑是信念的一部分，与名单同族非新物种。用途：迟到直接
    /// 告别帧的验签（RM 先到、名单已除名时，直接接收者经
    /// members ∪ left_voices 取公钥验签后照常发见证 RM——幂等应用
    /// 正是 rm_multiplicity 语义）。只存公钥非秘密——不 Zeroizing；
    /// 名字复用优先级：members 优先，ADD 应用时清除该名字的条目。
    left_voices: HashMap<String, ed25519_dalek::VerifyingKey>,
    /// 见证义务账（2026-10-17 裁决的工程面）：我已为其履行过见证 RM
    /// 的离场者名字——issue_witness_rm 的幂等守卫：同一离场只发一次
    /// RM（bye 路径与 crash 路径对同一人双触发时、告别帧重放时，此处
    /// 兜底空转）；ADD 重入时清除（新任期新义务）。随圈生灭。
    witnessed_departs: std::collections::HashSet<String>,
    /// 线绑定（P-15）：对话编号 → 绑定（见 LineBinding）。写入者仅三种
    /// 协议行为：form_circle 快照 / 0x0F·0x10·0x12 送达 / complete_bring。
    /// 0x09 与发言到达不绑线（负结果，rulings 2026_10_17）。their_name
    /// 从此不再进入任何路由/裁决/见证判定。
    line_bindings: HashMap<u64, LineBinding>,
}

impl Gathering {
    /// 我的信念名单（路由表=世界观）：成员 ∪（铸钥前）创世名单 ∪ 我自己。
    /// 圈帧只走名字在这张名单上的线（P-02 fix）。
    fn belief_names(&self) -> std::collections::HashSet<String> {
        let mut s: std::collections::HashSet<String> = self.members.keys().cloned().collect();
        if let Some(inv) = &self.invited {
            s.extend(inv.iter().cloned());
        }
        s.insert(self.my_row.name.clone());
        s
    }

    /// 信念的 pk 面（P16）：成员 ∪ 我的 sig_pk 集——pk 锚线的判定输入。
    /// 凑话期（invited 未转 members）行未到齐，锚线按 pk 自然不路由
    ///（她的行不在信念）——与名字锚的窗口语义对偶，无特判。
    fn belief_pks(&self) -> std::collections::HashSet<[u8; 32]> {
        let mut s: std::collections::HashSet<[u8; 32]> =
            self.members.values().map(|m| *m.sig_pk.as_bytes()).collect();
        s.insert(*self.my_row.sig_pk.as_bytes());
        s
    }

    /// 是否在圈内名单上（我自己的名字也算——我是圈的成员）。
    fn on_roster(&self, name: &str) -> bool {
        name == self.my_row.name
            || self.members.contains_key(name)
            || self.invited.as_ref().is_some_and(|i| i.contains(name))
    }

    /// r 唯一性检查（P-12）：与现有任何成员（含我）碰撞 = 响亮拒绝。
    /// 自选 r 下碰撞概率可忽略；恶意碰撞不许静默去重——那等于无声驱逐。
    fn r_collides(&self, r: &[u8]) -> bool {
        &self.my_row.r[..] == r || self.members.values().any(|m| &m.r[..] == r)
    }

    /// 在场辩护（RM 侧检查）：点名的成员在 30 秒挂钟窗口内有已验签
    /// 发言，且未见过其署名告别 ⇒ 与 RM 矛盾 ⇒ 扣下 + 响亮。
    /// 「还在说话即不可被判离开」；已见署名告别 → 照常应用（豁免）。
    fn presence_blocked(&self, leaver: &str) -> bool {
        if self.seen_bye.contains(leaver) {
            return false; // 她亲口告退——两条路径共用的豁免条款
        }
        self.presence
            .get(leaver)
            .is_some_and(|t| t.elapsed() < PRESENCE_WINDOW)
    }

    /// 名单到齐判定与铸钥（凑话路径共用）：创世名单上的名字（除我）
    /// 全部自署到场 → 铸钥、信念转为成员、返回人数。名字不参与铸钥
    /// ——贡献排序无关性是「无需成员名单」的根基。
    fn try_complete(&mut self) -> Option<usize> {
        if !self.genesis_verified || self.key.is_some() {
            return None;
        }
        let invited = self.invited.as_ref()?;
        let all_here = invited
            .iter()
            .filter(|n| **n != self.my_row.name)
            .all(|n| self.members.contains_key(n));
        if !all_here {
            return None;
        }
        let mut all: Vec<&[u8]> = self.members.values().map(|m| &m.r[..]).collect();
        all.push(&self.my_row.r[..]);
        let members = all.len();
        self.key = Some(Zeroizing::new(crate::circle::derive_group_key(&all)));
        self.invited = None; // 信念从「名单」转为「成员」
        Some(members)
    }

    /// 应用一次 RM（幂等语义，T2 收敛的规约来源）：
    /// 条件删除（告退者在则删、不在则跳过——不能因已删而丢弃后半帧）
    /// + 无条件行替换（见证人签名验证通过则换她的行）。
    /// 见证人 == 离场者时不替换（她自己已随删除离席——且这保住了
    /// 交换代数：先删后换会复活她，先换后删会漏删）。
    /// 除名时写嗓音墓碑（2026-10-17 裁决）：信念记得她走过——迟到
    /// 直接告别帧经墓碑验签，见证复活。
    /// 返回 (新人数, 变更人话)。
    fn apply_rm(&mut self, leaver: &str, witness_row: &crate::circle::RosterRow) -> (usize, String) {
        let removed_row = self.members.remove(leaver);
        let removed = removed_row.is_some();
        if let Some(row) = removed_row {
            self.left_voices.insert(leaver.to_string(), row.sig_pk);
        }
        // P-15（peer 键修复）+ P16（pk 锚统一熄灭）：除名时急切撤销——
        // ① Qualified{peer:leaver} 的线随除名熄灭（撤销键=线对端，
        // A3 封存的作用对象；RM 载荷携带见证人新 r，除名者续收任何
        // 圈帧即凑齐新 K 的全部输入——复审阻断项 B）；② 一切 pk 锚
        //（proven_pk/Proven）== 被删行 sig_pk 的线熄灭——行删⇒线熄
        // 的单一机制，名字复用（v1/v2 同名不同 pk）结构性不可混淆。
        // Name 名字锚不撤（成键后严格档在路由判定熄灭它；绑定本身
        // 留给迟到告别的窗口判定——points_to(离场者)）。
        let removed_pk = self.left_voices.get(leaver).copied();
        self.line_bindings.retain(|_, b| {
            if let Some(rpk) = &removed_pk
                && b.anchor_pk() == Some(rpk)
            {
                return false; // pk 锚随行删熄灭（P16）
            }
            !matches!(b, LineBinding::Qualified { peer, .. } if peer == leaver)
        });
        // 无条件行替换（见证人 != 离场者；== 我自己则更新 my_row）
        if witness_row.name != leaver {
            if witness_row.name == self.my_row.name {
                self.my_row = witness_row.clone();
            } else {
                self.members.insert(witness_row.name.clone(), witness_row.clone());
            }
        }
        self.seen_bye.remove(leaver); // 她走了，豁免条款完成使命
        self.presence.remove(leaver);
        let mut all: Vec<&[u8]> = self.members.values().map(|m| &m.r[..]).collect();
        all.push(&self.my_row.r[..]);
        let members = all.len();
        self.key = Some(Zeroizing::new(crate::circle::derive_group_key(&all)));
        let change = if removed {
            format!("{leaver} 离席")
        } else {
            format!("{leaver} 已不在名单（多见证人补话收敛）")
        };
        (members, change)
    }

    /// 圈内人数（含我）——测试用（断言名单规模）。
    #[cfg(test)]
    fn headcount(&self) -> usize {
        self.members.len() + 1
    }

    /// 应用一次 ADD（幂等语义，与 RM 镜像对称）：无条件插入新人行 +
    /// 无条件行替换（引荐人旋转后的行）。引荐人 == 新人不可能（名字
    /// 冲突已拒）——护栏照留（与 apply_rm 对称）。
    /// 名字复用清账（2026-10-17 裁决 ①）：同名重入 = 新任期——墓碑、
    /// 见证义务、在场豁免一并清除（旧任期的告别不得豁免/吞掉新任期）。
    /// 返回 (新人数, 变更人话)。
    fn apply_add(
        &mut self,
        newcomer: &crate::circle::RosterRow,
        introducer_row: &crate::circle::RosterRow,
    ) -> (usize, String) {
        self.left_voices.remove(&newcomer.name);
        self.witnessed_departs.remove(&newcomer.name);
        self.seen_bye.remove(&newcomer.name);
        // P-15：同名重入清账（作用域 = 该名的一切**窗口态**绑定，先于
        // 引荐人写入新绑定——complete_bring 在 apply_add 之后 insert，
        // 顺序由调用结构保证）——旧任期的 Name(她) 与 Qualified{peer:她}
        // 窗口绑定熄灭：旧身体的线不得因新身体入座而复明。P16 后此门
        // 降级为**防御性冗余**（pk 锚下 v1 的锚随 v1 行删已熄，v2 同名
        // 不同 pk 结构性不混淆——裁决书问 6；保留无害，删它反要论证）。
        // pk 锚（Proven/proven_pk）不按名字清——同名不同 pk 是不同身体。
        self.line_bindings.retain(|_, b| {
            !matches!(b, LineBinding::Name { name, proven_pk: None } if name == &newcomer.name)
                && !matches!(
                    b,
                    LineBinding::Qualified { peer, proven_pk: None, .. } if peer == &newcomer.name
                )
        });
        self.members.insert(newcomer.name.clone(), newcomer.clone());
        if introducer_row.name != newcomer.name {
            if introducer_row.name == self.my_row.name {
                self.my_row = introducer_row.clone();
            } else {
                self.members
                    .insert(introducer_row.name.clone(), introducer_row.clone());
            }
        }
        let mut all: Vec<&[u8]> = self.members.values().map(|m| &m.r[..]).collect();
        all.push(&self.my_row.r[..]);
        let members = all.len();
        self.key = Some(Zeroizing::new(crate::circle::derive_group_key(&all)));
        (members, format!("{} 入席", newcomer.name))
    }
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
    /// 见面的指纹（Noise transcript 哈希）：对词（/word·1:1 短词）的
    /// 派生源——两端相同 ⇒ 同词，拼线 ⇒ 异词（P-08）。
    /// Zeroizing：psk0 模式下 h 混入过暗号（MixKeyAndHash），按自持
    /// 秘密纪律包装；裸 NN 下它本是公开值，多包一层只多不亏。
    transcript: Zeroizing<[u8; 32]>,
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
    expecting_secret: Option<Zeroizing<Vec<u8>>>,
    /// 常备门禁（机器密钥形态）：每位来客都要对上才谈得成，且从不消耗。
    /// one-shot 的「烧」护的是低熵人类暗号（防在线爆破）；32 字节机器
    /// 密钥在线爆破本就不可能——于是错多少次都锁不住台，真钥永远进得来。
    /// 一次性切口（/await）若在，优先生效；烧掉之后门禁照常。
    stable_secret: Option<Zeroizing<Vec<u8>>>,
    /// 暗号链（主动侧）：地址 → 上一场告别时预约的下一把。用过即焚。
    chained_secrets: HashMap<String, Zeroizing<Vec<u8>>>,
    /// 挂起待确认的预约：对话编号 → 新暗号。收到确认才入链。
    pending_offers: HashMap<u64, Zeroizing<Vec<u8>>>,
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
    /// 早到的凑话暂存（乱序容忍：0x09 可能先于 0x0F 到达——打洞线上
    /// 帧即数据报，无序）。开台公告到达时按圈号补放。有界，纯内存。
    /// P-15：暂存带原始到达线号（补放的直连声明判定与转发排除读它）。
    pending_contribs:
        std::collections::VecDeque<(u64, Vec<u8>, u64)>, // (圈号, 帧体, 到达线)
    /// 早到的线证明暂存（P16 乱序容忍——pending_contribs 同族）：对端
    /// 凑话后的 0x13 紧随其 0x09/0x0F，可能抢在我 join 之前到达（圈
    /// 还不在）——丢弃会让她的凑话期证明失锚，成键后三发与用户手势
    /// （speak）竞速。join 后按圈号补验。有界，纯内存。
    pending_proofs: std::collections::VecDeque<(u64, [u8; 64], u64)>, // (圈号, 签名, 到达线)
    /// 泛洪去重：见过的（圈号 ‖ 凑话贡献 32B）与（圈号 ‖ 标签 ‖ 计数）。
    /// 上限保护见 FLOOD_SEEN_CAP——超过即整表重置（泛洪圈换钥后
    /// 旧键自然失效，重置只丢防重放记忆，不丢正确性）。
    flood_seen: std::collections::HashSet<[u8; 48]>,
    /// 待接的看座邀约（候选人侧）：对话编号 → 邀约（骨架+落座材料）。
    seat_offers: HashMap<u64, SeatOffer>,
    /// 在途的领人（引荐人侧）：候选人对话编号 → 圈上下文 + 期望门。
    pending_brings: HashMap<u64, PendingBring>,
}

/// 泛洪去重表上限：环形内存约束（每条 48B，上限 ~2.4MB）。
/// 真实圈聊速率下永远到不了；到得了也是该重围的信号。
const FLOOD_SEEN_CAP: usize = 50_000;

/// 早到凑话暂存上限（乱序容忍窗口）：每条 ~200B，上限 ~6.4KB。
const PENDING_CONTRIB_CAP: usize = 32;

/// 早到线证明暂存上限：每条 80B，上限 ~5KB。
const PENDING_PROOF_CAP: usize = 64;

/// 一份待接的看座邀约（候选人侧）：席位骨架（名字+嗓音公钥，绝无
/// r——P-09）+ 本人落座材料（/sit 之后才有，0x12 到达时用它建圈）。
struct SeatOffer {
    circle_id: u64,
    /// 席位（0x12 自带最新值——这里的留作 /sit 时的桌况参考）
    #[allow(dead_code)]
    seats: u8,
    /// 引荐人（到达线对端名）
    introducer: String,
    /// 桌上的名字+嗓音公钥（骨架——给人类看的桌子）
    #[allow(dead_code)]
    table: Vec<(String, [u8; 32])>,
    /// /sit 的落座材料：嗓音钥 + 自署行（等 0x12 时取回验身）
    my_seat: Option<(ed25519_dalek::SigningKey, crate::circle::RosterRow)>,
}

/// 一份在途的领人（引荐人侧）：圈上下文 + 期望门。
#[derive(Clone)]
struct PendingBring {
    circle_id: u64,
    /// 期望门：Some(拟名) 时，0x11 回执的自署名必须与拟名一致；
    /// None = 无门直接自动（她聚焦了这条线，她的直连感知就是验证）
    expected: Option<String>,
}

/// 一份正在进门的东西：名字、总长、已收的字节、下一块的序号。
/// buf 是明文（最大 32MB 驻留内存）——Zeroizing：收完交给事件后清零。
struct InboundFile {
    name: String,
    total: u64,
    buf: Zeroizing<Vec<u8>>,
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

/// /word 对词的报告（slice-5）：聚焦 1:1 的会话短词与（在圈时）
/// 群短词——可两者兼有。短词 12 位数字分 4 组（~40bit），即时计算
/// 零落盘；两边各看各的屏幕，想核对的人口头对一遍——不一样
/// = 这条线上有人拼话（P-08）。不对账是用户的自由（T5）。
#[derive(Debug)]
pub struct SasReport {
    /// (对端名, 短词)——聚焦的 1:1：Noise transcript 派生，两端相同
    pub p2p: Option<(String, String)>,
    /// (人数, 短词)——在圈的群短词：HKDF(K, "circle-sas") 派生，全员相同
    pub circle: Option<(usize, String)>,
}

impl Person {
    /// 出生：起个名字，守在一个地址上听。
    /// 返回这个人，和他的事件流。
    /// （3.0.0：局域网喊话耳朵已退役——出生只竖 TCP 这一只耳朵，
    /// 无邀请暴露源归零，removals_3_0_0.v1_discovery_family。）
    pub async fn born(name: &str, listen_addr: &str) -> Result<(Person, Events), PersonError> {
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
                pending_contribs: std::collections::VecDeque::new(),
                pending_proofs: std::collections::VecDeque::new(),
                flood_seen: std::collections::HashSet::new(),
                seat_offers: HashMap::new(),
                pending_brings: HashMap::new(),
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
                    let greet_secret = secret.as_ref().map(|s| &s[..]);
                    if let Err(f) =
                        greet(&inner, stream, false, greet_secret, peer_addr).await
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

        Ok((person, rx))
    }

    /// 这个人守着的地址（实际绑定；监听时端口填 0，这里给出真端口）。
    pub async fn listen_addr(&self) -> String {
        self.inner.lock().await.listen_addr.clone()
    }

    /// 备好切口：下一位来客必须对得上这句暗号才谈得成（一位一焚）。
    pub async fn expect_secret(&self, secret: &[u8]) {
        self.inner.lock().await.expecting_secret = Some(Zeroizing::new(secret.to_vec()));
    }

    /// 立起常备门禁：**每一位**来客都要对上这句暗号才谈得成，且从不消耗——
    /// 错多少次、进多少次，门禁都还立在门口，真钥永远进得来。
    /// 这句应是 32 字节级的随机机器密钥（客户端内嵌），不是人类暗号：
    /// 一次性切口的「烧」防的是对低熵暗号的在线爆破，机器密钥在线爆破
    /// 本就不可能，不需要那份保护——于是瞎拨锁不住台（DoS 无效）。
    pub async fn expect_secret_stable(&self, secret: &[u8]) {
        self.inner.lock().await.stable_secret = Some(Zeroizing::new(secret.to_vec()));
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
        let chained_ref = chained.as_ref().map(|s| &s[..]);
        match self.dial_inner(addr, chained_ref).await {
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
        let (transport, transcript) = handshake(&mut stream, as_initiator, secret)
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
            transcript,
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
        let (transport, transcript) = handshake(&mut hole, as_initiator, secret)
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
            transcript,
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

    /// 和当前对话的人道别。收摊由耳朵任务自己完成。
    /// 道别之前，先把下一把暗号预约进这条已认证的信道——
    /// 对方确认了，链就续上了；没等到确认，预约作废（丢的是便利，不是安全）。
    /// 茶馆线不预约（锚不存在，见 Convo::chainable）。
    /// v2：若在圈，先泛洪一圈告退帧（0x0C）——触发全员补话换钥。
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
        // 圈子告退（在圈才发）
        self.circle_bye_frames().await;
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
    /// v2：先向所有在场的圈子泛洪告退。
    pub async fn leave(&self) {
        self.circle_bye_frames().await;
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

    /// 圈子告退（0x0C·署名）：告别是身体的行为（A1），用嗓音钥签
    /// （圈号‖名长‖名字）——坏签名一律被拒收（P-01 fix）。发完即散：
    /// 离开者读不了新话（换气+断递，A3），本地名单随手放下。
    async fn circle_bye_frames(&self) {
        let (targets, frame) = {
            let mut p = self.inner.lock().await;
            let Some(g) = p.gathering.as_ref() else {
                return;
            };
            // 圈语境的「我的名字」= 名单行名（/sit 覆盖落座名后，
            // p.my_name 只是 1:1 自报名——告别必须签名单行名，否则
            // 收端在名单里找不到这个名字）
            let my_roster_name = g.my_row.name.clone();
            let mut v = Vec::with_capacity(8 + 1 + my_roster_name.len() + 64);
            v.extend_from_slice(&g.circle_id.to_be_bytes());
            v.push(my_roster_name.len() as u8);
            v.extend_from_slice(my_roster_name.as_bytes());
            let sig = g.sig.sign(&v);
            v.extend_from_slice(&sig.to_bytes());
            // 快照发送对象后即散（gathering 一放，信念名单就没了——
            // 散伙的告别按当时的锚定线发出）。P16：targets 按锚判定
            //（pk 锚=锚行在信念；名字锚窗口态=成键前名字在信念）——
            // their_name 从判定路径退役（全仓最后一处残迹，spec 问 5）
            let belief = g.belief_names();
            let belief_pks = g.belief_pks();
            let keyed = g.key.is_some();
            let targets: Vec<_> = p
                .convos
                .iter()
                .filter(|(id, _)| {
                    g.line_bindings
                        .get(id)
                        .is_some_and(|b| b.routes(&belief, &belief_pks, keyed))
                })
                .map(|(_, c)| (c.cipher.clone(), c.writer.clone()))
                .collect();
            p.gathering = None;
            (targets, v)
        };
        for (cipher, writer) in targets {
            let _ = send_typed(&cipher, &writer, frame_kind::CIRCLE_BYE, &frame).await;
        }
    }

    /// 开台 / 围坐 / 点名（v3·创世与点名同构）：扫描当前会话 →
    /// 署名邀请名单 → 0x0F 只走一跳 → 自署行泛洪。席位默认：多人封桌、
    /// 一人开台。返回名单人数（含发起者）。
    pub async fn form_circle(&self) -> Result<usize, PersonError> {
        self.form_circle_seats(None).await
    }

    /// 同 [`Person::form_circle`]，但可显式覆盖席位（封桌/开台）。
    /// 「/circle 可显式覆盖」——开放台是刻意的宣言，不是默认。
    pub async fn form_circle_seats(&self, seats_override: Option<u8>) -> Result<usize, PersonError> {
        let (invited_count, genesis_frame, contrib_frame, solo_formed, circle_id) = {
            let mut p = self.inner.lock().await;
            // 名单 = 发起者感知快照：在场对话对端 + 我（事实陈述，非权力凭证）。
            // P-15 快照绑定：扫描自己的线（自洽感知宣誓，与署名 genesis
            // 同责）——每条线按其对话对端名写 Name 绑定。their_name 在
            // 此仅作为「我对这条线的称呼」入档（发起者的直接感知），
            // 不再进入后续任何路由判定。
            let mut names: Vec<String> = p
                .convos
                .values()
                .map(|c| c.their_name.clone())
                .collect();
            names.sort();
            names.dedup();
            let snapshot_bindings: HashMap<u64, LineBinding> = p
                .convos
                .iter()
                .map(|(id, c)| {
                    (
                        *id,
                        LineBinding::Name { name: c.their_name.clone(), proven_pk: None },
                    )
                })
                .collect();
            let mut invited: std::collections::HashSet<String> =
                names.iter().cloned().collect();
            let my_name = p.my_name.clone();
            // 名单含发起者本人（凑话完成判定按「除我以外的名字到齐」）。
            // 若对端恰与我同名（奇观），名单去重后仍以我为准——
            // 同名行到达时走见证裁决（P-05）。
            invited.insert(my_name.clone());
            let seats = seats_override.unwrap_or(if invited.len() >= 2 {
                SEATS_CLOSED // 多人成圈默认封桌（私人圆桌）
            } else {
                SEATS_OPEN // 一人成圈默认开台（舞台，等人来的宣言）
            });
            let mut idb = [0u8; 8];
            getrandom::fill(&mut idb)
                .map_err(|_| PersonError::Greet("造不出圈号。".into()))?;
            let circle_id = u64::from_be_bytes(idb);
            let mut seed = Zeroizing::new([0u8; 32]);
            getrandom::fill(&mut seed[..])
                .map_err(|_| PersonError::Greet("造不出嗓音钥。".into()))?;
            let sig = ed25519_dalek::SigningKey::from_bytes(&seed);
            let my_row = crate::circle::RosterRow::self_seat(circle_id, &my_name, &sig);
            // 回声免疫：自己的贡献键先记下（泛洪回来的 0x09 直接丢弃）
            let fk = flood_key(circle_id, &my_row.r[..]);
            p.flood_seen.insert(fk);
            // 创世公告载荷（除署名外全帧）——发起者自签即验
            let mut payload = Vec::new();
            payload.extend_from_slice(&circle_id.to_be_bytes());
            payload.push(seats);
            payload.push(my_name.len() as u8);
            payload.extend_from_slice(my_name.as_bytes());
            payload.push(invited.len() as u8);
            for n in &invited {
                payload.push(n.len() as u8);
                payload.extend_from_slice(n.as_bytes());
            }
            let genesis_sig = sig.sign(&payload);
            payload.extend_from_slice(&genesis_sig.to_bytes());
            let invited_count = invited.len();
            p.gathering = Some(Gathering {
                circle_id,
                seats,
                sig: sig.clone(),
                my_row: my_row.clone(),
                members: HashMap::new(),
                invited: Some(invited),
                genesis_payload: None,
                genesis_verified: true,
                key: None,
                counter: 0,
                presence: HashMap::new(),
                seen_bye: std::collections::HashSet::new(),
                left_voices: HashMap::new(),
                witnessed_departs: std::collections::HashSet::new(),
                line_bindings: snapshot_bindings,
            });
            let mut contrib = Vec::with_capacity(8 + 130 + my_name.len());
            contrib.extend_from_slice(&circle_id.to_be_bytes());
            contrib.extend_from_slice(&my_row.to_bytes());
            // 一人圈：名单只有我，立即成钥（开台，纯等入席流）
            let solo_formed = p
                .gathering
                .as_mut()
                .and_then(Gathering::try_complete);
            (invited_count, payload, contrib, solo_formed, circle_id)
        };
        if let Some(members) = solo_formed {
            self.inner
                .lock()
                .await
                .events
                .send(Event::CircleFormed { circle_id, members })
                .ok();
        }
        // 开台公告只走一跳：名单=发起者直连对端。扇出追随誓言——
        // 只走刚写下的快照绑定集（rulings.2026_10_17_flood_frame_all_migration：
        // 全对话泛洪家族的最后残骸已删；漂移窗内新到的线得零帧而非
        // 一帧孤儿 0x0F——她的名字本不在署名名单上，结构上更诚实）
        belief_flood(&self.inner, None, frame_kind::GENESIS, &genesis_frame).await;
        // 我的自署行泛洪出去（一人圈 = 无线可发，已就地成钥）
        belief_flood(&self.inner, None, frame_kind::CONTRIB_V2, &contrib_frame).await;
        // P16 三发之一（凑话后——点名重开桌走同一代码路径，即「点名后」
        // 时机）：向自己的每条绑定线递身体印（幂等；一人圈无线可发）
        send_line_proofs(&self.inner).await;
        Ok(invited_count)
    }

    /// 在圈内说一句话（v3）：子钥加密 + 嗓音钥签名，信念路由扇出。
    /// 收方双闸：子钥解密（只有贡献对的人的子钥能开）＋公钥验签
    /// （只有名单行里公布这个公钥的人能签）——署名与路径无关（T3）。
    pub async fn circle_speak(&self, text: &str) -> Result<(), PersonError> {
        let frame = {
            let mut p = self.inner.lock().await;
            let Some(g) = p.gathering.as_mut() else {
                return Err(PersonError::NoCircle);
            };
            let Some(k) = g.key.as_ref() else {
                return Err(PersonError::NoCircle);
            };
            let subkey = crate::circle::derive_sender_subkey(k, &g.my_row.r[..]);
            let counter = g.counter;
            g.counter += 1;
            let blob = crate::circle::seal(&subkey, counter, &g.my_row.r[..], text.as_bytes());
            let tag = crate::circle::sender_tag_of(&g.my_row.r[..]);
            let mut payload = Vec::with_capacity(12 + blob.len() + 64);
            payload.extend_from_slice(&g.circle_id.to_be_bytes());
            payload.extend_from_slice(&tag);
            payload.extend_from_slice(&counter.to_be_bytes());
            payload.extend_from_slice(&blob);
            let sig = g.sig.sign(&payload);
            payload.extend_from_slice(&sig.to_bytes());
            payload
        };
        // 锁外：信念路由扇出（圈帧只走名单上的线）
        belief_flood(&self.inner, None, frame_kind::CIRCLE_SIGNED, &frame).await;
        Ok(())
    }

    /// 领人入座（/bring·引荐人的手势）：向聚焦对端（候选人）发 0x10
    /// 看座——席位骨架（名字+嗓音公钥，绝无 r，P-09）。带拟名则开
    /// 期望门（0x11 自署名不匹配 → 响亮暂停）；不带 = 无门直接自动
    /// （她聚焦了这条线，她的直连感知就是她的验证，A2 允许）。
    /// 封桌时照发——拒绝是全员契约行为，不是发起人的自我审查。
    /// 返回席位标志（封桌/开台），外层可据此警告。
    pub async fn bring(&self, expected: Option<&str>) -> Result<u8, PersonError> {
        let (convo_id, cipher, writer, frame, seats) = {
            let mut p = self.inner.lock().await;
            let Some(id) = p.focus else {
                return Err(PersonError::NoFocus);
            };
            let Some(c) = p.convos.get(&id) else {
                return Err(PersonError::NoConvo);
            };
            let (cipher, writer) = (c.cipher.clone(), c.writer.clone());
            // 借用分两段：从 gathering 取值（不可变）→ 挂账（可变）
            let (frame, seats, circle_id) = {
                let Some(g) = p.gathering.as_ref() else {
                    return Err(PersonError::NoCircle);
                };
                if g.key.is_none() {
                    return Err(PersonError::NoCircle); // 还没铸成钥：这张桌还没开讲
                }
                // 看座骨架：圈号 ‖ 席位 ‖ 人数 ‖（名长‖名‖嗓音公钥）——无 r！
                let count = g.members.len() + 1;
                if count > u8::MAX as usize {
                    return Err(PersonError::Greet("这张桌太大了，看座帧装不下。".into()));
                }
                let mut frame = Vec::with_capacity(10 + count * 40);
                frame.extend_from_slice(&g.circle_id.to_be_bytes());
                frame.push(g.seats);
                frame.push(count as u8);
                for row in g.members.values().chain(std::iter::once(&g.my_row)) {
                    frame.push(row.name.len() as u8);
                    frame.extend_from_slice(row.name.as_bytes());
                    frame.extend_from_slice(row.sig_pk.as_bytes());
                }
                (frame, g.seats, g.circle_id)
            };
            p.pending_brings.insert(
                id,
                PendingBring {
                    circle_id,
                    expected: expected.map(|s| s.to_string()),
                },
            );
            (id, cipher, writer, frame, seats)
        };
        // 没送出去 = 手势没发生：撤销挂账（每次手势的协议效果完整）
        if let Err(e) = send_typed(&cipher, &writer, frame_kind::SEAT_OFFER, &frame).await {
            self.inner.lock().await.pending_brings.remove(&convo_id);
            return Err(e);
        }
        Ok(seats)
    }

    /// 落座（/sit·候选人的手势，A1：落座署名必须本人触发）：本人自选
    /// r、生成嗓音钥、自签整行（0x11）递回引荐人。默认取本人 0x05 自
    /// 报名（你的名字是你已经说过的话），可覆盖（「这桌叫我阿花」）。
    /// 多份邀约：聚焦线上有则用之，唯一则用之，多个则响亮列候选。
    pub async fn sit(&self, name_override: Option<&str>) -> Result<(), PersonError> {
        let (cipher, writer, row_bytes) = {
            let mut p = self.inner.lock().await;
            let my_name = p.my_name.clone();
            // 选邀约：聚焦线优先 → 唯一 → 歧义列候选（/talk 模式）
            let pick = if let Some(f) = p.focus.filter(|f| p.seat_offers.contains_key(f)) {
                Some(f)
            } else if p.seat_offers.len() == 1 {
                p.seat_offers.keys().next().copied()
            } else if p.seat_offers.is_empty() {
                return Err(PersonError::NoSeatOffer);
            } else {
                None // 多份邀约，歧义
            };
            let Some(pick) = pick else {
                let detail = p
                    .seat_offers
                    .iter()
                    .map(|(id, o)| format!("#{id} {}", o.introducer))
                    .collect::<Vec<_>>()
                    .join("、");
                return Err(PersonError::AmbiguousSeatOffer { detail });
            };
            // 落座材料：新鲜嗓音钥 + 自署行（每次 /sit 全新——重试不留旧材料）
            let circle_id = p.seat_offers.get(&pick).map(|o| o.circle_id);
            let Some(circle_id) = circle_id else {
                return Err(PersonError::NoSeatOffer);
            };
            let name = name_override.unwrap_or(&my_name);
            let mut seed = Zeroizing::new([0u8; 32]);
            getrandom::fill(&mut seed[..])
                .map_err(|_| PersonError::Greet("造不出嗓音钥。".into()))?;
            let sig = ed25519_dalek::SigningKey::from_bytes(&seed);
            let row = crate::circle::RosterRow::self_seat(circle_id, name, &sig);
            if let Some(offer) = p.seat_offers.get_mut(&pick) {
                offer.my_seat = Some((sig, row.clone()));
            }
            let row_bytes = row.to_bytes();
            let Some(c) = p.convos.get(&pick) else {
                return Err(PersonError::NoConvo);
            };
            (c.cipher.clone(), c.writer.clone(), row_bytes)
        };
        send_typed(&cipher, &writer, frame_kind::SEAT_ACCEPT, &row_bytes).await
    }

    /// 对词（/word·slice-5）：把信道指纹蒸成 12 位数字短词。
    /// 聚焦 1:1 → 会话短词（transcript 派生，两端相同）；在圈 →
    /// 群短词（HKDF(K, "circle-sas") 派生，全员相同）；两者兼有时
    /// 两个都报（两条信道都活着，各自可核对）。即时计算、零落盘——
    /// 无身份层的一部分：对词是给愿意谨慎的人的可选仪式，不是检测器。
    pub async fn sas_word(&self) -> Result<SasReport, PersonError> {
        let p = self.inner.lock().await;
        let p2p = p
            .focus
            .and_then(|id| p.convos.get(&id))
            .map(|c| {
                (
                    c.their_name.clone(),
                    crate::circle::sas_word_from_transcript(&c.transcript[..]),
                )
            });
        let circle = p.gathering.as_ref().and_then(|g| {
            g.key.as_ref().map(|k| {
                (
                    g.members.len() + 1,
                    crate::circle::sas_word_from_group_key(k),
                )
            })
        });
        if p2p.is_none() && circle.is_none() {
            return Err(PersonError::NoSasChannel);
        }
        Ok(SasReport { p2p, circle })
    }
}

/// 信念路由（v3·P-02/P-04 fix）：圈帧只走『对方名字在我当前信念
/// 名单上』的线 − 来源线 − 除名者的线。名单即路由表即世界观（A2）。
/// exclude = None 即发起；Some(id) 即转发（来源线除外）。
/// 圈不在（或圈号对不上）= 一条线都不该走。
async fn belief_flood(
    inner: &Arc<Mutex<Inner>>,
    exclude: Option<u64>,
    kind: u8,
    payload: &[u8],
) {
    let circle_id = u64::from_be_bytes(payload[..8].try_into().unwrap_or([0; 8]));
    let targets: Vec<_> = {
        let p = inner.lock().await;
        let Some(g) = p.gathering.as_ref() else {
            return;
        };
        if g.circle_id != circle_id {
            return; // 不是我信念里的圈：一句都不转发
        }
        let belief = g.belief_names();
        let belief_pks = g.belief_pks();
        // P-16 严格档只封**密封**圈载荷（0x0A 发言/0x0B 变更——谎报名
        // 线的窗口围栏）；0x09 凑话按窗口规则路由**无论成键与否**：
        // ①围栏裁决明说窗口内谎报名线可收创世+0x09 行（已接受价格，
        // 遏制=密封帧零到达）；②凑话期语义跨成员错位（我已收齐成键、
        // 她还在等行）——成键者转发迟到凑话是 T2 乱序的合法场景，
        // 若受严格档会因对端 0x13 未到而断递（跨线不保序竞态，点名
        // 后 mesh 收齐 flaky 的根因）。
        let keyed = g.key.is_some() && kind != frame_kind::CONTRIB_V2;
        // P-16（rulings.2026_10_17_p16_spec_approved）：路由判定统一走
        // LineBinding::routes——pk 锚线按「锚 pk 行在信念」（行删即熄，
        // 单一机制）；名字锚窗口态仅成键前路由（凑话泛洪依赖），
        // 成键后严格档熄灭（谎报名线的窗口围栏：创世+0x09 可达、
        // 一帧密封圈载荷不可达）。their_name 不进入任何判定。
        p.convos
            .iter()
            .filter(|(id, _)| {
                Some(**id) != exclude
                    && g.line_bindings
                        .get(id)
                        .is_some_and(|b| b.routes(&belief, &belief_pks, keyed))
            })
            .map(|(_, c)| (c.cipher.clone(), c.writer.clone()))
            .collect()
    };
    for (cipher, writer) in targets {
        let _ = send_typed(&cipher, &writer, kind, payload).await;
    }
}

/// 线证明发送（P16·0x13，rulings.2026_10_17_p16_spec_approved）：
/// 向自己的每条绑定线递身体印——签名载荷 = circle_id ‖ 该线 transcript
///（per-line：证明绑死在这条线上，截获跨线重放必败）。幂等三发点：
/// 凑话后（join_genesis / form_circle 尾）、成键后（try_complete /
/// 座次表入座）、点名后（= form_circle 同路径）。无确认机制——确认
/// 是同步机（A2）；丢帧的代价是线停留名字锚 ⇒ 成键后熄灭 ⇒ mumble
/// ⇒ 点名恢复（A4 兜底闭环）。绑定线含窗口名字锚线（对端行未到也
/// 发——她收到时无行可验则丢弃，行到后三发点会再来）。
async fn send_line_proofs(inner: &Arc<Mutex<Inner>>) {
    // 锁内一次取定：圈号、嗓音 sk、每条绑定线的 transcript 与出线
    //（锁外签名发送——sk 的本地副本用后即弃，Zeroizing 纪律照旧）
    let (circle_id, sig, lines): (
        u64,
        ed25519_dalek::SigningKey,
        Vec<(
            Zeroizing<[u8; 32]>,
            Arc<Mutex<TransportState>>,
            Arc<Mutex<BoxedWriter>>,
        )>,
    ) = {
        let p = inner.lock().await;
        let Some(g) = p.gathering.as_ref() else {
            return;
        };
        (
            g.circle_id,
            g.sig.clone(),
            p.convos
                .iter()
                .filter(|(id, _)| g.line_bindings.contains_key(id))
                .map(|(_, c)| {
                    (
                        Zeroizing::new(*c.transcript),
                        c.cipher.clone(),
                        c.writer.clone(),
                    )
                })
                .collect(),
        )
    };
    // 发送途中圈可能已换（点名竞态）：旧 circle_id 签出的证明在新圈
    // 验不过（收端按当前名单试钥、按当前圈号对账）——无害，新圈的
    // 三发点会重发
    for (transcript, cipher, writer) in lines {
        let payload = crate::circle::proof_payload(circle_id, &transcript);
        let sig_bytes = sig.sign(&payload).to_bytes();
        let mut frame = Vec::with_capacity(8 + 64);
        frame.extend_from_slice(&circle_id.to_be_bytes());
        frame.extend_from_slice(&sig_bytes);
        let _ = send_typed(&cipher, &writer, frame_kind::LINE_PROOF, &frame).await;
    }
}

/// 见到过的泛洪帧键：圈号 ‖ ≤40B 去重主体（凑话用 r、发言用 tag‖counter、
/// 补话用见证人新 r、告别用签名——sig 即事件 ID，Ed25519 确定性使同事件
/// 重放同 sig 被去重、重入后新钥新 sig 放行；2026-10-17 裁决取代
/// 早先『告别用名字』的预留——裸名字在离场→重入→再离场下吞掉第二次
/// 告别。各自语义见 ear）。
fn flood_key(circle_id: u64, body: &[u8]) -> [u8; 48] {
    let mut k = [0u8; 48];
    k[..8].copy_from_slice(&circle_id.to_be_bytes());
    let n = body.len().min(40);
    k[8..8 + n].copy_from_slice(&body[..n]);
    k
}

/// 0x0C 告退的签名载荷（定界防拼接歧义）。
fn bye_payload(circle_id: u64, name: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + 1 + name.len());
    v.extend_from_slice(&circle_id.to_be_bytes());
    v.push(name.len() as u8);
    v.extend_from_slice(name);
    v
}

/// 0x0B RM 的见证人签名载荷：circle_id ‖ 'RM' ‖ 离场者 ‖ 见证人新行。
fn rm_payload(circle_id: u64, leaver: &[u8], witness_row: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + 1 + 1 + leaver.len() + witness_row.len());
    v.extend_from_slice(&circle_id.to_be_bytes());
    v.push(b'R');
    v.push(leaver.len() as u8);
    v.extend_from_slice(leaver);
    v.extend_from_slice(witness_row);
    v
}

/// 见证人补话发起（RM 路径，v3）：我是离开的直接感知者——死线持有者
/// （crash 路径）或署名告退的一跳直接接收者（自愿路径）。我自己旋转
/// r、自署新行、签整帧；无纪元、无选举、无级联。在场辩护先行：点名的
/// 成员还在说话（30 秒窗口内已验签发言且未见其署名告别）→ 扣下不补
/// + 响亮冲突，人类裁决（点名）。帧丢失 → 僵尸成员留在旧钥，直到谁的
/// 线死再走一次见证路径（no_liveness_probes 既有语义）。
async fn issue_witness_rm(inner: &Arc<Mutex<Inner>>, circle_id: u64, leaver: &str) {
    let frame = {
        let mut p = inner.lock().await;
        // 借用分三段：检查+造帧（不可变）→ 去重（无借用）→ 应用（独占）
        let (frame, witness_row, blocked) = {
            let Some(g) = p.gathering.as_ref() else {
                return;
            };
            // 圈语境的「我」= 名单行名（/sit 覆盖落座名后同构）
            if g.circle_id != circle_id || leaver == g.my_row.name {
                return;
            }
            // 见证义务幂等守卫（2026-10-17）：同一离场只发一次 RM——
            // 迟到告别的直接副本、crash 路径的线死、告别帧重放，三条
            // 触发路径可能对同一人先后到达；已履行过 = 安静空转。
            // ADD 重入时 apply_add 清账（新任期新义务）。
            if g.witnessed_departs.contains(leaver) {
                return;
            }
            let blocked = g.presence_blocked(leaver);
            // 见证人旋转自己的行（换气只封存未来，A3）并重签整行
            let mut witness_row = g.my_row.clone();
            witness_row.rotate_and_resign(circle_id, &g.sig);
            let payload = rm_payload(circle_id, leaver.as_bytes(), &witness_row.to_bytes());
            let sig = g.sig.sign(&payload);
            let mut frame = payload;
            frame.extend_from_slice(&sig.to_bytes());
            (frame, witness_row, blocked)
        };
        // 在场辩护：还在说话即不可被判离开——扣下 + 响亮，交给人类
        if blocked {
            p.events
                .send(Event::CircleConflict {
                    circle_id,
                    detail: format!(
                        "[{leaver}] 的离席补话被扣下：她 30 秒内还在圈内说话。\
                         若确认她该离席，请重跑 /circle 点名重聚。"
                    ),
                })
                .ok();
            return;
        }
        // 去重自己的帧
        p.flood_seen
            .insert(flood_key(circle_id, &witness_row.r[..]));
        // 本地立即应用（见证人视角）+ 见证义务记账（此时才真正履行）
        let Some(mut g) = p.gathering.take() else {
            return;
        };
        g.witnessed_departs.insert(leaver.to_string());
        let (members, change) = g.apply_rm(leaver, &witness_row);
        p.gathering = Some(g);
        p.events
            .send(Event::CircleRekeyed { circle_id, members, change })
            .ok();
        frame
    };
    // 信念路由扇出（leaver 已从我的名单消失——断递，P-04）
    belief_flood(inner, None, frame_kind::TOPUP, &frame).await;
    // P16 三发之一（换气后）：RM 也是换气——锚线经得起行旋转
    //（sig_pk 不变），但补发幂等无成本，覆盖未锚定的窗口线
    send_line_proofs(inner).await;
}

/// 从缓冲区头部读出一行的线长（名长 u8 决定整行长度）；装不下 = None。
/// ADD/座次表帧里行是变长串联的——先量长短再下刀。
fn row_wire_len(b: &[u8]) -> Option<usize> {
    let name_len = *b.first()? as usize;
    let total = 1 + name_len + crate::circle::CONTRIB_LEN + 32 + 64;
    (b.len() >= total).then_some(total)
}

/// 0x0B ADD 的引荐收据载荷：圈号 ‖ 'A' ‖ 新人行（ accountability：
/// 出问题时『谁带来的人』有答案——A1 递话不代笔的收据面）。
fn add_sig_payload(circle_id: u64, newcomer_row: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + 1 + newcomer_row.len());
    v.extend_from_slice(&circle_id.to_be_bytes());
    v.push(b'A');
    v.extend_from_slice(newcomer_row);
    v
}

/// 座次表帧体（0x12）：圈号 ‖ 席位 ‖ 人数 ‖ 全部名单行（含 r——
/// 只递已成成员；ADD 应用后新人已是成员，这是她计算群钥的全部材料）。
fn roster_frame_of(g: &Gathering) -> Vec<u8> {
    let rows: Vec<&crate::circle::RosterRow> =
        g.members.values().chain(std::iter::once(&g.my_row)).collect();
    let mut v = Vec::with_capacity(10 + rows.len() * 130);
    v.extend_from_slice(&g.circle_id.to_be_bytes());
    v.push(g.seats);
    v.push(rows.len() as u8);
    for r in rows {
        v.extend_from_slice(&r.to_bytes());
    }
    v
}

/// 领人收尾（0x11 收端 = 引荐人）：行自署验证 → 期望门 → 名字/r 检查
/// → 旋转自己（P-13）→ 造 ADD → **发送原子地应用**（封桌也照常应用
/// ——fork 语义，settled 2026-10-16：签名即承诺、等批准=表决=同步机
/// 复活）→ 泛洪 ADD → 0x12 座次表递给新人。
async fn complete_bring(inner: &Arc<Mutex<Inner>>, candidate_line: u64, body: &[u8]) {
    // 圈号从回执行字节解（行首 8B；坏帧按 0 记——拒绝事件仍要响亮）
    let body_cid = u64::from_be_bytes(body[..body.len().min(8)].try_into().unwrap_or([0u8; 8]));
    let outcome = {
        let mut p = inner.lock().await;
        let Some(bring) = p.pending_brings.get(&candidate_line).cloned() else {
            p.events
                .send(Event::CircleRejected {
                    circle_id: body_cid,
                    detail: "收到一份没约过的落座回执——拒收。".into(),
                })
                .ok();
            return;
        };
        // 行自署验证（绑圈号——落座署名签的是这个圈的行）
        let Some(row) = crate::circle::RosterRow::parse_verified(body, bring.circle_id) else {
            p.events
                .send(Event::CircleRejected {
                    circle_id: bring.circle_id,
                    detail: "落座回执的自署验不过——伪造行进不了名单（P-04）。".into(),
                })
                .ok();
            return;
        };
        // 期望门：拟名不符 → 响亮冲突 + 回执丢弃 + 待座状态保留
        //（不缓存不悬挂——每次手势的协议效果完整；放行=重跑 /bring）
        if let Some(exp) = &bring.expected
            && *exp != row.name
        {
            p.events
                .send(Event::CircleConflict {
                    circle_id: bring.circle_id,
                    detail: format!(
                        "落座回执自称〔{}〕，与拟名〔{exp}〕不符——回执丢弃，\
                         待座保留；放行就重跑 /bring（改拟名或不带拟名）。",
                        row.name
                    ),
                })
                .ok();
            return;
        }
        let (add_frame, roster_frame, members, change, flood_k, alias_name) = {
            let Some(g) = p.gathering.as_mut() else {
                p.events
                    .send(Event::CircleRejected {
                        circle_id: bring.circle_id,
                        detail: "圈子已经散了——这次领人作废，重新 /bring。".into(),
                    })
                    .ok();
                return;
            };
            if g.circle_id != bring.circle_id || g.key.is_none() {
                p.events
                    .send(Event::CircleRejected {
                        circle_id: bring.circle_id,
                        detail: "圈子已经变了（点名过？）——重新看座。".into(),
                    })
                    .ok();
                return;
            }
            // 名字冲突：这名字已在桌上
            if row.name == g.my_row.name || g.members.contains_key(&row.name) {
                p.events
                    .send(Event::CircleRejected {
                        circle_id: bring.circle_id,
                        detail: format!("〔{}〕这名字已在桌上——入席拒收。", row.name),
                    })
                    .ok();
                return;
            }
            // r 唯一性（P-12）：碰撞若静默去重 = 无声驱逐
            if g.r_collides(&row.r[..]) {
                p.events
                    .send(Event::CircleRejected {
                        circle_id: bring.circle_id,
                        detail: format!(
                            "〔{}〕的 r 与现有成员碰撞——响亮拒绝，不静默去重（P-12）。",
                            row.name
                        ),
                    })
                    .ok();
                return;
            }
            // 引荐人旋转自己的行（P-13：不旋转则新人持全表现行 r、
            // 排除自己即算出 K_old，可解入座前历史）+ 重签整行
            let mut intro_row = g.my_row.clone();
            intro_row.rotate_and_resign(g.circle_id, &g.sig);
            let newcomer_bytes = row.to_bytes();
            let mut payload = Vec::with_capacity(9 + newcomer_bytes.len() + 130 + 64);
            payload.extend_from_slice(&g.circle_id.to_be_bytes());
            payload.push(b'A');
            payload.extend_from_slice(&newcomer_bytes);
            let intro_bytes = intro_row.to_bytes();
            payload.extend_from_slice(&intro_bytes);
            let sig = g.sig.sign(&add_sig_payload(g.circle_id, &newcomer_bytes));
            payload.extend_from_slice(&sig.to_bytes());
            // 应用（发送原子：封桌也照常——fork 由引荐人自己承担）
            let (members, change) = g.apply_add(&row, &intro_row);
            // 线绑定（P-15 + P16）：引荐人侧的候选人线锚定到 0x11 自署行
            // 的 pk——行自署在上面刚验过（帧内可验材料直接锚定，与
            // P-15「绑 0x11 行名（帧内可验）」同族升级：名字可验 ⇒ pk
            // 亦可验）。天生 pk 锚 = 严格档下无窗口竞态（成键后 speak
            // 不必等 0x13 往返）。
            g.line_bindings.insert(
                candidate_line,
                LineBinding::Name {
                    name: row.name.clone(),
                    proven_pk: Some(row.sig_pk),
                },
            );
            let roster_frame = roster_frame_of(g);
            (
                payload,
                roster_frame,
                members,
                change,
                flood_key(g.circle_id, &intro_row.r[..]),
                row.name,
            )
        };
        // 去重自己的 ADD（回声免疫）+ 手势完成 + 递 0x12 的线
        p.flood_seen.insert(flood_k);
        p.pending_brings.remove(&candidate_line);
        let send_target = p
            .convos
            .get(&candidate_line)
            .map(|c| (c.cipher.clone(), c.writer.clone()));
        p.events
            .send(Event::CircleRekeyed { circle_id: bring.circle_id, members, change: format!("{change}（{alias_name} 经我入席）") })
            .ok();
        (add_frame, roster_frame, send_target)
    };
    let (add_frame, roster_frame, send_target) = outcome;
    // 泛洪 ADD（信念路由——我的信念已含新人）
    belief_flood(inner, None, frame_kind::TOPUP, &add_frame).await;
    // 0x12 座次表走候选人那条 1:1 线（只递已成成员——她已在我名单上）
    if let Some((cipher, writer)) = send_target {
        let _ = send_typed(&cipher, &writer, frame_kind::SEAT_ROSTER, &roster_frame).await;
    }
    // P16 三发之一（成键后——引荐人本地应用路径）：换气即成键，向
    // 绑定线补发身体印（新人侧的出线锚定依赖引荐人的证明——严格档
    // 下漏发=新人的圈话无路可出）
    send_line_proofs(inner).await;
}

/// 座次表收端（0x12 = 候选人）：验每行自署 → 名字/r 唯一性 → 找到
/// 本人座位（按嗓音公钥——行是自签的，引荐人伪造不出我的 r）→ 铸钥
/// 入圈。旧桌（若有）就此放下（与 join_genesis 同构）。未 /sit 或
/// 没看过座却收到座次表：当没听见（跨版本容忍）。
async fn handle_seat_roster(inner: &Arc<Mutex<Inner>>, line: u64, body: &[u8]) {
    if body.len() < 10 {
        return; // 帧残缺：当没听见
    }
    let circle_id = u64::from_be_bytes(body[..8].try_into().expect("定长切8字节"));
    let seats = body[8];
    if seats > SEATS_OPEN {
        return; // 席位标志不合法：当没听见
    }
    let count = body[9] as usize;
    let mut pos = 10;
    let mut rows: Vec<crate::circle::RosterRow> = Vec::with_capacity(count);
    for _ in 0..count {
        let Some(len) = row_wire_len(&body[pos..]) else {
            break;
        };
        match crate::circle::RosterRow::parse_verified(&body[pos..pos + len], circle_id) {
            Some(row) => {
                rows.push(row);
                pos += len;
            }
            None => {
                let p = inner.lock().await;
                p.events
                    .send(Event::CircleRejected {
                        circle_id,
                        detail: "座次表里有验不过的名单行——拒收。".into(),
                    })
                    .ok();
                return;
            }
        }
    }
    if pos != body.len() || rows.len() != count {
        let p = inner.lock().await;
        p.events
            .send(Event::CircleRejected {
                circle_id,
                detail: "座次表残缺——拒收。".into(),
            })
            .ok();
        return;
    }
    // 名字唯一 + r 唯一（P-12 精神：碰撞不许静默）
    {
        let mut names = std::collections::HashSet::new();
        let mut rs = std::collections::HashSet::new();
        for row in &rows {
            if !names.insert(row.name.as_str()) || !rs.insert(row.r.to_vec()) {
                let p = inner.lock().await;
                p.events
                    .send(Event::CircleRejected {
                        circle_id,
                        detail: "座次表里有同名或同 r 的行——响亮拒绝（P-12）。".into(),
                    })
                    .ok();
                return;
            }
        }
    }
    let mut p = inner.lock().await;
    // 本人的落座材料（这条线上的待接邀约，且已 /sit）
    let my_seat = p.seat_offers.get(&line).and_then(|o| o.my_seat.clone());
    let Some((sig, my_sent_row)) = my_seat else {
        return; // 未落座/没邀约：当没听见
    };
    // 表里找到本人的座位（按嗓音公钥——我的行只能是我的钥签的）
    let Some(mine) = rows.iter().find(|r| r.sig_pk == sig.verifying_key()) else {
        p.events
            .send(Event::CircleRejected {
                circle_id,
                detail: "座次表里没有你的座位——拒收。".into(),
            })
            .ok();
        return;
    };
    if mine.name != my_sent_row.name || &mine.r[..] != &my_sent_row.r[..] {
        p.events
            .send(Event::CircleRejected {
                circle_id,
                detail: "座次表里你的座位与你递的落座不符——拒收。".into(),
            })
            .ok();
        return;
    }
    // 建圈（旧桌若有就此放下）：名单即信念，钥匙即桌——直接铸钥。
    // P-15 + P16 候选人侧绑定：送达 0x10/0x12 的线（引荐人的线）获得
    // 圈线资格——via 取 0x10 到达时的自报名 hint（0x12 无整帧签名，
    // 残余入档）；**pk 锚直接从 0x12 表内提取**（表行全部验过自署
    //——帧内可验材料，与 complete_bring 的 0x11 锚定同族）：天生
    // pk 锚 = 严格档下无窗口竞态。hint 名不在表内（引荐人覆盖落座名
    // 的罕见合成）→ 无锚，退窗口态（0x13 补发点覆盖，点名自愈）。
    let introducer_hint = p
        .seat_offers
        .get(&line)
        .map(|o| o.introducer.clone())
        .unwrap_or_default();
    let introducer_pk = rows
        .iter()
        .find(|r| r.name == introducer_hint)
        .map(|r| r.sig_pk);
    let members: HashMap<String, crate::circle::RosterRow> = rows
        .iter()
        .filter(|r| r.name != mine.name)
        .map(|r| (r.name.clone(), r.clone()))
        .collect();
    let mut all: Vec<&[u8]> = members.values().map(|m| &m.r[..]).collect();
    all.push(&mine.r[..]);
    let headcount = all.len();
    let key = Zeroizing::new(crate::circle::derive_group_key(&all));
    p.gathering = Some(Gathering {
        circle_id,
        seats,
        sig,
        my_row: mine.clone(),
        members,
        invited: None, // 信念从「名单」直接转为「成员」——我经座次表入座
        genesis_payload: None,
        genesis_verified: true,
        key: Some(key),
        counter: 0,
        presence: HashMap::new(),
        seen_bye: std::collections::HashSet::new(),
        left_voices: HashMap::new(),
        witnessed_departs: std::collections::HashSet::new(),
        line_bindings: HashMap::from([(
            line,
            LineBinding::Qualified {
                via: introducer_hint.clone(),
                peer: introducer_hint,
                proven_pk: introducer_pk,
            },
        )]),
    });
    p.seat_offers.remove(&line);
    p.events
        .send(Event::CircleFormed { circle_id, members: headcount })
        .ok();
    drop(p); // 先放锁——send_line_proofs 内部要再拿（不可重入）
    // P16 三发之一（成键后）：座次表入座即刻成键——向自己的绑定线
    //（引荐人线）补发线证明（幂等）
    send_line_proofs(inner).await;
}

/// 线证明收端（0x13·P16）：验签 = 按**当前信念名单**（members ∪
/// my_row）逐行试钥——签名归属即身份（不按名字查钥，their_name/
/// 载荷名皆不进判定）。恰一行通过 ⇒ 该线锚定到该行 pk：
/// - 已有绑定（Name/Qualified）→ proven_pk 升级（窗口态转 pk 锚，
///   名字保留为显示/溯源）；
/// - 未绑定线 → 直接授予 Proven{pk}（裁决点②：授予家族第三员，
///   relay_asymmetry 半开边消解点）。
/// 全败/无行/圈号不符/无 transcript = 当没听见（幂等：重放同证明
/// 无状态变化；同线旧圈证明圈号不符即弃）。**静默语义**：0x13 是
/// 无后果行为（不应用成员变更、不触发路由），响亮反而给探测者免费
/// oracle——与 bye-sig 去重的「垃圾进不了集合」同哲学（spec step_5）。
async fn handle_line_proof(inner: &Arc<Mutex<Inner>>, line: u64, body: &[u8]) {
    if body.len() < 8 + 64 {
        return; // 残缺：当没听见
    }
    let circle_id = u64::from_be_bytes(body[..8].try_into().expect("定长切8字节"));
    let sig = match ed25519_dalek::Signature::from_slice(&body[8..8 + 64]) {
        Ok(s) => s,
        Err(_) => return, // 签名都解不开：当没听见
    };
    let mut p = inner.lock().await;
    // 圈不在：暂存（乱序容忍——pending_contribs 同族；对端凑话后的
    // 0x13 可能抢在我 join 之前到达，join 后按圈号补验）
    if p.gathering.is_none() {
        if p.pending_proofs.len() >= PENDING_PROOF_CAP {
            p.pending_proofs.pop_front();
        }
        p.pending_proofs.push_back((circle_id, sig.to_bytes(), line));
        return;
    }
    // 圈在但圈号不符：旧圈证明对当前圈无意义，丢弃（不暂存——暂存
    // 条目按圈号补验，旧圈号在 join 后仍不符，存了也是死信）
    if !p
        .gathering
        .as_ref()
        .is_some_and(|g| g.circle_id == circle_id)
    {
        return;
    }
    anchor_line_proof(&mut p, line, circle_id, &sig);
}

/// 线证明的验签与锚定核心（直验与暂存补验共用）：本线 transcript →
/// 载荷 → 按名单（members ∪ my_row）逐行试钥 → 恰一行通过 ⇒ 锚定
///（升级 proven_pk / 授予 Proven）。任何失败 = 安静返回（无后果行为）。
fn anchor_line_proof(
    p: &mut Inner,
    line: u64,
    circle_id: u64,
    sig: &ed25519_dalek::Signature,
) {
    // 本线 transcript（证明的绑定面——per-line，不可跨线重放）——
    // 先于可变借用取走（gathering 的独占借用与 convos 读取互斥）
    let transcript = p.convos.get(&line).map(|c| Zeroizing::new(*c.transcript));
    let Some(g) = p.gathering.as_mut() else {
        return;
    };
    if g.circle_id != circle_id {
        return;
    }
    let Some(transcript) = transcript else {
        return;
    };
    let payload = crate::circle::proof_payload(circle_id, &transcript);
    // 逐行试钥（members ∪ my_row）：签名归属即身份
    let proven = if g.my_row.sig_pk.verify_strict(&payload, sig).is_ok() {
        Some(g.my_row.sig_pk)
    } else {
        g.members
            .values()
            .find(|row| row.sig_pk.verify_strict(&payload, sig).is_ok())
            .map(|row| row.sig_pk)
    };
    let Some(pk) = proven else {
        return; // 名单外之手（或伪造/重放）：当没听见
    };
    match g.line_bindings.get_mut(&line) {
        Some(LineBinding::Name { proven_pk, .. })
        | Some(LineBinding::Qualified { proven_pk, .. }) => {
            *proven_pk = Some(pk); // 窗口态 → pk 锚（升级；重放幂等）
        }
        Some(LineBinding::Proven { .. }) => {} // 已锚定：幂等空转
        None => {
            // 裁决点②：未绑定线的授予（第三授予行为——快照/送达/证明）
            g.line_bindings.insert(line, LineBinding::Proven { pk });
        }
    }
}

/// 落座新桌（0x0F 收端）：生成嗓音钥与自署行、记下邀请名单与待验的
/// 开台公告，把自己的行泛洪出去（自传染），补放早到的凑话。
/// 同圈重放由调用方挡（此处只管落座）；旧桌（若有）就此放下——
/// 点名 = 新圈号重新聚首，社会连续、密码学新鲜（A4）。
/// src_line = 送达线（P-15 spec a）：落座时这条线获得圈线资格
///（Qualified{via:creator}——creator 名来自帧内，非线的对端自称）。
async fn join_genesis(
    inner: &Arc<Mutex<Inner>>,
    circle_id: u64,
    seats: u8,
    creator: String,
    invited: std::collections::HashSet<String>,
    full_body: &[u8],
    src_line: u64,
) {
    let (my_frame, drained, drained_proofs) = {
        let mut p = inner.lock().await;
        let my_name = p.my_name.clone();
        let mut seed = Zeroizing::new([0u8; 32]);
        if getrandom::fill(&mut seed[..]).is_err() {
            return; // 造不出嗓音钥：这次没落成座，弃权（诚实：不掩错）
        }
        let sig = ed25519_dalek::SigningKey::from_bytes(&seed);
        let my_row = crate::circle::RosterRow::self_seat(circle_id, &my_name, &sig);
        // 回声免疫：自己的贡献键先记下
        p.flood_seen.insert(flood_key(circle_id, &my_row.r[..]));
        p.gathering = Some(Gathering {
            circle_id,
            seats,
            sig,
            my_row,
            members: HashMap::new(),
            invited: Some(invited),
            genesis_payload: Some((creator.clone(), full_body.to_vec())),
            genesis_verified: false,
            key: None,
            counter: 0,
            presence: HashMap::new(),
            seen_bye: std::collections::HashSet::new(),
            left_voices: HashMap::new(),
            witnessed_departs: std::collections::HashSet::new(),
            // P-15 spec a：送达线获得圈线资格。双键：via=帧内 creator
            //（签名补验走 genesis_payload；这条线**不是**「creator 的
            // 线」，转发送达时对端是中继者，资格语义不误绑）；peer=
            // 送达线对端的 0x05 自报名捕获快照（A2 认可的唯一线对端
            // 感知源）——撤销/路由/见证三判定都读它（阻断项 B 修复）
            line_bindings: {
                let peer = p
                    .convos
                    .get(&src_line)
                    .map(|c| c.their_name.clone())
                    .unwrap_or_default();
                HashMap::from([(
                    src_line,
                    LineBinding::Qualified {
                        via: creator.clone(),
                        peer,
                        proven_pk: None,
                    },
                )])
            },
        });
        let mut frame = Vec::with_capacity(8 + 130 + my_name.len());
        frame.extend_from_slice(&circle_id.to_be_bytes());
        frame.extend_from_slice(&p.gathering.as_ref().unwrap().my_row.to_bytes());
        // 补放早到的凑话（乱序容忍）——带原始到达线号（P-15：直连
        // 声明判定读线绑定，暂存必须保线号）
        let drained: Vec<(Vec<u8>, u64)> = p
            .pending_contribs
            .iter()
            .filter(|(cid, _, _)| *cid == circle_id)
            .map(|(_, body, line)| (body.clone(), *line))
            .collect();
        p.pending_contribs.retain(|(cid, _, _)| *cid != circle_id);
        // 补验早到的线证明（P16 乱序容忍——对端凑话后的 0x13 抢在
        // 我 join 之前到达的暂存）：此刻我的名单可能还没有对端的行
        //（她的 0x09 也在暂存队列里，drained 补放后才齐）——补验放在
        // 凑话补放之后（members 就绪）再跑
        let drained_proofs: Vec<([u8; 64], u64)> = p
            .pending_proofs
            .iter()
            .filter(|(cid, _, _)| *cid == circle_id)
            .map(|(_, sig, line)| (*sig, *line))
            .collect();
        p.pending_proofs.retain(|(cid, _, _)| *cid != circle_id);
        (frame, drained, drained_proofs)
    };
    // 我的自署行泛洪（信念 = 邀请名单 ∪ 我；路由走刚授予的送达线资格）
    belief_flood(inner, None, frame_kind::CONTRIB_V2, &my_frame).await;
    for (body, line) in drained {
        handle_contribution(inner, Some(line), &body).await;
    }
    // P16 补验暂存的线证明（凑话补放之后——试钥的名单此刻就绪）
    for (sig_bytes, line) in drained_proofs {
        if let Ok(sig) = ed25519_dalek::Signature::from_slice(&sig_bytes) {
            let mut p = inner.lock().await;
            anchor_line_proof(&mut p, line, circle_id, &sig);
        }
    }
    // P16 三发之一（凑话后）：向自己的每条绑定线递身体印（幂等）
    send_line_proofs(inner).await;
}

/// 凑话（0x09·自署行）收端处理：验自署 → 去重 → 信念检查 → 入册/
/// 裁决 → 创世补验 → 铸钥 → 信念路由转发。一条路径管四种响亮：
/// r 碰撞（P-12）、名单外（P-03）、同名对峙（P-05）、创世验签。
/// src_line = 到达线（转发排除 + P-15 直连声明判定：线的 Name 绑定
/// == 行名 ⟺ 这条线对面就是她本人——their_name 判定已退役）。
async fn handle_contribution(
    inner: &Arc<Mutex<Inner>>,
    src_line: Option<u64>,
    body: &[u8], // 圈号 8 ‖ 行（原样转发）
) {
    if body.len() < 8 + 1 + crate::circle::CONTRIB_LEN + 32 + 64 {
        return; // 行都装不下：当没听见
    }
    let circle_id = u64::from_be_bytes(body[..8].try_into().expect("定长切8字节"));
    let Some(row) = crate::circle::RosterRow::parse_verified(&body[8..], circle_id) else {
        return; // 行自署验不过（P-04/P-05）：伪造行进不了名单，当没听见
    };
    // 去重：见过的贡献不再处理/转发
    let fk = flood_key(circle_id, &row.r[..]);
    let mut p = inner.lock().await;
    if p.flood_seen.contains(&fk) {
        return;
    }
    // 圈不在/圈号对不上：暂存（乱序容忍——0x09 可能先于 0x0F 到达）。
    // 去重键此刻**不记**：早到暂存的行还没被真正处理过，若先记键，
    // 开台补放会撞上去重被静默丢弃——暂存机制名存实亡（slice-6 发现：
    // 0x09 在多线并发下可稳定先于 0x0F 到达，暴露此缺陷；单运行时
    // 对局里 genesis 几乎总先到，时运掩盖了它）。补放走完整处理
    //（含去重+转发）；重复暂存无害——补放时第二份撞去重。
    let circle_here = p
        .gathering
        .as_ref()
        .is_some_and(|g| g.circle_id == circle_id);
    if !circle_here {
        if p.pending_contribs.len() >= PENDING_CONTRIB_CAP {
            p.pending_contribs.pop_front();
        }
        // 暂存带原始到达线号（P-15：补放时的直连声明判定与转发排除
        // 都要对原线生效；无线的凑话（理论不可达）按无源处理）
        let line = src_line.unwrap_or(0);
        p.pending_contribs
            .push_back((circle_id, body.to_vec(), line));
        return;
    }
    if p.flood_seen.len() >= FLOOD_SEEN_CAP {
        p.flood_seen.clear();
    }
    p.flood_seen.insert(fk);
    // 自己的回声：按名单行名判（/sit 覆盖落座名后，p.my_name 只是
    // 1:1 自报名——回声对的是名单行名）
    if p
        .gathering
        .as_ref()
        .is_some_and(|g| row.name == g.my_row.name)
    {
        return;
    }
    // (锁内) 先决后放：收集事件与转发决定，借用结束后统一上屏
    let mut ev: Option<Event> = None;
    let mut forward = true;
    let mut formed: Option<usize> = None;
    {
        let Some(g) = p.gathering.as_mut() else {
            return;
        };
        // P-15 + P16：直连声明判定读线绑定——pk 锚优先（锚 pk == 行
        // sig_pk ⟺ 对端在这条线上亲手证明过自己，A1 的完成态）；锚
        // 缺席的窗口按 Name 绑定 == 行名（快照/bring 的信任映射）。
        // Qualified 资格线（0x0F/0x12 送达）不是身份断言：不构成直连
        // 声明——转述到达，等她的锚定/名字绑定线裁决（未绑线 = 非直连，
        // 顶替退为响亮冲突 + 点名，spec ④）
        let direct_claim = src_line.is_some_and(|l| {
            g.line_bindings.get(&l).is_some_and(|b| {
                b.anchor_pk() == Some(&row.sig_pk)
                    || (b.anchor_pk().is_none()
                        && matches!(b, LineBinding::Name { name, .. } if *name == row.name))
            })
        });
        // P-12：r 碰撞响亮拒绝（无声去重 = 无声驱逐）
        if g.r_collides(&row.r[..]) {
            ev = Some(Event::CircleRejected {
                circle_id,
                detail: format!(
                    "〔{}〕的凑话 r 与现有成员碰撞——响亮拒绝，不静默去重（P-12）。",
                    row.name
                ),
            });
            forward = false;
        // P-03：名单外的名字拒收（响亮）——凑话完成由名单说了算
        } else if !g.on_roster(&row.name) {
            ev = Some(Event::CircleRejected {
                circle_id,
                detail: format!(
                    "〔{}〕不在这张桌上——名单外凑话拒收（P-03）。",
                    row.name
                ),
            });
            forward = false;
        } else if let Some(taken) = g.members.get(&row.name) {
            // P-05：同名对峙——先到暂定；名字绑定线上的声明 = 见证裁决
            if taken.sig_pk == row.sig_pk {
                // 同嗓音：直连到达则更新（本人的最新印），转述则保留原行
                if direct_claim {
                    g.members.insert(row.name.clone(), row.clone());
                } else {
                    forward = false; // 转述的同钥重复：当没听见
                }
            } else if direct_claim {
                // 见证裁决：直连声明取代暂定（这条线对面就是她本人）
                ev = Some(Event::CircleConflict {
                    circle_id,
                    detail: format!(
                        "名字〔{}〕有两份名单行：直连声明取代先到的暂定行（P-05 裁决）。",
                        row.name
                    ),
                });
                g.members.insert(row.name.clone(), row.clone());
            } else {
                // 转述到达的对峙：保留暂定、响亮冲突——转发出去，
                // 让她的名字绑定线裁决
                ev = Some(Event::CircleConflict {
                    circle_id,
                    detail: format!(
                        "名字〔{}〕有两份名单行对峙：暂保留先到的，等直连声明裁决（P-05）。",
                        row.name
                    ),
                });
            }
        } else {
            g.members.insert(row.name.clone(), row.clone());
            // 创世补验：发起人行到达 → 用她的嗓音公钥验 0x0F 公告
            if !g.genesis_verified
                && let Some((creator, full)) = g.genesis_payload.clone()
                && creator == row.name
                && full.len() >= 64
                && let Ok(sig) = ed25519_dalek::Signature::from_slice(&full[full.len() - 64..])
                && row
                .sig_pk
                .verify_strict(&full[..full.len() - 64], &sig)
                .is_ok()
            {
                g.genesis_verified = true;
            }
            // 名单到齐即铸钥（计数死，P-03 fix）
            formed = g.try_complete();
        }
    }
    if let Some(e) = ev {
        p.events.send(e).ok();
    }
    if let Some(members) = formed {
        p.events.send(Event::CircleFormed { circle_id, members }).ok();
    }
    drop(p);
    // 信念路由转发（除来源线）
    if forward {
        belief_flood(inner, src_line, frame_kind::CONTRIB_V2, body).await;
    }
    // P16 三发之一（成键后）：名单到齐铸钥即向绑定线补发身体印
    //（幂等；UDP 乱序丢的证明由此就地恢复，无需等点名）
    if formed.is_some() {
        send_line_proofs(inner).await;
    }
}

/// 补话行的 r 碰撞检查（P-12）：见证人新 r 与名单上**其他**成员碰撞
/// → 拒绝；与她自己的旧行相同不算（旋转自洽）。
fn g_check_r_collision(p: &Inner, witness_row: &crate::circle::RosterRow) -> bool {
    let Some(g) = p.gathering.as_ref() else {
        return true;
    };
    let witness = &witness_row.name;
    g.members
        .values()
        .filter(|m| m.name != *witness)
        .any(|m| &m.r[..] == &witness_row.r[..])
        || (*witness != g.my_row.name && &g.my_row.r[..] == &witness_row.r[..])
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
    let (transport, transcript) = handshake(&mut stream, as_initiator, secret)
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
        transcript,
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
    transcript: Zeroizing<[u8; 32]>,
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
        let mut plain = Zeroizing::new(vec![0u8; ciphertext.len()]);
        let n = cipher
            .lock()
            .await
            .read_message(&ciphertext, &mut plain[..])
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
                transcript,
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
                // 明文缓冲用后即焚：每一帧的解密产物在帧处理完就清零
                let mut plain = Zeroizing::new(vec![0u8; ciphertext.len()]);
                // 先把读结果取出：锁守卫若留在 match 里，
                // 下面回 ACK 时再锁同一把 cipher 就是自锁
                let read = cipher.lock().await.read_message(&ciphertext, &mut plain[..]);
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
                                    p.expecting_secret = Some(Zeroizing::new(plain[1..n].to_vec()));
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
                        frame_kind::GENESIS => {
                            // 开台公告（0x0F）：署名邀请名单 = 发起者感知
                            // 快照（事实陈述，非权力凭证）。布局：圈号 8
                            // ‖ 席位 1 ‖ 发起人(名长1‖名字) ‖ 人数 1
                            // ‖ 名单（名长1‖名字 ×n）‖ 署名 64。
                            // P-15（2026-10-17）：一跳 direct-lock
                            //（creator==their_name）退役——它本身就是一处
                            // their_name 检查（P-15 攻击面在创世处的实例）。
                            // 取而代之：送达绑定（spec a）——送达使这条线
                            // 获得此圈的**路由资格**（Qualified{via:creator}），
                            // creator 身份来自帧内签名（genesis_payload 补验
                            // 路径），不来自线。转发送达天然按资格语义处理
                            //（线的对端不是 creator 也绝不误绑）。
                            if n >= 1 + 8 + 1 + 1 + 1 + 1 + 64 {
                                let body = &plain[1..n];
                                let circle_id = u64::from_be_bytes(
                                    body[..8].try_into().expect("定长切8字节"),
                                );
                                let seats = body[8];
                                if seats > SEATS_OPEN {
                                    continue; // 席位标志不合法：当没听见
                                }
                                let creator_len = body[9] as usize;
                                if body.len() < 10 + creator_len + 1 + 64 {
                                    continue; // 帧残缺：当没听见
                                }
                                let Ok(creator) =
                                    std::str::from_utf8(&body[10..10 + creator_len])
                                else {
                                    continue;
                                };
                                let count = body[10 + creator_len] as usize;
                                let mut pos = 11 + creator_len;
                                let mut invited = std::collections::HashSet::new();
                                let mut ok = true;
                                for _ in 0..count {
                                    if body.len() < pos + 1 + 64 {
                                        ok = false;
                                        break;
                                    }
                                    let l = body[pos] as usize;
                                    if body.len() < pos + 1 + l + 64 {
                                        ok = false;
                                        break;
                                    }
                                    match std::str::from_utf8(&body[pos + 1..pos + 1 + l]) {
                                        Ok(name) => {
                                            invited.insert(name.to_string());
                                        }
                                        Err(_) => {
                                            ok = false;
                                            break;
                                        }
                                    }
                                    pos += 1 + l;
                                }
                                if !ok || body.len() != pos + 64 {
                                    continue; // 名单残缺：当没听见
                                }
                                // 同圈重放：不重 join（P-15：join_genesis 会
                                // 替换旧桌，放行重放即重置凑话进度并二次
                                // 泛洪新行）——**但新线到达的副本补绑**
                                //（P16 修复：genesis 多径竞速会把 join 钉死
                                // 在首达路径（快线），直连线（慢线副本先到
                                // 慢）被冷落 → 无绑定 → 严格档下熄灭 →
                                // 直连拓扑退化为中继。多径送达 = 多条授予
                                //（A2：每条送达线独立获得资格）；已绑定的
                                // 线不重绑（幂等）。静默授予，无事件。
                                {
                                    let mut p = inner.lock().await;
                                    if p
                                        .gathering
                                        .as_ref()
                                        .is_some_and(|g| g.circle_id == circle_id)
                                    {
                                        let peer_name = p.convos.get(&id).map(|c| c.their_name.clone());
                                        if let (Some(g), Some(peer)) = (p.gathering.as_mut(), peer_name) {
                                            g.line_bindings
                                                .entry(id)
                                                .or_insert_with(|| LineBinding::Qualified {
                                                    via: creator.to_string(),
                                                    peer,
                                                    proven_pk: None,
                                                });
                                        }
                                        continue;
                                    }
                                }
                                let my_name = {
                                    let p = inner.lock().await;
                                    p.my_name.clone()
                                };
                                if !invited.contains(&my_name) {
                                    // 名单=发起者感知快照，不在名单上的人
                                    // 不落座（P-03）——也不授绑（P-09 拒入者
                                    // 连圈线资格都不该拿到）
                                    continue;
                                }
                                // 不同圈（点名/新桌）：直接落座新桌——社会
                                // 连续、密码学新鲜（A4）。送达线在落座时获得
                                // 圈线资格（spec a）
                                join_genesis(
                                    &inner,
                                    circle_id,
                                    seats,
                                    creator.to_string(),
                                    invited,
                                    body,
                                    id,
                                )
                                    .await;
                            }
                        }
                        frame_kind::CONTRIB | frame_kind::CONTRIB_V2 => {
                            // 凑话。v1（0x03，裸 32B）与 v2 布局（带规模字段）
                            // 已随 3.0.0 断代（wire-breaking）：不再接待，
                            // 沉默跳过——v3 只认自署行（0x09 新布局）。
                            if plain[0] == frame_kind::CONTRIB_V2 && n >= 1 + 8 + 1 + 32 + 32 + 64
                            {
                                let body = &plain[1..n];
                                handle_contribution(&inner, Some(id), body).await;
                            }
                        }
                        frame_kind::CIRCLE_SIGNED => {
                            // 带署名的圈子发言：子钥解密 + 公钥验签双闸。
                            // 布局：圈号 8 ‖ 标签 4 ‖ 计数 8 ‖ 群密文 ‖ 署名 64
                            if n >= 1 + 8 + 4 + 8 + 12 + 64 {
                                let body = &plain[1..n];
                                let circle_id = u64::from_be_bytes(
                                    body[..8].try_into().expect("定长切8字节"),
                                );
                                let tag: [u8; 4] =
                                    body[8..12].try_into().expect("定长切4字节");
                                // 去重键取（标签‖计数）：同一句话只过一次闸
                                let fk = flood_key(circle_id, &body[8..20]);
                                let mut p = inner.lock().await;
                                if p.flood_seen.contains(&fk) {
                                    continue;
                                }
                                if p.flood_seen.len() >= FLOOD_SEEN_CAP {
                                    p.flood_seen.clear();
                                }
                                p.flood_seen.insert(fk);
                                drop(p);

                                // 双闸验证：第一闸（子钥解密）锁内找说话人
                                let mut spoken: Option<(String, Zeroizing<Vec<u8>>)> = None;
                                {
                                    let p = inner.lock().await;
                                    if let Some(g) = p.gathering.as_ref() {
                                        if g.circle_id == circle_id {
                                            if let Some(k) = g.key.as_ref() {
                                                // 按署名标签找说话的人（子钥即署名——
                                                // 只有贡献对的人的子钥能开这把锁）
                                                for m in g.members.values() {
                                                    if crate::circle::sender_tag_of(&m.r[..]) == tag {
                                                        let sub = crate::circle::derive_sender_subkey(k, &m.r[..]);
                                                        if let Some(text) =
                                                            crate::circle::open(&sub, &body[20..body.len() - 64])
                                                        {
                                                            spoken = Some((
                                                                m.name.clone(),
                                                                Zeroizing::new(text),
                                                            ));
                                                            break;
                                                        }
                                                    }
                                                }
                                                // 我自己的话泛洪回来：自己子钥验一次
                                                if spoken.is_none()
                                                    && crate::circle::sender_tag_of(&g.my_row.r[..]) == tag
                                                {
                                                    let sub =
                                                        crate::circle::derive_sender_subkey(k, &g.my_row.r[..]);
                                                    if let Some(text) =
                                                        crate::circle::open(&sub, &body[20..body.len() - 64])
                                                    {
                                                        spoken = Some((
                                                            g.my_row.name.clone(),
                                                            Zeroizing::new(text),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if let Some((speaker, text)) = spoken {
                                    // 说话者的公钥验签（第二闸，防标签碰撞冒充）
                                    // 验签通过 = 在场辩护记账（本地挂钟，不落盘）
                                    let mut sig_ok = false;
                                    {
                                        let mut p = inner.lock().await;
                                        if let Some(g) = p.gathering.as_mut() {
                                            if g.circle_id == circle_id
                                                && let Ok(sig) = ed25519_dalek::Signature::from_slice(
                                                &body[body.len() - 64..],
                                            )
                                            {
                                                let vk = if speaker == g.my_row.name {
                                                    Some(g.my_row.sig_pk)
                                                } else {
                                                    g.members.get(&speaker).map(|m| m.sig_pk)
                                                };
                                                if let Some(vk) = vk {
                                                    sig_ok = vk
                                                        .verify_strict(&body[..body.len() - 64], &sig)
                                                        .is_ok();
                                                }
                                            }
                                            if sig_ok {
                                                // 在场证据：她在说话——RM 不许说她在 30 秒前就走了
                                                g.presence.insert(speaker.clone(), std::time::Instant::now());
                                            }
                                        }
                                    }
                                    if sig_ok {
                                        let text =
                                            String::from_utf8_lossy(&text).into_owned();
                                        let p = inner.lock().await;
                                        p.events
                                            .send(Event::Heard {
                                                id,
                                                name: speaker,
                                                text,
                                            })
                                            .ok();
                                    } else {
                                        let p = inner.lock().await;
                                        p.events
                                            .send(Event::CircleMumble {
                                                circle_id,
                                                from: their_name.clone(),
                                            })
                                            .ok();
                                    }
                                } else {
                                    // 子钥解不开：密钥不合或密文被动过
                                    // （T1 响亮分歧——信念分岔的感知）
                                    let p = inner.lock().await;
                                    p.events
                                        .send(Event::CircleMumble {
                                            circle_id,
                                            from: their_name.clone(),
                                        })
                                        .ok();
                                }
                                // 验证通过与否都转发（泛洪的义务）——
                                // 但只转给还没见过的（去重表已保证）；
                                // 信念路由：只走名单上的线
                                belief_flood(&inner, Some(id), plain[0], &plain[1..n]).await;
                            }
                        }
                        frame_kind::TOPUP => {
                            // 补话（v3·RM/ADD 见证制）：圈号 8 ‖ 变更字节
                            //（'R'=RM 离席 / 'A'=ADD 入席）‖ 变更载荷。
                            // RM：离场者(名长1‖名字) ‖ 见证人新行（自署整行，
                            // r 已旋转）‖ 见证人署名 64（签除署名外全帧）。
                            // 无纪元、无选举、无级联——幂等应用：条件删除 +
                            // 无条件行替换（多见证人收敛，T2 交换代数）。
                            if n >= 1 + 8 + 1 + 1 + 1 + 32 + 32 + 64 + 64 {
                                let body = &plain[1..n];
                                let circle_id = u64::from_be_bytes(
                                    body[..8].try_into().expect("定长切8字节"),
                                );
                                if body[8] == b'A' {
                                    // ── ADD（入席流第三步）：'A' ‖ 新人行
                                    //（自签）‖ 引荐人新行（自签，含旋转后
                                    // r——P-13）‖ 引荐收据 64（签
                                    // circle_id‖'A'‖新人行）。布局与 RM 镜像
                                    // 对称，行结构三载体同构。──
                                    let rest = &body[9..];
                                    let Some(new_len) = row_wire_len(rest) else {
                                        continue; // 帧残缺：当没听见
                                    };
                                    let newcomer_row =
                                        match crate::circle::RosterRow::parse_verified(
                                            &rest[..new_len],
                                            circle_id,
                                        ) {
                                            Some(r) => r,
                                            None => {
                                                // P-04：引荐人代笔的伪造行进不了名单
                                                let p = inner.lock().await;
                                                p.events
                                                    .send(Event::CircleRejected {
                                                        circle_id,
                                                        detail: "一份 ADD 的新人行自署验不过——\
                                                                 伪造行进不了名单（P-04）。"
                                                            .into(),
                                                    })
                                                    .ok();
                                                continue;
                                            }
                                        };
                                    let rest2 = &rest[new_len..];
                                    let Some(intro_len) = row_wire_len(rest2) else {
                                        continue; // 帧残缺：当没听见
                                    };
                                    if rest2.len() != intro_len + 64 {
                                        continue; // 帧残缺：当没听见
                                    }
                                    let introducer_row = match crate::circle::RosterRow::parse_verified(
                                        &rest2[..intro_len],
                                        circle_id,
                                    ) {
                                        Some(r) => r,
                                        None => {
                                            let p = inner.lock().await;
                                            p.events
                                                .send(Event::CircleRejected {
                                                    circle_id,
                                                    detail: "一份 ADD 的引荐人行自署验不过——拒收。"
                                                        .into(),
                                                })
                                                .ok();
                                            continue;
                                        }
                                    };
                                    let receipt_sig = &rest2[intro_len..];
                                    // 去重（按引荐人新 r——帧签名者的旋转，
                                    // 与 RM 的见证人新 r 镜像对称）
                                    let fk = flood_key(circle_id, &introducer_row.r[..]);
                                    let mut p = inner.lock().await;
                                    if p.flood_seen.contains(&fk) {
                                        continue;
                                    }
                                    if p.flood_seen.len() >= FLOOD_SEEN_CAP {
                                        p.flood_seen.clear();
                                    }
                                    p.flood_seen.insert(fk);
                                    // 圈对不上：不是我信念里的圈
                                    if p
                                        .gathering
                                        .as_ref()
                                        .is_none_or(|g| g.circle_id != circle_id)
                                    {
                                        continue;
                                    }
                                    // 引荐人须在名单，且行内嗓音公钥与名单一致
                                    //（A1：行名可伪、嗓音不可顶替——堵『名字行
                                    // 换钥』冒名洞；我自己的回声同理须钥一致）
                                    let is_echo = p.gathering.as_ref().is_some_and(|g| {
                                        introducer_row.name == g.my_row.name
                                    });
                                    let intro_known = p.gathering.as_ref().is_some_and(|g| {
                                        if introducer_row.name == g.my_row.name {
                                            introducer_row.sig_pk == g.my_row.sig_pk
                                        } else {
                                            g.members
                                                .get(&introducer_row.name)
                                                .is_some_and(|m| m.sig_pk == introducer_row.sig_pk)
                                        }
                                    });
                                    if !intro_known {
                                        p.events
                                            .send(Event::CircleRejected {
                                                circle_id,
                                                detail: format!(
                                                    "〔{}〕不在名单上或嗓音对不上，却署了一份 ADD——\
                                                     外人冒引荐人，拒收。",
                                                    introducer_row.name
                                                ),
                                            })
                                            .ok();
                                        continue;
                                    }
                                    // 引荐收据验证（问责线：谁带来的人）
                                    let mut receipt_ok = false;
                                    if let Ok(sig) =
                                        ed25519_dalek::Signature::from_slice(receipt_sig)
                                    {
                                        receipt_ok = introducer_row
                                            .sig_pk
                                            .verify_strict(
                                                &add_sig_payload(circle_id, &rest[..new_len]),
                                                &sig,
                                            )
                                            .is_ok();
                                    }
                                    if !receipt_ok {
                                        p.events
                                            .send(Event::CircleRejected {
                                                circle_id,
                                                detail: format!(
                                                    "署名〔{}〕的引荐收据验不过——伪造 ADD 拒收。",
                                                    introducer_row.name
                                                ),
                                            })
                                            .ok();
                                        continue;
                                    }
                                    // 旋转检查（P-13，非回声路径）：引荐人必须
                                    // 换气——不旋转则新人拿全表现行 r 即可解
                                    // 入座前历史
                                    if !is_echo {
                                        let rotated = p.gathering.as_ref().is_some_and(|g| {
                                            g.members
                                                .get(&introducer_row.name)
                                                .is_some_and(|m| &m.r[..] != &introducer_row.r[..])
                                        });
                                        if !rotated {
                                            p.events
                                                .send(Event::CircleRejected {
                                                    circle_id,
                                                    detail: format!(
                                                        "〔{}〕引荐入席没换气（P-13：必须旋转 r）——拒收。",
                                                        introducer_row.name
                                                    ),
                                                })
                                                .ok();
                                            continue;
                                        }
                                    }
                                    // 名字冲突 + r 碰撞（P-12）+ 席位契约
                                    let rejected = {
                                        let Some(g) = p.gathering.as_ref() else {
                                            continue;
                                        };
                                        if newcomer_row.name == g.my_row.name
                                            || g.members.contains_key(&newcomer_row.name)
                                        {
                                            Some(format!(
                                                "〔{}〕这名字已在桌上——入席拒收。",
                                                newcomer_row.name
                                            ))
                                        } else if g.r_collides(&newcomer_row.r[..]) {
                                            Some(format!(
                                                "新人的 r 与现有成员碰撞——响亮拒绝（P-12）。"
                                            ))
                                        } else if g
                                            .members
                                            .values()
                                            .filter(|m| m.name != introducer_row.name)
                                            .any(|m| &m.r[..] == &introducer_row.r[..])
                                            || (introducer_row.name != g.my_row.name
                                                && &g.my_row.r[..] == &introducer_row.r[..])
                                        {
                                            Some("引荐人的新 r 与现有成员碰撞——响亮拒绝（P-12）。".into())
                                        } else if !is_echo && g.seats == SEATS_CLOSED {
                                            // 封桌：席位契约，本地响亮拒收（不应用）
                                            // ——fork 由引荐人自己承担（settled
                                            // 2026-10-16：封桌是契约不是法律，协议
                                            // 让违约响亮，不使违约不可能）
                                            Some(format!(
                                                "封桌：〔{}〕引荐〔{}〕的入席被拒（席位契约）——\
                                                 换血=点名。",
                                                introducer_row.name, newcomer_row.name
                                            ))
                                        } else {
                                            None
                                        }
                                    };
                                    if let Some(detail) = rejected {
                                        p.events
                                            .send(Event::CircleRejected { circle_id, detail })
                                            .ok();
                                        drop(p);
                                        // P-15：契约类拒收（名字冲突/r 碰撞/
                                        // 封桌——签名已验过的合法帧）**照转**
                                        // ——星型路由下静默丢弃会让下游成员
                                        // 看不见这次尝试，违反『让违约响亮』
                                        //（泛洪去重防环）。伪造类拒收（行自署/
                                        // 收据/名单不符/P-13）仍到此为止
                                        //（P-01 家族：伪造帧不给扩散）。
                                        belief_flood(&inner, Some(id), plain[0], &plain[1..n]).await;
                                        continue;
                                    }
                                    // 应用（开台：引荐即背书）
                                    let (members, change) = {
                                        let Some(g) = p.gathering.as_mut() else {
                                            continue;
                                        };
                                        g.apply_add(&newcomer_row, &introducer_row)
                                    };
                                    p.events
                                        .send(Event::CircleRekeyed { circle_id, members, change })
                                        .ok();
                                    drop(p);
                                    // 信念路由转发
                                    belief_flood(&inner, Some(id), plain[0], &plain[1..n]).await;
                                    // P16 三发之一（成键后——ADD 路径）：换气即
                                    // 成键，向绑定线补发身体印（引荐人收端同
                                    // complete_bring——新人侧依赖引荐人的证明
                                    // 才能锚定她的出线，漏发=严格档下新人的
                                    // 圈话无路可出）
                                    send_line_proofs(&inner).await;
                                    continue;
                                }
                                if body[8] != b'R' {
                                    continue; // 不认识的变更：当没听见
                                }
                                let leaver_len = body[9] as usize;
                                if body.len() < 10 + leaver_len + 32 + 32 + 64 + 64 {
                                    continue; // 帧残缺：当没听见
                                }
                                let Ok(leaver) =
                                    std::str::from_utf8(&body[10..10 + leaver_len])
                                else {
                                    continue;
                                };
                                let row_bytes = &body[10 + leaver_len..body.len() - 64];
                                let sig_bytes = &body[body.len() - 64..];
                                // 行自署验证（P-04/P-05：伪造行进不了名单）
                                let Some(witness_row) =
                                    crate::circle::RosterRow::parse_verified(row_bytes, circle_id)
                                else {
                                    let p = inner.lock().await;
                                    p.events
                                        .send(Event::CircleRejected {
                                            circle_id,
                                            detail: "一份补话的名单行自署验不过——拒收。".into(),
                                        })
                                        .ok();
                                    continue;
                                };
                                // 去重（按见证人新 r）：同一见证人的重放只应用
                                // 一次；不同见证人的 RM 各自应用（T2 不相交）
                                let fk = flood_key(circle_id, &witness_row.r[..]);
                                let mut p = inner.lock().await;
                                if p.flood_seen.contains(&fk) {
                                    continue;
                                }
                                if p.flood_seen.len() >= FLOOD_SEEN_CAP {
                                    p.flood_seen.clear();
                                }
                                p.flood_seen.insert(fk);
                                // 圈对不上：不是我信念里的圈，一句不转发
                                if p
                                    .gathering
                                    .as_ref()
                                    .is_none_or(|g| g.circle_id != circle_id)
                                {
                                    continue;
                                }
                                // 见证人须在名单，且行内嗓音公钥与名单一致
                                //（A1：行名可伪、嗓音不可顶替——堵『名字行
                                // 换钥』冒名洞；我自己的回声同理须钥一致）
                                let witness_ok = p.gathering.as_ref().is_some_and(|g| {
                                    if witness_row.name == g.my_row.name {
                                        witness_row.sig_pk == g.my_row.sig_pk
                                    } else {
                                        g.members
                                            .get(&witness_row.name)
                                            .is_some_and(|m| m.sig_pk == witness_row.sig_pk)
                                    }
                                });
                                if !witness_ok {
                                    p.events
                                        .send(Event::CircleRejected {
                                            circle_id,
                                            detail: format!(
                                                "〔{}〕不在名单上或嗓音对不上，却署了一份补话——\
                                                 外人冒见证人，拒收。",
                                                witness_row.name
                                            ),
                                        })
                                        .ok();
                                    continue;
                                }
                                // 见证人整帧签名验证（帧里的名字行是她的印）
                                let mut verified = false;
                                if let Ok(sig) = ed25519_dalek::Signature::from_slice(sig_bytes) {
                                    verified = witness_row
                                        .sig_pk
                                        .verify_strict(&body[..body.len() - 64], &sig)
                                        .is_ok();
                                }
                                if !verified {
                                    p.events
                                        .send(Event::CircleRejected {
                                            circle_id,
                                            detail: format!(
                                                "署名〔{}〕的补话整帧验签不过——伪造补话拒收。",
                                                witness_row.name
                                            ),
                                        })
                                        .ok();
                                    continue;
                                }
                                // r 唯一性（P-12）：见证人新 r 与其他成员碰撞
                                // → 响亮拒绝（排除见证人自己的旧行——旋转不是碰撞）
                                let collide = g_check_r_collision(&p, &witness_row);
                                if collide {
                                    p.events
                                        .send(Event::CircleRejected {
                                            circle_id,
                                            detail: "补话携带的 r 与现有成员碰撞，响亮拒绝（P-12）。".into(),
                                        })
                                        .ok();
                                    continue;
                                }
                                // 在场辩护（收端）：还在说话的人不可被判离开
                                let Some(g) = p.gathering.as_mut() else {
                                    continue;
                                };
                                if leaver != g.my_row.name && g.presence_blocked(leaver) {
                                    p.events
                                        .send(Event::CircleConflict {
                                            circle_id,
                                            detail: format!(
                                                "〔{leaver}〕 的离席补话被扣下：\
                                                 她 30 秒内还在圈内说话（RM 在场辩护）。\
                                                 若确认她该离席，请重跑 /circle 点名重聚。"
                                            ),
                                        })
                                        .ok();
                                    continue; // 扣下：不应用、不转发，交给人类
                                }
                                // 应用：条件删除 + 无条件行替换（幂等收敛）
                                let (members, change) = g.apply_rm(leaver, &witness_row);
                                p.events
                                    .send(Event::CircleRekeyed { circle_id, members, change })
                                    .ok();
                                drop(p);
                                // 信念路由转发（leaver 已除名——断递，P-04）
                                belief_flood(&inner, Some(id), plain[0], &plain[1..n]).await;
                                // P16 三发之一（换气后）：RM 也是换气（幂等补发）
                                send_line_proofs(&inner).await;
                            }
                        }
                        frame_kind::CIRCLE_BYE => {
                            // 告退（v3·署名）：告别是身体的行为（A1），必须
                            // 用嗓音钥签。布局：圈号 8 ‖ 名长 1 ‖ 名字 ‖ 署名 64
                            //（签 circle_id‖名长‖名字）。验不过 = 响亮拒收
                            //（P-01：伪造他人告退 = 无特权踢人，已死）。
                            // 一跳直接接收者（线的对端 == 告退者名）是见证人；
                            // 转发收到者只记账不发起（感知必须是一跳的）。
                            // 迟到告别的见证复活（2026-10-17 裁决）：另一
                            // 见证人的 RM 抢在直接告别帧之前到达时，直接
                            // 接收者经 members ∪ left_voices（嗓音墓碑）
                            // 验签后照常发自己的见证 RM——幂等应用正是
                            // rm_multiplicity 语义；提前跳过仅对非见证
                            //（转发）接收者保留，旧闻无害。
                            if n >= 1 + 8 + 1 + 1 + 64 {
                                let body = &plain[1..n];
                                let circle_id = u64::from_be_bytes(
                                    body[..8].try_into().expect("定长切8字节"),
                                );
                                let name_len = body[8] as usize;
                                if body.len() < 9 + name_len + 64 {
                                    continue; // 帧残缺：当没听见
                                }
                                let Ok(leaver) =
                                    std::str::from_utf8(&body[9..9 + name_len])
                                else {
                                    continue;
                                };
                                let mut p = inner.lock().await;
                                // 圈对不上：不是我信念里的圈
                                if p
                                    .gathering
                                    .as_ref()
                                    .is_none_or(|g| g.circle_id != circle_id)
                                {
                                    continue;
                                }
                                // P16：验签按 pk——对 members ∪ left_voices 的
                                // 全部（名, 钥）对试签（签名归属即身份，不按
                                // 名字查钥——their_name/载荷名不进查找路径）；
                                // 签名者行名须 == 载荷名（自洽：告别是身体的
                                // 行为，她只能签自己的告别，A1——与原「按名
                                // 查钥再验签」语义等价的 pk 化）。在座优先、
                                // 墓碑兜底（迟到告别的验签源——直接/转发接收
                                // 者验签面统一，后续见证判定分野不变）。
                                let signer: Option<(String, ed25519_dalek::VerifyingKey)> = {
                                    let Some(g) = p.gathering.as_ref() else {
                                        continue;
                                    };
                                    let payload = bye_payload(circle_id, leaver.as_bytes());
                                    ed25519_dalek::Signature::from_slice(
                                        &body[9 + name_len..9 + name_len + 64],
                                    )
                                    .ok()
                                    .and_then(|sig| {
                                        g.members
                                            .iter()
                                            .find(|(_, row)| {
                                                row.sig_pk.verify_strict(&payload, &sig).is_ok()
                                            })
                                            .map(|(n, row)| (n.clone(), row.sig_pk))
                                            .or_else(|| {
                                                g.left_voices
                                                    .iter()
                                                    .find(|(_, pk)| {
                                                        pk.verify_strict(&payload, &sig).is_ok()
                                                    })
                                                    .map(|(n, pk)| (n.clone(), *pk))
                                            })
                                    })
                                    .filter(|(n, _)| n == leaver) // 自洽：签名者==告退者
                                };
                                let Some((_, signer_pk)) = signer else {
                                    p.events
                                        .send(Event::CircleRejected {
                                            circle_id,
                                            detail: format!(
                                                "自称〔{leaver}〕的署名告退验不过——\
                                                 伪造告别拒收（P-01）。"
                                            ),
                                        })
                                        .ok();
                                    continue; // 不转发：伪造帧到此为止
                                };
                                // 见证判定（P16）：线绑定锚 pk == 签名者 pk
                                //（她在这条线上亲手证明过自己——直接感知的
                                // 完成态）；锚缺席的窗口回退名字判定
                                //（Name/peer points_to——快照/bring/送达的
                                // 窗口信任映射）
                                let i_am_witness = p.gathering.as_ref().is_some_and(|g| {
                                    g.line_bindings.get(&id).is_some_and(|b| {
                                        b.anchor_pk() == Some(&signer_pk)
                                            || (b.anchor_pk().is_none()
                                                && b.points_to(leaver))
                                    })
                                });
                                // sig 键去重（2026-10-17 裁决 ②，验签之后——
                                // 集合只收有效签名，垃圾进不了集合）：sig 即
                                // 事件 ID（Ed25519 确定性）——同事件重放同
                                // sig 被去重，重入后新钥新 sig 放行。
                                let fk = flood_key(
                                    circle_id,
                                    &body[9 + name_len..9 + name_len + 64],
                                );
                                let fresh = !p.flood_seen.contains(&fk);
                                // 已见其署名告别——在场辩护豁免记账（两条
                                // 路径共用）。直接接收者的非新鲜副本也记：
                                // 见证义务可能待履行（转发副本先到、直接
                                // 副本迟到的交错），豁免要先于
                                // issue_witness_rm 就位（apply_rm 会再清账）
                                if fresh || i_am_witness {
                                    if let Some(g) = p.gathering.as_mut() {
                                        g.seen_bye.insert(leaver.to_string());
                                    }
                                }
                                if fresh {
                                    if p.flood_seen.len() >= FLOOD_SEEN_CAP {
                                        p.flood_seen.clear();
                                    }
                                    p.flood_seen.insert(fk);
                                    drop(p);
                                    // 转发（泛洪义务，信念路由）——只转新事件
                                    belief_flood(&inner, Some(id), plain[0], &plain[1..n]).await;
                                } else {
                                    drop(p);
                                }
                                // 见证人补话：她自己旋转 r、自署新行、签整帧。
                                // 直接副本无论新旧都触发（issue_witness_rm 的
                                // 义务幂等守卫兜底：重放副本与已履行的义务
                                // 安静空转；迟到副本在此复活）
                                if i_am_witness {
                                    issue_witness_rm(&inner, circle_id, leaver).await;
                                }
                            }
                        }
                        frame_kind::SEAT_OFFER => {
                            // 看座（0x10）：席位骨架（名字+嗓音公钥，绝无
                            // r——P-09：拒入者拿到 r 即可推导群钥）。
                            // 布局：圈号 8 ‖ 席位 1 ‖ 人数 1
                            // ‖（名长 1 ‖ 名字 ‖ 嗓音公钥 32）×。
                            // 同一条线的新邀约覆盖旧的（每次看座都是最新桌况）。
                            if n >= 1 + 10 {
                                let body = &plain[1..n];
                                let circle_id = u64::from_be_bytes(
                                    body[..8].try_into().expect("定长切8字节"),
                                );
                                let seats = body[8];
                                if seats > SEATS_OPEN {
                                    continue; // 席位标志不合法：当没听见
                                }
                                let count = body[9] as usize;
                                let mut pos = 10;
                                let mut table: Vec<(String, [u8; 32])> =
                                    Vec::with_capacity(count);
                                let mut ok = true;
                                for _ in 0..count {
                                    if body.len() < pos + 1 + 32 {
                                        ok = false;
                                        break;
                                    }
                                    let l = body[pos] as usize;
                                    if body.len() < pos + 1 + l + 32 {
                                        ok = false;
                                        break;
                                    }
                                    match std::str::from_utf8(&body[pos + 1..pos + 1 + l]) {
                                        Ok(name) => {
                                            let pk: [u8; 32] = body[pos + 1 + l
                                                ..pos + 1 + l + 32]
                                                .try_into()
                                                .expect("定长切32字节");
                                            table.push((name.to_string(), pk));
                                        }
                                        Err(_) => {
                                            ok = false;
                                            break;
                                        }
                                    }
                                    pos += 1 + l + 32;
                                }
                                if !ok || pos != body.len() {
                                    continue; // 骨架残缺：当没听见
                                }
                                let mut p = inner.lock().await;
                                p.seat_offers.insert(
                                    id,
                                    SeatOffer {
                                        circle_id,
                                        seats,
                                        introducer: their_name.clone(),
                                        table: table.clone(),
                                        my_seat: None,
                                    },
                                );
                                p.events
                                    .send(Event::SeatOffered {
                                        from: their_name.clone(),
                                        table: table
                                            .iter()
                                            .map(|(name, _)| name.clone())
                                            .collect(),
                                    })
                                    .ok();
                            }
                        }
                        frame_kind::SEAT_ACCEPT => {
                            // 落座回执（0x11）：候选人自署整行（与 0x09 行
                            // 同构）。期望门、P-04/P-12 检查、ADD 收尾都在
                            // complete_bring——一次手势，一次完整协议效果。
                            if n >= 1 + 1 + 32 + 32 + 64 {
                                complete_bring(&inner, id, &plain[1..n]).await;
                            }
                        }
                        frame_kind::SEAT_ROSTER => {
                            // 座次表（0x12）：全表含 r，只递已成成员——
                            // 新人由此计算群钥入圈。
                            handle_seat_roster(&inner, id, &plain[1..n]).await;
                        }
                        frame_kind::LINE_PROOF => {
                            // 线证明（0x13·P16）：载荷 = 圈号 8 ‖ 署名 64，
                            // 签名载荷 = 圈号 ‖ 本线 transcript。收端按
                            // 名单（members ∪ my_row）逐行试钥，恰一行
                            // 通过 ⇒ 该线锚定到该行 pk（已有绑定则升级
                            // proven_pk；未绑定线则授予 Proven——裁决点②，
                            // 半开边消解点）。全败/无行/圈号不符 = 当没
                            // 听见（无后果行为不适用静默=罪——合法证明
                            // 结构性必过，对照论证见 spec step_5）。
                            if n >= 1 + 8 + 64 {
                                handle_line_proof(&inner, id, &plain[1..n]).await;
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
                                            data: Zeroizing::default(),
                                        })
                                        .ok();
                                } else {
                                    p.inbound_files.insert(
                                        id,
                                        InboundFile {
                                            name,
                                            total,
                                            buf: Zeroizing::new(Vec::with_capacity(total as usize)),
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
                                let mut arrived: Option<(String, Zeroizing<Vec<u8>>)> = None;
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

    // 收摊：锁内只做同步收尾（摘对话/发事件/算焦点），一切 await 都在
    // 锁外——曾经把 shutdown 放在锁内 await：TCP 关线慢的窗口里，所有
    // 等 Inner 锁的任务（补话处理、看座收尾……）全部卡死，RM 帧到了
    // 也没人处理（上游 flaky 的根因，随 slice-4 修复）
    let (closed_writer, witness_ctx) = {
        let mut p = inner.lock().await;
        p.events
            .send(Event::Left {
                id,
                name: their_name.clone(),
                reason,
            })
            .ok();
        let closed = p.convos.remove(&id).map(|c| c.writer);
        // 线都断了，进出门的文件一并散伙（内存随之归还）
        p.inbound_files.remove(&id);
        p.outgoing_files.remove(&id);
        if p.focus == Some(id) {
            p.focus = p.convos.keys().max().copied();
            let to = p.focus;
            p.events.send(Event::FocusReturned { to }).ok();
        }
        // 圈内成员的线断了 = 我持有死线（A2：离席只对死线持有者可观察）
        // ——我是见证人，发起 RM 补话换钥：被缴/掉线的成员从此读不到
        // 新话（A3：换气只封存未来）。在场辩护照常先行（她刚说完话就
        // 掉线 → 扣下 + 响亮，人类点名裁决）。
        // P-15 + P16：判定读死线绑定的**锚**——pk 锚优先（锚 pk 对应
        // 行/墓碑 → 名字，RM 载荷需要名字；行删即熄 = 名字复用结构性
        // 不混淆）；锚缺席的窗口按名字（Name/peer——快照/bring/送达的
        // 窗口信任映射），指向的人须在 members ∪ left_voices（另一见证
        // 人的 RM 先到〔已除名+墓碑〕时，我的线死仍要发自己的 RM——
        // 幂等收敛 T2；义务幂等守卫兜底双触发）。未绑定的线（圈外语线）
        // 断了不见证——它从未承载圈语义（Proven 锚线属圈内：锚行在
        // 信念才路由/见证）。
        let witness_ctx: Option<(u64, String)> = p.gathering.as_ref().and_then(|g| {
            let binding = g.line_bindings.get(&id)?;
            let leaver = match binding.anchor_pk() {
                // pk 锚：锚 → 行（在座优先，墓碑兜底）→ 名字
                Some(pk) => g
                    .members
                    .values()
                    .find(|row| &row.sig_pk == pk)
                    .map(|row| row.name.clone())
                    .or_else(|| {
                        g.left_voices
                            .iter()
                            .find(|(_, v)| *v == pk)
                            .map(|(n, _)| n.clone())
                    })?,
                None => match binding {
                    LineBinding::Name { name, .. } => {
                        if g.members.contains_key(name) || g.left_voices.contains_key(name) {
                            name.clone()
                        } else {
                            return None;
                        }
                    }
                    // peer 键：资格线读**线对端**（不是授予者 via）
                    LineBinding::Qualified { peer, .. } => {
                        if g.members.contains_key(peer) || g.left_voices.contains_key(peer) {
                            peer.clone()
                        } else {
                            return None;
                        }
                    }
                    LineBinding::Proven { .. } => unreachable!("Proven 恒有锚"),
                },
            };
            Some((g.circle_id, leaver))
        });
        (closed, witness_ctx)
    };
    // 锁外关线（best effort——线已经死了，关不掉也无妨）
    if let Some(w) = closed_writer {
        let _ = w.lock().await.shutdown().await;
    }
    if let Some((circle_id, leaver)) = witness_ctx {
        issue_witness_rm(&inner, circle_id, &leaver).await;
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
    let mut plain = Zeroizing::new(Vec::with_capacity(1 + payload.len()));
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
    let mut plain = Zeroizing::new(Vec::with_capacity(3 + text.len() + 255));
    plain.push(frame_kind::PADDED_CHAT);
    plain.extend_from_slice(&(text.len() as u16).to_be_bytes());
    plain.extend_from_slice(text);
    // 随机垫：取不出随机数就发无垫（话照送，只是这一帧的形状不糊）
    let mut how_much = [0u8; 1];
    if getrandom::fill(&mut how_much).is_ok() && how_much[0] > 0 {
        let new_len = plain.len() + how_much[0] as usize;
        plain.resize(new_len, 0);
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
    let mut x = Zeroizing::new([0u8; 32]);
    if getrandom::fill(&mut x[..]).is_err() {
        return; // 造不出暗号就算了：链是便利，不是必需
    }
    if send_typed(cipher, writer, frame_kind::OFFER, &x[..]).await.is_err() {
        return; // 没送出去：作废
    }
    inner.lock().await.pending_offers.insert(convo_id, Zeroizing::new(x.to_vec()));
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
) -> Result<(TransportState, Zeroizing<[u8; 32]>), HandshakeFailure> {
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
) -> Result<(TransportState, Zeroizing<[u8; 32]>), HandshakeFailure> {
    // 第 1 件：暗号。有则先对，得出点火钥匙；无则跳过。
    // 对暗号也限时：占着切口不说话的，等不得（超时不算查验，切口归还）。
    let psk: Option<Zeroizing<[u8; 32]>> = match secret {
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
        builder = builder.psk(0, &*psk).map_err(HandshakeFailure::early)?;
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
/// 返回（传输态, transcript 指纹）：指纹在转传输态之前取走——
/// 对词（/word·1:1 短词）由它派生，两端天然相同（P-08 的检测面）。
async fn noise_stage<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    mut hs: HandshakeState,
    as_initiator: bool,
) -> Result<(TransportState, Zeroizing<[u8; 32]>), Box<dyn std::error::Error + Send + Sync>> {
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

    // 指纹先取（into_transport_mode 会吃掉握手态）；套件定死 BLAKE2s ⇒ 32 字节
    let transcript: [u8; 32] = hs
        .get_handshake_hash()
        .try_into()
        .expect("BLAKE2s transcript 恒 32 字节");
    Ok((hs.into_transport_mode()?, Zeroizing::new(transcript)))
}

/// 对暗号（SPAKE2）：双方各发一条遮罩消息。
/// 暗号相同 → 双方得到同一把 32 字节点火钥匙；
/// 暗号不同 → 钥匙不同，随后的 Noise 握手在数学上无法通过。
/// 暗号本身从不出现在线上。
/// 半路断线的失败（early）不算查验过暗号：切口可原样归还；
/// 遮罩都交换完、finish 跑过（对错都算）才算（checked）。
/// 返回 Zeroizing：点火钥匙交给 Noise 后即焚（snow fork 侧的 psks
/// 同样是 Zeroizing，整条链不留明文副本）。
async fn pake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    as_initiator: bool,
    secret: &[u8],
) -> Result<Zeroizing<[u8; 32]>, HandshakeFailure> {
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
    let key = Zeroizing::new(
        state
            .finish(&their_mask)
            .map_err(|_| HandshakeFailure::checked("暗号对不上。"))?,
    );
    let ignition: [u8; 32] = key[..]
        .try_into()
        .map_err(|_| HandshakeFailure::checked("暗号钥匙长度不对。"))?;
    Ok(Zeroizing::new(ignition))
}

// ───────────────────────────────────────────────────────────────
// 判例注入面（P3 slice-6，settled 2026-10-16 rogue_injection_feature_gated）
//
// `#[cfg(feature = "rogue")]`——默认构建编译期零注入符号（结构解：
// doc(hidden)+约定不调用 = 检测式编码，P-10 判例同族，被否）。
// 注入即恶意对端能力的实验室化身：一切注入经真实发送路径
// （每线 Noise 加密），被测的正是收端验证——T0 界外无攻击面增量。
// 注意 #[cfg(test)] 干不了这活：集成测试编译库时 cfg(test) 关闭。
// 跑法：cargo test --features rogue；缺 feature 时 canary 测试响亮失败。
// ───────────────────────────────────────────────────────────────
#[cfg(feature = "rogue")]
pub mod rogue {
    use super::*;

    /// 帧类型常量（注入面引用——线格式见 frame_kind 与 README_AGENT）
    pub const CONTRIB_V2: u8 = super::frame_kind::CONTRIB_V2;
    pub const CIRCLE_BYE: u8 = super::frame_kind::CIRCLE_BYE;
    pub const SEAT_ACCEPT: u8 = super::frame_kind::SEAT_ACCEPT;
    pub const TOPUP: u8 = super::frame_kind::TOPUP;
    pub const GENESIS: u8 = super::frame_kind::GENESIS;
    pub const LINE_PROOF: u8 = super::frame_kind::LINE_PROOF;

    impl Person {
        /// 在指定 1:1 线上注入任意帧（经真实发送路径：该线 Noise 加密）。
        /// 恶意/故障对端的本职能力——P-12 的注入通道（0x11 回执侧）。
        pub async fn rogue_frame(
            &self,
            convo: u64,
            kind: u8,
            payload: &[u8],
        ) -> Result<(), PersonError> {
            let (cipher, writer) = {
                let p = self.inner.lock().await;
                let Some(c) = p.convos.get(&convo) else {
                    return Err(PersonError::NoConvo);
                };
                (c.cipher.clone(), c.writer.clone())
            };
            send_typed(&cipher, &writer, kind, payload).await
        }

        /// 以圈内成员身份泛洪一帧圈帧（信念路由——恶意成员的真实
        /// 路径）。P-01/P-03/P-04/P-05 的注入通道。注意：不预插泛洪
        /// 去重键（诚实发起路径才做回声免疫）——注入者可能收到自己
        /// 帧的回声，那正是幼稚恶意成员的真实处境，无害于测试断言。
        pub async fn rogue_flood(&self, kind: u8, payload: &[u8]) {
            belief_flood(&self.inner, None, kind, payload).await;
        }

        /// 当前圈的圈号（非机密：随帧可见的正当信息）。
        pub async fn circle_id(&self) -> Option<u64> {
            self.inner.lock().await.gathering.as_ref().map(|g| g.circle_id)
        }

        /// 当前信念名单快照（0x12 同款全表内容——只递成员的正当材料）。
        pub async fn roster_snapshot(&self) -> Option<Vec<crate::circle::RosterRow>> {
            let p = self.inner.lock().await;
            let g = p.gathering.as_ref()?;
            Some(
                g.members
                    .values()
                    .cloned()
                    .chain(std::iter::once(g.my_row.clone()))
                    .collect(),
            )
        }

        /// 当前圈的线绑定全貌（P-15/P16 判例断言面）：每线一条
        /// (对话编号, 线对端自报名, 绑定描述)。绑定描述：
        /// "name:名[+pk]" / "qualified:via=… peer=…[+pk]" / "proven: pk"——
        /// +pk 后缀 = pk 锚已建立（P16 proven_pk/Proven，锚=pk 前 8 hex）。
        /// their_name 仅供测试可读性对照。
        pub async fn line_bindings_debug(&self) -> Option<Vec<(u64, String, String)>> {
            let p = self.inner.lock().await;
            let g = p.gathering.as_ref()?;
            let mut v: Vec<(u64, String, String)> = Vec::new();
            for (id, c) in &p.convos {
                if let Some(b) = g.line_bindings.get(id) {
                    let anchor = b
                        .anchor_pk()
                        .map(|pk| format!("+{:02x}{:02x}{:02x}{:02x}", pk.as_bytes()[0], pk.as_bytes()[1], pk.as_bytes()[2], pk.as_bytes()[3]))
                        .unwrap_or_default();
                    let desc = match b {
                        LineBinding::Name { name, .. } => format!("name:{name}{anchor}"),
                        LineBinding::Qualified { via, peer, .. } => {
                            format!("qualified:via={via} peer={peer}{anchor}")
                        }
                        LineBinding::Proven { pk } => {
                            format!("proven:{:02x}{:02x}{:02x}{:02x}", pk.as_bytes()[0], pk.as_bytes()[1], pk.as_bytes()[2], pk.as_bytes()[3])
                        }
                    };
                    v.push((*id, c.their_name.clone(), desc));
                }
            }
            v.sort();
            Some(v)
        }

        /// 我方待验的开台公告原文（被邀请者侧的 genesis_payload）——
        /// spec (a) 转发测试的注入材料（把署名公告原样转给别人）。
        /// 发起者（天生已验，payload=None）返回 None。
        pub async fn pending_genesis(&self) -> Option<Vec<u8>> {
            let p = self.inner.lock().await;
            p.gathering
                .as_ref()
                .and_then(|g| g.genesis_payload.as_ref().map(|(_, b)| b.clone()))
        }
    }

    /// 伪造一枚自署名单行的线格式（判例注入材料）。行是自证的——
    /// 任何名字都能用自己的钥自签（名字归属由收端名单/见证裁决，
    /// 这正是 P-03/P-05 的攻击面）；r = Some 指定（P-12 碰撞注入），
    /// None 则随机。P-04 的「自署验不过」材料由调用方把尾部 64 字节
    /// 置零得到（全零签名确定性验不过）。
    pub fn forge_row(circle_id: u64, name: &str, r: Option<&[u8]>) -> Vec<u8> {
        let mut seed = Zeroizing::new([0u8; 32]);
        let _ = getrandom::fill(&mut seed[..]);
        let sig = ed25519_dalek::SigningKey::from_bytes(&seed);
        let row = match r {
            Some(r) => crate::circle::RosterRow::seat_with_r(circle_id, name, r, &sig),
            None => crate::circle::RosterRow::self_seat(circle_id, name, &sig),
        };
        row.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* ── v3 圈机制的单元测试（判例库的纯逻辑面）── */

    /// 定种子的嗓音钥（测试可复现；生产钥永远是随机的）。
    fn seeded_key(seed: u8) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
    }

    /// 造一行**定值**贡献的名单行（确定性测试装置；contrib_sig 不参与
    /// 本组测试的验证路径——验签逻辑另有专属测试）。
    fn fixed_row(name: &str, seed: u8, r_val: u8) -> crate::circle::RosterRow {
        let sig = seeded_key(seed);
        crate::circle::RosterRow {
            name: name.to_string(),
            r: Zeroizing::new(vec![r_val; 32]),
            sig_pk: sig.verifying_key(),
            contrib_sig: sig.sign(name.as_bytes()),
        }
    }

    /// 造一场四人圈（我=Alice，另三人各有定值行），铸钥完毕。
    /// 全部输入确定——两次调用终态可逐位比较（交换代数断言的前提）。
    fn four_person_gathering() -> Gathering {
        let cid = 0x0E2E_u64;
        let sig = seeded_key(1);
        let mut invited = std::collections::HashSet::from(["Alice".to_string()]);
        let mut members = HashMap::new();
        for (name, seed, r_val) in [("Bob", 2u8, 0x01u8), ("Carol", 3, 0x02), ("Dave", 4, 0x03)] {
            invited.insert(name.to_string());
            members.insert(name.to_string(), fixed_row(name, seed, r_val));
        }
        let mut g = Gathering {
            circle_id: cid,
            seats: SEATS_CLOSED,
            sig,
            my_row: fixed_row("Alice", 1, 0xA0),
            members,
            invited: Some(invited),
            genesis_payload: None,
            genesis_verified: true,
            key: None,
            counter: 0,
            presence: HashMap::new(),
            seen_bye: std::collections::HashSet::new(),
            left_voices: HashMap::new(),
            witnessed_departs: std::collections::HashSet::new(),
            line_bindings: HashMap::new(),
        };
        assert_eq!(g.try_complete(), Some(4), "四人到齐即铸钥");
        g
    }

    /// 终态快照：成员名集合 + 全部贡献 + 群钥（交换代数断言的比较面）。
    fn snapshot(g: &Gathering) -> (Vec<String>, Vec<Vec<u8>>, [u8; 32]) {
        let mut names: Vec<String> = g.members.keys().cloned().collect();
        names.push(g.my_row.name.clone());
        names.sort();
        let mut rs: Vec<Vec<u8>> = g.members.values().map(|m| m.r.to_vec()).collect();
        rs.push(g.my_row.r.to_vec());
        rs.sort();
        let k = g.key.as_ref().map(|k| **k).unwrap_or([0u8; 32]);
        (names, rs, k)
    }

    #[test]
    fn 署名告退_验签逻辑_伪造拒收() {
        // P-01：告别是身体的行为——只有告退者本人的嗓音钥签的才算数
        let cid = 0x0E2E_u64;
        let carol_key = seeded_key(3);
        let carol_row = crate::circle::RosterRow::self_seat(cid, "Carol", &carol_key);
        // 本人签的：验过
        let good = carol_key.sign(&bye_payload(cid, b"Carol"));
        assert!(carol_row
            .sig_pk
            .verify_strict(&bye_payload(cid, b"Carol"), &good)
            .is_ok());
        // 别人替她签的（代笔）：验不过
        let mallory_key = seeded_key(9);
        let forged = mallory_key.sign(&bye_payload(cid, b"Carol"));
        assert!(carol_row
            .sig_pk
            .verify_strict(&bye_payload(cid, b"Carol"), &forged)
            .is_err());
        // 拿她的签名改名（拼接攻击）：验不过（载荷定界起效）
        assert!(carol_row
            .sig_pk
            .verify_strict(&bye_payload(cid, b"Bob"), &good)
            .is_err());
    }

    #[test]
    fn 嗓音墓碑_生死周期_重入清账再离场重写() {
        // 2026-10-17 裁决：apply_rm 除名写墓碑（公钥入档）；apply_add
        // 同名重入清账（墓碑/见证义务/在场豁免——新任期新账）；再离场
        // 用**新公钥**重写墓碑（重入生成新嗓音钥，seat flow 定稿行为）。
        let cid = 0x0E2E_u64;
        let mut g = four_person_gathering();
        let carol_pk = g.members.get("Carol").unwrap().sig_pk;
        // 第一次离席：墓碑写入 Carol 的公钥
        let mut bob_row = g.members.get("Bob").unwrap().clone();
        bob_row.rotate_and_resign(cid, &seeded_key(2));
        g.apply_rm("Carol", &bob_row);
        assert_eq!(g.left_voices.get("Carol"), Some(&carol_pk), "除名写墓碑");
        assert!(!g.members.contains_key("Carol"));
        // 见证义务记账（issue_witness_rm 履行时插入；此处直插模拟履行）
        g.witnessed_departs.insert("Carol".into());
        g.seen_bye.insert("Carol".into());
        // 重入（同名、新钥——sit 的新嗓音钥）：三本账全清
        let carol_new = fixed_row("Carol", 33, 0x0C); // 种子 33 = 新钥
        let mut bob_row2 = g.members.get("Bob").unwrap().clone();
        bob_row2.rotate_and_resign(cid, &seeded_key(2));
        g.apply_add(&carol_new, &bob_row2);
        assert!(g.left_voices.get("Carol").is_none(), "重入清墓碑（裁决①）");
        assert!(!g.witnessed_departs.contains("Carol"), "重入清见证义务（新任期新义务）");
        assert!(!g.seen_bye.contains("Carol"), "重入清在场豁免（旧告别不豁免新任期）");
        assert_eq!(g.members.get("Carol").unwrap().sig_pk, carol_new.sig_pk);
        // 再离席：墓碑以**新公钥**重写（旧公钥的告别验不过新墓碑——
        // sig 键去重 + 墓碑更新共同保证第二次告别不被吞）
        let mut bob_row3 = g.members.get("Bob").unwrap().clone();
        bob_row3.rotate_and_resign(cid, &seeded_key(2));
        g.apply_rm("Carol", &bob_row3);
        assert_eq!(
            g.left_voices.get("Carol"),
            Some(&carol_new.sig_pk),
            "再离席墓碑重写为新公钥"
        );
        // 成员优先于墓碑（裁决①优先级）：在座时 vk 查找必中 members
        let dave_pk = g.members.get("Dave").unwrap().sig_pk;
        assert_eq!(g.left_voices.get("Dave"), None, "未离席者无墓碑");
        let _ = dave_pk;
    }

    #[test]
    fn 线绑定_生死周期_对端键熄灭_授予键留溯源() {
        // P-15（peer 键修复，阻断项 B）+ P16（pk 锚，spec 裁决①②）：
        // 撤销键=线对端 peer / pk 锚（行删即熄，单一机制）；名字锚
        // 窗口态仅成键前路由（严格档）；Proven=未绑定线的第三授予。
        let cid = 0x0E2E_u64;
        let mut g = four_person_gathering();
        let carol_pk = g.members.get("Carol").unwrap().sig_pk;
        let dave_pk = g.members.get("Dave").unwrap().sig_pk;
        // 装上六条线：1→Name{Carol, 窗口}；2→Qualified{Carol,Carol}
        //（Carol 直连星形线）；3→Qualified{via:Alice, peer:Carol}（转发线）；
        // 4→Qualified{via:Alice, peer:Dave, proven_pk:Dave}（Dave 直连线，
        // 已锚定）；5→Qualified{via:Dave, peer:Carol}（对照二）；
        // 6→Proven{Dave}（未绑定线的第三授予，裁决点②）
        g.line_bindings.insert(
            1,
            LineBinding::Name { name: "Carol".into(), proven_pk: None },
        );
        g.line_bindings.insert(
            2,
            LineBinding::Qualified {
                via: "Carol".into(),
                peer: "Carol".into(),
                proven_pk: None,
            },
        );
        g.line_bindings.insert(
            3,
            LineBinding::Qualified {
                via: "Alice".into(),
                peer: "Carol".into(),
                proven_pk: None,
            },
        );
        g.line_bindings.insert(
            4,
            LineBinding::Qualified {
                via: "Alice".into(),
                peer: "Dave".into(),
                proven_pk: Some(dave_pk),
            },
        );
        g.line_bindings.insert(
            5,
            LineBinding::Qualified {
                via: "Dave".into(),
                peer: "Carol".into(),
                proven_pk: None,
            },
        );
        g.line_bindings
            .insert(6, LineBinding::Proven { pk: dave_pk });
        // 严格档（成键后名字锚熄灭）：four_person_gathering 已成键
        let (belief, belief_pks) = (g.belief_names(), g.belief_pks());
        assert!(
            !g.line_bindings
                .get(&1)
                .unwrap()
                .routes(&belief, &belief_pks, true),
            "严格档：成键后名字锚窗口线熄灭（spec 裁决①——窗口围栏）"
        );
        assert!(
            !g.line_bindings
                .get(&2)
                .unwrap()
                .routes(&belief, &belief_pks, true),
            "严格档：无锚的 Qualified 同熄（凑话期路径，成键即熄）"
        );
        assert!(
            g.line_bindings
                .get(&4)
                .unwrap()
                .routes(&belief, &belief_pks, true),
            "pk 锚线：锚行（Dave）在信念 ⇒ 路由（成键前后无差）"
        );
        assert!(
            g.line_bindings
                .get(&6)
                .unwrap()
                .routes(&belief, &belief_pks, true),
            "Proven 线（第三授予）：锚行在信念 ⇒ 路由（裁决点②）"
        );
        // 窗口回退：未成键（凑话期）时名字锚按信念路由
        g.key = None;
        let belief = g.belief_names();
        assert!(
            g.line_bindings
                .get(&1)
                .unwrap()
                .routes(&belief, &belief_pks, false),
            "窗口态：成键前名字锚照常路由（凑话泛洪依赖）"
        );
        // Carol 离席（见证人 Bob 旋转）：peer==Carol 线熄灭（2/3/5）；
        // pk 锚线不牵连（4/6 锚 Dave）；Name 窗口绑定保留（迟到告别判定）
        let mut bob_row = g.members.get("Bob").unwrap().clone();
        bob_row.rotate_and_resign(cid, &seeded_key(2));
        g.apply_rm("Carol", &bob_row);
        assert!(
            g.line_bindings.get(&1).is_some_and(|b| b.points_to("Carol")),
            "Name 窗口绑定保留（迟到告别判定——pk 锚缺席时的回退路径）"
        );
        assert!(g.line_bindings.get(&2).is_none(), "直连星形线随 RM(Carol) 熄灭（P-04）");
        assert!(
            g.line_bindings.get(&3).is_none(),
            "转发线随**对端** Carol 离席熄灭（阻断项 B：撤销键在 via 时此线续命泄漏新 K）"
        );
        assert!(g.line_bindings.get(&5).is_none(), "第二转发面同熄");
        assert!(
            g.line_bindings.get(&4).is_some(),
            "Qualified{{peer:Dave, 锚:Dave}} 不受 RM(Carol) 牵连（幸存者收敛）"
        );
        // pk 锚统一熄灭（P16 核心机制）：Dave 离席 ⇒ 锚==Dave.pk 的一切
        // 线熄灭（4 的锚 + 6 的 Proven）——不需要任何按名字的撤销循环
        let mut bob_row4 = g.members.get("Bob").unwrap().clone();
        bob_row4.rotate_and_resign(cid, &seeded_key(2));
        g.apply_rm("Dave", &bob_row4);
        assert!(
            g.line_bindings.get(&4).is_none(),
            "RM(Dave) 熄灭锚线（proven_pk==Dave.pk，行删即熄）"
        );
        assert!(
            g.line_bindings.get(&6).is_none(),
            "RM(Dave) 熄灭 Proven 线（同一机制——单一熄灭语义）"
        );
        let _ = carol_pk;
    }

    #[test]
    fn 线证明_载荷绑定本线_跨线重放必败() {
        // P16 单元（spec §6 单元面）：proof_payload = circle_id ‖ transcript
        //（定长定界）；同线同载荷可验、异线（transcript 不同）验不过、
        // 换圈（circle_id 不同）验不过——截获重放的两条死路。
        let cid = 0x0E2E_u64;
        let sk = seeded_key(9);
        let t_a = [0x11u8; 32];
        let t_b = [0x22u8; 32];
        let sig = sk.sign(&crate::circle::proof_payload(cid, &t_a));
        let vk = sk.verifying_key();
        assert!(
            vk.verify_strict(&crate::circle::proof_payload(cid, &t_a), &sig)
                .is_ok(),
            "同线同圈：验过"
        );
        assert!(
            vk.verify_strict(&crate::circle::proof_payload(cid, &t_b), &sig)
                .is_err(),
            "跨线重放（transcript 不同）：验不过——证明绑死本线"
        );
        assert!(
            vk.verify_strict(&crate::circle::proof_payload(cid + 1, &t_a), &sig)
                .is_err(),
            "旧圈证明在新圈（circle_id 不同）：验不过"
        );
    }

    #[test]
    fn 在场辩护_窗口与豁免() {
        let mut g = four_person_gathering();
        // 无证据：不拦（沉默成员可被伪证 RM——入档不修的诚实边界）
        assert!(!g.presence_blocked("Carol"));
        // 她刚说话（30 秒窗口内）：拦——还在说话即不可被判离开
        g.presence.insert("Carol".into(), std::time::Instant::now());
        assert!(g.presence_blocked("Carol"));
        // 已见其署名告别：豁免（两条路径共用的例外条款）
        g.seen_bye.insert("Carol".into());
        assert!(!g.presence_blocked("Carol"));
        g.seen_bye.remove("Carol");
        // 窗口外（老黄历）：不拦
        let stale = std::time::Instant::now()
            .checked_sub(PRESENCE_WINDOW + PRESENCE_WINDOW)
            .expect("时钟回拨的极端环境不支持测试");
        g.presence.insert("Carol".into(), stale);
        assert!(!g.presence_blocked("Carol"));
    }

    #[test]
    fn 交换代数_双见证人RM_任意顺序同终态() {
        // P-06：Carol 离席有两个直接对端（Bob、Dave）——各自发 RM
        //（删同一人 + 各旋转各自的行）= 不相交变更，T2 序无关收敛
        let cid = 0x0E2E_u64;
        // 顺序 1：先 Bob 的 RM，后 Dave 的 RM
        let mut g1 = four_person_gathering();
        let mut bob_row = g1.members.get("Bob").unwrap().clone();
        bob_row.rotate_and_resign(cid, &seeded_key(2));
        let mut dave_row = g1.members.get("Dave").unwrap().clone();
        dave_row.rotate_and_resign(cid, &seeded_key(4));
        g1.apply_rm("Carol", &bob_row);
        g1.apply_rm("Carol", &dave_row);
        let s1 = snapshot(&g1);
        // 顺序 2：先 Dave 后 Bob
        let mut g2 = four_person_gathering();
        g2.apply_rm("Carol", &dave_row);
        g2.apply_rm("Carol", &bob_row);
        let s2 = snapshot(&g2);
        // 终态断言：同名同钥（幂等语义：条件删除 + 无条件行替换）
        assert_eq!(s1.0, s2.0, "成员名集合序无关收敛");
        assert_eq!(s1.1, s2.1, "贡献集合序无关收敛");
        assert_eq!(s1.2, s2.2, "群钥匙序无关收敛（T2 交换代数）");
        assert_eq!(s1.0, vec!["Alice".to_string(), "Bob".into(), "Dave".into()]);
        // 幂等：第三份 RM（同见证人重放不同 r）条件删除跳过、行替换照常
        let before = g1.headcount();
        let mut bob_row2 = g1.members.get("Bob").unwrap().clone();
        bob_row2.rotate_and_resign(cid, &seeded_key(2));
        g1.apply_rm("Carol", &bob_row2);
        assert_eq!(g1.headcount(), before, "重放 RM 不增不减人数");
        // 见证人 == 离场者：不复活（条件删除后跳过行替换）
        let mut g3 = four_person_gathering();
        let mut carol_row = g3.members.get("Carol").unwrap().clone();
        carol_row.rotate_and_resign(cid, &seeded_key(3));
        g3.apply_rm("Carol", &carol_row);
        assert!(!g3.members.contains_key("Carol"), "自我除名不复活（交换代数护栏）");
        assert_eq!(g3.headcount(), 3);
    }

    #[test]
    fn r碰撞与名单外_判定逻辑() {
        // P-12：r 碰撞必须能被看见（响亮拒绝的判定输入）
        let g = four_person_gathering();
        let bob_r = g.members.get("Bob").unwrap().r.to_vec();
        assert!(g.r_collides(&bob_r), "与现有成员同 r = 碰撞");
        assert!(!g.r_collides(&[0xEE; 32]), "新鲜 r 不碰撞");
        // P-03：名单外判定（计数死后的新守门员）
        assert!(g.on_roster("Carol"), "在册");
        assert!(!g.on_roster("Mallory"), "名单外");
        assert!(g.on_roster("Alice"), "我本人在册");
    }

    #[test]
    fn 相交变更_同旋转者二次领人_任意序必分叉() {
        // T2 的边界（slice-6）：不相交变更序无关（见上测试）；同旋转者
        // （Bob）的两次 ADD 相交——任意序必分叉：终态成员集合相同、
        // 贡献集合与群钥随应用序不同 ⇒ 双向 mumble（T1 接手，点名恢复）。
        // 诚实单源 TCP 有序投递下全员按同序应用（集成测试已证收敛）；
        // 分叉需要多路径乱序——此处把纯逻辑面钉死。
        let bob_row_at = |r_val: u8| {
            // Bob 的嗓音钥不变（sig_pk 一致性检查的前提），r 定值（确定性）
            let sig = seeded_key(2);
            crate::circle::RosterRow {
                name: "Bob".to_string(),
                r: Zeroizing::new(vec![r_val; 32]),
                sig_pk: sig.verifying_key(),
                contrib_sig: sig.sign(&format!("Bob@{r_val}").as_bytes()),
            }
        };
        let bob_1 = bob_row_at(0x11); // 第一次旋转后的行
        let bob_2 = bob_row_at(0x22); // 第二次旋转后的行
        let n1 = fixed_row("N1", 5, 0x0A);
        let n2 = fixed_row("N2", 6, 0x0B);
        // 顺序 1：先带 N1（Bob→r1），后带 N2（Bob→r2）——终态 Bob=r2
        let mut g1 = four_person_gathering();
        g1.apply_add(&n1, &bob_1);
        g1.apply_add(&n2, &bob_2);
        let s1 = snapshot(&g1);
        // 顺序 2：反序应用——终态 Bob=r1
        let mut g2 = four_person_gathering();
        g2.apply_add(&n2, &bob_2);
        g2.apply_add(&n1, &bob_1);
        let s2 = snapshot(&g2);
        assert_eq!(s1.0, s2.0, "成员集合序无关（行替换无条件）");
        assert_ne!(s1.1, s2.1, "贡献集合随 Bob 终态 r 分叉");
        assert_ne!(
            s1.2, s2.2,
            "同旋转者相交变更：应用序决定终态钥——必分叉（T1 响亮分歧，点名修复）"
        );
    }

    /* ── 原有的 1:1 帧逻辑测试 ── */

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
