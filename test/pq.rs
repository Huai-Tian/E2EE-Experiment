//! 后量子混成握手实验：验证 snow hfs 套件可用、往返成立。
//! 这是施工脚手架：通过后结论并入主握手。

use snow::Builder;

#[test]
fn 混成套件_解析并完成往返() {
    for suite in [
        "Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s",
        "Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s",
    ] {
        let params: snow::params::NoiseParams = suite.parse().expect("套件应能解析");

        let psk = [7u8; 32];
        let mut init = if suite.contains("psk0") {
            Builder::new(params.clone())
                .psk(0, &psk)
                .unwrap()
                .build_initiator()
                .expect("发起方应能构造")
        } else {
            Builder::new(params.clone())
                .build_initiator()
                .expect("发起方应能构造")
        };
        let mut resp = if suite.contains("psk0") {
            Builder::new(params)
                .psk(0, &psk)
                .unwrap()
                .build_responder()
                .expect("应答方应能构造")
        } else {
            Builder::new(params)
                .build_responder()
                .expect("应答方应能构造")
        };

        // 往返：I 写 → R 读 → R 写 → I 读（严格交替）
        let mut buf = vec![0u8; 4096];
        let n = init.write_message(&[], &mut buf).expect("I 写");
        let i2r = buf[..n].to_vec();
        resp.read_message(&i2r, &mut buf).expect("R 读");
        let n = resp.write_message(&[], &mut buf).expect("R 写");
        let r2i = buf[..n].to_vec();
        init.read_message(&r2i, &mut buf).expect("I 读");

        // 传输态收发
        let mut i_t = init.into_transport_mode().expect("I 传输态");
        let mut r_t = resp.into_transport_mode().expect("R 传输态");
        let n = i_t.write_message(b"pq ok", &mut buf).unwrap();
        let ct = buf[..n].to_vec();
        let mut plain = vec![0u8; ct.len()];
        let m = r_t.read_message(&ct, &mut plain).unwrap();
        assert_eq!(&plain[..m], b"pq ok");

        println!("{suite}: 发起帧 {}B 应答帧 {}B —— OK", i2r.len(), r2i.len());
    }
}
