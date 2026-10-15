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
}
