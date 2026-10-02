//! 围坐一圈：多人共用的「屋里的话」。
//!
//! 凑话铸钥：每人一份 32 字节随机贡献，经自己的 Noise 线路广播给全员；
//! 所有人把全部贡献按字节排序后喂进 HKDF，各自铸出同一把群钥匙 K——
//! 无人能独断，也无人能预知。
//! 发言：每人从 K 派生自己的子钥 sᵢ = HKDF(K, rᵢ)，加密一次，
//! 同一份群密文从每条线扇出（外层仍是各线独立的 Noise 信封）。
//! 署名：解密用的子钥取决于「这条线对面的贡献」——线路即署名，
//! 谁也无法替别人开口。

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;

/// 一份凑话贡献的长度。
pub const CONTRIB_LEN: usize = 32;

/// 群 AEAD 的 nonce 长度。
const NONCE_LEN: usize = 12;

/// 从全部贡献铸出群钥匙。贡献必须按字节排序后拼接——顺序无关，
/// 无需成员名单，人人得出同一把。
pub fn derive_group_key(contribs: &[Vec<u8>]) -> [u8; 32] {
    let mut sorted: Vec<&[u8]> = contribs.iter().map(|c| c.as_slice()).collect();
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
    use sha2::Digest;
    let h = Sha256::digest(r);
    [h[0], h[1], h[2], h[3]]
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

    #[test]
    fn 排序无关_人人同钥() {
        let rs: Vec<Vec<u8>> = (0..4).map(|i| vec![i as u8; 32]).collect();
        let k1 = derive_group_key(&rs);
        let mut shuffled = rs.clone();
        shuffled.reverse();
        let k2 = derive_group_key(&shuffled);
        assert_eq!(k1, k2, "贡献顺序不应影响群钥匙");
    }

    #[test]
    fn 子钥加密_指定人可解() {
        let rs: Vec<Vec<u8>> = (0..3).map(|i| vec![i as u8 + 1; 32]).collect();
        let k = derive_group_key(&rs);
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
}
