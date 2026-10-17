//! 围坐一圈：多人共用的「屋里的话」。
//!
//! 凑话铸钥：每人一份 32 字节随机贡献，名单按字节排序后喂进 HKDF，
//! 各自铸出同一把群钥匙 K——无人能独断，也无人能预知。
//! 发言：每人从 K 派生自己的子钥 sᵢ = HKDF(K, rᵢ)，加密一次，
//! 同一份群密文从每条线扇出（外层仍是各线独立的 Noise 信封）。
//!
//! v3（名单即圈）：圈不是对象，是 N 个局部信念的对齐。名单（roster）
//! 是圈的唯一状态，每行自署（身体的印，A1）：名字 ‖ r ‖ 嗓音公钥 ‖ 贡献
//! 签名。行结构三载体同构——凑话 0x09、补话 0x0B 携带的行、座次表 0x12
//! 用同一个解析器同一个验证器，任何人伪造不了任何一行。

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// 一份凑话贡献的长度。
pub const CONTRIB_LEN: usize = 32;

/// 名字长度上限（帧内单字节计）。
pub const NAME_MAX: usize = 255;

/// 嗓音公钥长度（Ed25519）。
pub const SIG_PK_LEN: usize = 32;

/// 贡献签名长度（Ed25519）。
pub const SIG_LEN: usize = 64;

/// 群 AEAD 的 nonce 长度。
const NONCE_LEN: usize = 12;

/// 名单上的一行（v3：身体的印）。
///
/// 行的当前有效签名 = 最初的落座署名，或最近一次旋转她的补话签名
/// （r 旋转后原签名失效，旋转者必须随帧携带重签的整行）。
/// 每行自署是公理级不变量：伪造不了任何一行（A1）。
#[derive(Clone)]
pub struct RosterRow {
    pub name: String,
    /// 凑话贡献（用后即焚包装——密钥函数只借不拿）
    pub r: Zeroizing<Vec<u8>>,
    /// 嗓音公钥（验 0x0A 发言 / 0x0C 告别 / RM 见证签名）
    pub sig_pk: VerifyingKey,
    /// 自署：签 (circle_id ‖ 名长 ‖ 名字 ‖ r ‖ sig_pk)
    pub contrib_sig: Signature,
}

/// 自署行的签名载荷（定界防拼接歧义）。
fn row_payload(circle_id: u64, name: &str, r: &[u8], sig_pk: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + 1 + name.len() + r.len() + 32);
    v.extend_from_slice(&circle_id.to_be_bytes());
    v.push(name.len() as u8);
    v.extend_from_slice(name.as_bytes());
    v.extend_from_slice(r);
    v.extend_from_slice(sig_pk);
    v
}

impl RosterRow {
    /// 落座：自选 r、用嗓音钥自署整行（A1：落座署名必须本人触发）。
    pub fn self_seat(circle_id: u64, name: &str, sig: &SigningKey) -> Self {
        let mut r = Zeroizing::new(vec![0u8; CONTRIB_LEN]);
        getrandom::fill(&mut r[..]).expect("系统随机源不可用");
        let sig_pk = sig.verifying_key();
        let contrib_sig = sig.sign(&row_payload(circle_id, name, &r, sig_pk.as_bytes()));
        Self {
            name: name.to_string(),
            r,
            sig_pk,
            contrib_sig,
        }
    }

    /// r 旋转后重签自己的行（换气只封存未来——旧 r 从此作废）。
    pub fn rotate_and_resign(&mut self, circle_id: u64, sig: &SigningKey) {
        getrandom::fill(&mut self.r[..]).expect("系统随机源不可用");
        self.contrib_sig =
            sig.sign(&row_payload(circle_id, &self.name, &self.r, self.sig_pk.as_bytes()));
    }

    /// 验自署：这一行确实是这个名字的嗓音钥主人亲手按的印。
    pub fn verify_self(&self, circle_id: u64) -> bool {
        self.sig_pk
            .verify_strict(
                &row_payload(circle_id, &self.name, &self.r, self.sig_pk.as_bytes()),
                &self.contrib_sig,
            )
            .is_ok()
    }

    /// 行的线格式：名长 u8 ‖ 名字 ‖ r(32) ‖ sig_pk(32) ‖ contrib_sig(64)。
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(1 + self.name.len() + CONTRIB_LEN + 32 + 64);
        v.push(self.name.len() as u8);
        v.extend_from_slice(self.name.as_bytes());
        v.extend_from_slice(&self.r[..]);
        v.extend_from_slice(self.sig_pk.as_bytes());
        v.extend_from_slice(&self.contrib_sig.to_bytes());
        v
    }

    /// 从线格式取行。任何字段不合法 = None（坏实现与恶意对端的铁栏杆）。
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 1 + CONTRIB_LEN + 32 + 64 {
            return None;
        }
        let name_len = b[0] as usize;
        if b.len() != 1 + name_len + CONTRIB_LEN + 32 + 64 {
            return None;
        }
        let name = std::str::from_utf8(&b[1..1 + name_len]).ok()?.to_string();
        let r = Zeroizing::new(b[1 + name_len..1 + name_len + CONTRIB_LEN].to_vec());
        let pk: [u8; 32] = b[1 + name_len + CONTRIB_LEN..1 + name_len + CONTRIB_LEN + 32]
            .try_into()
            .ok()?;
        let sig_pk = VerifyingKey::from_bytes(&pk).ok()?;
        let contrib_sig =
            Signature::from_slice(&b[1 + name_len + CONTRIB_LEN + 32..]).ok()?;
        Some(Self {
            name,
            r,
            sig_pk,
            contrib_sig,
        })
    }

    /// 解析并验自署（收端标准路径：解析+验证一步到位）。
    pub fn parse_verified(b: &[u8], circle_id: u64) -> Option<Self> {
        let row = Self::from_bytes(b)?;
        row.verify_self(circle_id).then_some(row)
    }

    /// 定 r 落座（判例注入面的材料构造器）：r 由调用者指定——
    /// 生产路径永远是 [`RosterRow::self_seat`]（r 自选随机）。
    /// P-12 的碰撞注入（指定的 r 撞现有成员）用得上。
    #[cfg(feature = "rogue")]
    pub fn seat_with_r(circle_id: u64, name: &str, r: &[u8], sig: &SigningKey) -> Self {
        assert_eq!(r.len(), CONTRIB_LEN, "凑话贡献恒 32 字节");
        let sig_pk = sig.verifying_key();
        let contrib_sig = sig.sign(&row_payload(circle_id, name, r, sig_pk.as_bytes()));
        Self {
            name: name.to_string(),
            r: Zeroizing::new(r.to_vec()),
            sig_pk,
            contrib_sig,
        }
    }
}

/// 从全部贡献铸出群钥匙。贡献必须按字节排序后拼接——顺序无关，
/// 无需成员名单，人人得出同一把。
/// 借用切片入参（不移动贡献本体）——调用方的贡献存放于
/// Zeroizing 容器，密钥函数只看不拿。
pub fn derive_group_key(contribs: &[&[u8]]) -> [u8; 32] {
    let mut sorted: Vec<&[u8]> = contribs.to_vec();
    sorted.sort_unstable();
    let mut ikm = Vec::with_capacity(CONTRIB_LEN * sorted.len());
    for c in &sorted {
        ikm.extend_from_slice(c);
    }
    let hk = Hkdf::<Sha256>::new(Some(b"e2ee-circle"), &ikm);
    let mut k = [0u8; 32];
    hk.expand(b"group-key", &mut k).expect("32 字节恒成立");
    k
}

/// 从群钥匙与某人的贡献派生该人的发言子钥。
pub fn derive_sender_subkey(group_key: &[u8; 32], r: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(group_key), r);
    let mut s = [0u8; 32];
    hk.expand(b"sender-subkey", &mut s).expect("32 字节恒成立");
    s
}

/// 贡献的标签：用于 nonce 的发送者区分（取哈希前 4 字节）。
fn sender_tag(r: &[u8]) -> [u8; 4] {
    let h = Sha256::digest(r);
    [h[0], h[1], h[2], h[3]]
}

/// sender_tag 的公开面：v3 泛洪圈的发言帧用它标注「谁说的」，
/// 收方按标签在名单表里找说话人（子钥即署名的第一闸）。
pub fn sender_tag_of(r: &[u8]) -> [u8; 4] {
    sender_tag(r)
}

/// 用子钥加密一句话，返回可扇出的群密文：nonce ‖ AEAD。
/// counter 由调用者递增，保证同一发送者 nonce 永不重复。
pub fn seal(subkey: &[u8; 32], counter: u64, r: &[u8], text: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new_from_slice(subkey).expect("32 字节恒成立");
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..8].copy_from_slice(&counter.to_be_bytes());
    nonce[8..].copy_from_slice(&sender_tag(r));
    let ct = cipher
        .encrypt(&nonce.into(), Payload { msg: text, aad: &nonce })
        .expect("加密不应失败");
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

/// 用子钥解开群密文。失败 = 说话的人不对、或密文被动过。
pub fn open(subkey: &[u8; 32], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return None;
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new_from_slice(subkey).expect("32 字节恒成立");
    cipher
        .decrypt(
            nonce[..NONCE_LEN].try_into().ok()?,
            Payload { msg: ct, aad: nonce },
        )
        .ok()
}

/// SAS 对词（1:1·slice-5）：从 Noise transcript 哈希派生的 12 位数字
/// 短词（4 组 × 3 位，约 40bit）。两端 transcript 相同 ⇒ 同词；
/// MITM 把见面劈成两半 ⇒ 两个 transcript ⇒ 两半短词不同（P-08 的
/// 可检测性来源——无 SAS 则机密性定理无法陈述，T5）。
pub fn sas_word_from_transcript(transcript: &[u8]) -> String {
    sas_word(transcript, b"p2p-sas")
}

/// SAS 对词（群·slice-5）：HKDF(K, "circle-sas") 派生——全员同 K 同词，
/// 每次聚首新鲜（新圈号新贡献 ⇒ 新词），不累积不持久。
pub fn sas_word_from_group_key(k: &[u8; 32]) -> String {
    sas_word(k, b"circle-sas")
}

/// 短词蒸馏：IKM 与域标签喂进 HKDF，取 8 字节（64bit）模 10^12
/// （偏置约 1e-7，对 40bit 量级的仪式词可忽略），铸成 12 位数字
/// 分 4 组——电话念 12 位分组数字是数十年验证过的人类工程学。
fn sas_word(ikm: &[u8], label: &[u8]) -> String {
    let hk = Hkdf::<Sha256>::new(None, ikm);
    let mut out = [0u8; 8];
    hk.expand(label, &mut out).expect("8 字节恒成立");
    let v = u64::from_be_bytes(out) % 1_000_000_000_000;
    let s = format!("{v:012}");
    format!("{} {} {} {}", &s[0..3], &s[3..6], &s[6..9], &s[9..12])
}

/// 线证明的签名载荷（P16，rulings.2026_10_17_p16_spec_approved）：
/// circle_id ‖ 本线 transcript（8B‖32B 定长——定界无歧义）。per-line
/// transcript 把证明绑死在这条线上：截获的证明在别的线验签必败
///（transcript 不同 ⇒ 载荷不同），不可跨线重放（INV-P16-1）。
pub fn proof_payload(circle_id: u64, transcript: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + 32);
    v.extend_from_slice(&circle_id.to_be_bytes());
    v.extend_from_slice(transcript);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key(seed: u8) -> SigningKey {
        let mut s = Zeroizing::new([0u8; 32]);
        s[0] = seed;
        SigningKey::from_bytes(&s)
    }

    #[test]
    fn 排序无关_人人同钥() {
        let rs: Vec<Vec<u8>> = (0..4).map(|i| vec![i as u8; 32]).collect();
        let k1 = derive_group_key(&[&rs[0][..], &rs[1][..], &rs[2][..], &rs[3][..]]);
        let k2 = derive_group_key(&[&rs[3][..], &rs[2][..], &rs[1][..], &rs[0][..]]);
        assert_eq!(k1, k2, "贡献顺序不应影响群钥匙");
    }

    #[test]
    fn 子钥加密_指定人可解() {
        let rs: Vec<Vec<u8>> = (0..3).map(|i| vec![i as u8 + 1; 32]).collect();
        let k = derive_group_key(&[&rs[0][..], &rs[1][..], &rs[2][..]]);
        let s0 = derive_sender_subkey(&k, &rs[0]);
        let blob = seal(&s0, 7, &rs[0], "今晚吃火锅".as_bytes());
        assert_eq!(open(&s0, &blob).unwrap(), "今晚吃火锅".as_bytes());
        // 别人的子钥解不开
        let s1 = derive_sender_subkey(&k, &rs[1]);
        assert!(open(&s1, &blob).is_none());
        // 同一发送者 nonce 不重复：counter 不同密文不同
        let again = seal(&s0, 8, &rs[0], "今晚吃火锅".as_bytes());
        assert_ne!(blob, again);
    }

    #[test]
    fn 自署行_往返与防伪() {
        let cid = 0xABCD_1234_5678_9ABCu64;
        let alice = test_key(1);
        let row = RosterRow::self_seat(cid, "Alice", &alice);
        // 线格式往返无损
        let bytes = row.to_bytes();
        let back = RosterRow::parse_verified(&bytes, cid).expect("自署行应可验");
        assert_eq!(back.name, "Alice");
        assert_eq!(&back.r[..], &row.r[..]);
        // 换个圈号验 = 失败（签名绑圈）
        assert!(RosterRow::parse_verified(&bytes, cid + 1).is_none());
        // 篡改名字 = 自署失败（伪造不了别人的行）
        let mut forged = bytes.clone();
        forged[1] = b'X';
        assert!(RosterRow::parse_verified(&forged, cid).is_none());
        // Bob 用自己的钥签不出 Alice 的行（代笔死路）
        let bob = test_key(2);
        let ghost = RosterRow::self_seat(cid, "Alice", &bob);
        assert!(ghost.verify_self(cid), "Bob 造的行自洽——但名字归属由见证裁决");
        // 圈内名单以 sig_pk 归属：真 Alice 的行与 Bob 冒名的行 sig_pk 不同，
        // 同名冲突走响亮裁决（P-05），此处只钉行自证语义
        assert_ne!(ghost.sig_pk, row.sig_pk);
    }

    #[test]
    fn r旋转_重签后行仍可验_旧行失签() {
        let cid = 42u64;
        let alice = test_key(1);
        let mut row = RosterRow::self_seat(cid, "Alice", &alice);
        let old_sig = row.contrib_sig;
        row.rotate_and_resign(cid, &alice);
        // 新行可验（行的当前有效签名 = 最近一次旋转她的签名）
        assert!(row.verify_self(cid));
        // 旧签名不再匹配新 r（历史是历史，A3）
        let old_r_before = RosterRow {
            name: row.name.clone(),
            r: row.r.clone(),
            sig_pk: row.sig_pk,
            contrib_sig: old_sig,
        };
        assert!(!old_r_before.verify_self(cid), "旋转后行失签是常态，帧必须带重签整行");
    }

    #[test]
    fn 对词_格式与派生() {
        // slice-5：12 位数字分 4 组（~40bit）；同一 K 同词；换气即换词；
        // 1:1（transcript 面）与群（K 面）两个派生域彼此分离。
        let k1 = [7u8; 32];
        let k2 = [8u8; 32];
        let w1 = sas_word_from_group_key(&k1);
        assert_eq!(w1, sas_word_from_group_key(&k1), "确定性：全员同 K 必同词");
        assert_ne!(sas_word_from_group_key(&k2), w1, "换气即换词");
        // 格式：4 组、每组 3 位、纯数字
        assert_eq!(w1.split(' ').count(), 4, "12 位分 4 组：{w1}");
        for g in w1.split(' ') {
            assert_eq!(g.len(), 3);
            assert!(g.bytes().all(|b| b.is_ascii_digit()), "纯数字：{g}");
        }
        // 1:1 面同构：确定性 + 区分度 + 与群面域分离
        let t1 = [1u8; 32];
        assert_eq!(
            sas_word_from_transcript(&t1),
            sas_word_from_transcript(&t1),
            "两端 transcript 相同 ⇒ 同词"
        );
        assert_ne!(
            sas_word_from_transcript(&t1),
            sas_word_from_transcript(&[2u8; 32]),
            "拼线 ⇒ 两半 transcript 不同 ⇒ 异词（P-08）"
        );
        assert_ne!(
            sas_word_from_transcript(&t1),
            sas_word_from_group_key(&t1),
            "p2p-sas 与 circle-sas 域分离"
        );
    }

    #[test]
    fn 判例p13_共谋解史_引荐人旋转封住入座前历史() {
        // P-13 的密码学核心（确定性断言）：恶意载体存档全部密文 +
        // 共谋候选人入座 → 新人持 0x12 全表现行 r，也铸不出 K_old。
        // 旧圈：Alice、Bob(r_b1)、Carol；Bob 在 K_old 下说过话
        let r_a = [0x11u8; 32];
        let r_b1 = [0x22u8; 32];
        let r_c = [0x33u8; 32];
        let k_old = derive_group_key(&[&r_a, &r_b1, &r_c]);
        let sub_old = derive_sender_subkey(&k_old, &r_b1);
        let old_frame = seal(&sub_old, 1, &r_b1, "ru zuo qian de hua".as_bytes());

        // 反事实（修复的必要性）：若引荐人不旋转（r_b2 == r_b1），
        // 新人排除自己的 r 即重铸 K_old——攻击成立
        let k_no_rotation = derive_group_key(&[&r_a, &r_b1, &r_c]);
        assert_eq!(k_no_rotation, k_old, "不旋转 ⇒ 新人解旧历史（P-13 攻击面）");

        // 现实（P-13 旋转后）：Bob 转 r_b2、新人 N1 落座。
        // 新人手里的全部材料 = 0x12 全表：{r_a, r_c, r_b2, r_n}
        let r_b2 = [0x44u8; 32];
        let r_n = [0x55u8; 32];
        let k_new = derive_group_key(&[&r_a, &r_b2, &r_c, &r_n]);
        // 她能推导的候选钥：排除自己（假装算旧圈）与含自己（新圈）
        let k_guess = derive_group_key(&[&r_a, &r_b2, &r_c]);
        // 全部候选子钥（她持有的一切 r × 两把候选钥）都解不开旧帧——
        // K_old 缺 r_b1 这块，无从推导（密钥边界=成员边界）
        for (k, label) in [(k_new, "新圈钥"), (k_guess, "排除自己")] {
            for r in [&r_a, &r_c, &r_b2, &r_n] {
                let sub = derive_sender_subkey(&k, r);
                assert!(
                    open(&sub, &old_frame).is_none(),
                    "{label} × r={:#x} 的子钥解不开入座前旧帧",
                    r[0]
                );
            }
        }
        // 新圈的话照解（她自己的权益）
        let sub_new = derive_sender_subkey(&k_new, &r_b2);
        let new_frame = seal(&sub_new, 2, &r_b2, "ru zuo hou de hua".as_bytes());
        assert_eq!(
            open(&sub_new, &new_frame).unwrap(),
            "ru zuo hou de hua".as_bytes()
        );
    }
}
