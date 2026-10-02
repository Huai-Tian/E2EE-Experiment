//! 集成测试：真实起人、真实握手、真实密谈——不断言内部，只断言经历。
//!
//! 每个测试都是同一台机器上的一场小型相遇。

use e2ee::{Event, LeaveReason, Person};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{timeout, Instant};

/// 等到事件流里出现满足条件的事件（跳过无关事件），超时则失败。
async fn expect<T>(
    events: &mut UnboundedReceiver<Event>,
    cond: impl Fn(&Event) -> Option<T>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let ev = timeout(left, events.recv())
            .await
            .expect("事件流不应关闭")
            .expect("事件流不应枯竭");
        if let Some(got) = cond(&ev) {
            return got;
        }
    }
}

/// 从事件里取出 Met（对方名字、编号、是否聚焦）。
fn met(ev: &Event) -> Option<(String, u64, bool)> {
    if let Event::Met { id, name, focused } = ev {
        Some((name.clone(), *id, *focused))
    } else {
        None
    }
}

/// 找一个没人占用的本地端口（先占后放，随后立刻使用）。
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// 出生在随机端口，返回（人，事件流，可拨地址）。
async fn spawn_person(name: &str) -> (Person, UnboundedReceiver<Event>, String) {
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let (p, e) = Person::born(name, &addr).await.expect("出生不应失败");
    (p, e, addr)
}

#[tokio::test]
async fn 密谈_双向说话与听见() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _bob_addr) = spawn_person("Bob").await;

    // Bob 主动去找 Alice
    bob.dial(&alice_addr).await.expect("拨通");

    let (a_name, _, a_focused) = expect(&mut alice_ev, met).await;
    assert_eq!(a_name, "Bob");
    assert!(a_focused, "空手时的第一位来客应成为注意力所在");

    let (b_name, _, b_focused) = expect(&mut bob_ev, met).await;
    assert_eq!(b_name, "Alice");
    assert!(b_focused, "主动去找的人应聚焦这场对话");

    // Bob 说，Alice 听见
    bob.speak("听得到吗").await.expect("Bob 发言");
    let (heard_from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!(heard_from, "Bob");
    assert_eq!(text, "听得到吗");

    // Alice 回话，Bob 听见
    alice.speak("听得到").await.expect("Alice 回话");
    let (heard_from, text) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!(heard_from, "Alice");
    assert_eq!(text, "听得到");
}

#[tokio::test]
async fn 道别_对方干净收场() {
    let (_alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _bob_addr) = spawn_person("Bob").await;

    bob.dial(&alice_addr).await.expect("拨通");
    expect(&mut bob_ev, met).await;

    // Bob 道别（空明文帧）
    let who = bob.bye().await.expect("道别");
    assert_eq!(who, "Alice");

    // Alice 看到「挂断了」，并完成收摊
    let (name, was_farewell) = expect(&mut alice_ev, |ev| {
        if let Event::Left { name, reason, .. } = ev {
            Some((name.clone(), matches!(reason, LeaveReason::Farewell)))
        } else {
            None
        }
    })
        .await;
    assert_eq!(name, "Bob");
    assert!(was_farewell, "应为道别而非断线");

    // Bob 侧也会因线路关闭而收摊
    let name = expect(&mut bob_ev, |ev| {
        if let Event::Left { name, .. } = ev {
            Some(name.clone())
        } else {
            None
        }
    })
        .await;
    assert_eq!(name, "Alice");
}

#[tokio::test]
async fn 来客不抢注意力() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (_bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (carol, _carol_ev, _carol_addr) = spawn_person("Carol").await;

    // Alice 主动去找 Bob —— 注意力在 Bob
    alice.dial(&bob_addr).await.expect("Alice 找 Bob");

    // Carol 找上门来 —— 不应抢走 Alice 的注意力
    carol.dial(&alice_addr).await.expect("Carol 找 Alice");

    let (a_met_bob, _, _) = expect(&mut alice_ev, met).await;
    assert_eq!(a_met_bob, "Bob");

    let (a_met_carol, _carol_id, focused) = expect(&mut alice_ev, met).await;
    assert_eq!(a_met_carol, "Carol");
    assert!(!focused, "来客不应抢走已有的注意力");

    // 注意力仍在 Bob：Alice 说话只有 Bob 听见
    alice.speak("还在吗").await.expect("Alice 对 Bob 说");
    let (heard, _) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!(heard, "Alice");
}

#[tokio::test]
async fn 名片_每网卡一行() {
    let (alice, _ev, _addr) = spawn_person("Alice").await;
    let lines = alice.card().await;
    assert!(!lines.is_empty(), "至少应有一张网卡");
    for l in &lines {
        assert!(l.starts_with("Alice  "), "名片行应以名字开头：{l}");
        assert!(l.contains(':'), "名片行应含地址：{l}");
    }
}

/// 起一间茶馆在随机端口，返回它的地址。
async fn spawn_teahouse() -> String {
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let bind = addr.clone();
    tokio::spawn(async move {
        let _ = e2ee::courier::serve(&bind).await;
    });
    // 给茶馆一瞬开张的时间
    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

#[tokio::test]
async fn 茶馆_两人经房密谈() {
    let teahouse = spawn_teahouse().await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // Alice 先进房等，Bob 随后到
    let alice_waiter = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_waiter.meet_at_teahouse(&t, 4242).await;
    });
    tokio::time::sleep(Duration::from_millis(600)).await;
    bob.meet_at_teahouse(&teahouse, 4242).await.expect("Bob 进房");

    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 双向说话——这条线整段流经茶馆，但茶馆一个字也读不了
    bob.speak("穿墙的话").await.expect("Bob 说");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "穿墙的话"));

    alice.speak("两头都通").await.expect("Alice 说");
    let (from, text) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Alice", "两头都通"));
}

#[tokio::test]
async fn 暗号_对得上才谈得成() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;
    let (mallory, _mallory_ev, _m) = spawn_person("Mallory").await;

    // Alice 备好切口；Bob 对得上，谈成
    alice.expect_secret(b"lan-tong-qi-hao").await;
    bob.dial_secret(&alice_addr, Some(b"lan-tong-qi-hao"))
        .await
        .expect("对得上应谈成");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // Mallory 暗号错了：谈不成（响亮失败，绝不无声放行）
    alice.expect_secret(b"di-er-ci").await;
    let err = mallory
        .dial_secret(&alice_addr, Some(b"cuo-de"))
        .await
        .expect_err("对不上必须失败");
    assert!(
        err.to_string().contains("暗号"),
        "错误信息应提到暗号：{err}"
    );
    // 失败后两边都没多出对话
    assert!(mallory.roster().await.is_empty());
    // Alice 侧也应是 MeetFailed 而非 Met
    let failed = expect(&mut alice_ev, |ev| {
        if let Event::MeetFailed { error } = ev {
            Some(error.clone())
        } else {
            None
        }
    })
        .await;
    assert!(failed.contains("暗号"), "Alice 侧也应看到暗号失败：{failed}");
}

#[tokio::test]
async fn 暗号_经茶馆也照样对() {
    let teahouse = spawn_teahouse().await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // Alice 先进房（带暗号），Bob 后到（同一暗号）
    let alice_early = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_early.meet_at_teahouse_secret(&t, 77, Some(b"cha-guan-an-hao")).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    bob.meet_at_teahouse_secret(&teahouse, 77, Some(b"cha-guan-an-hao"))
        .await
        .expect("同暗号应谈成");

    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    bob.speak("墙内墙外一把锁").await.expect("Bob 说");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "墙内墙外一把锁"));
}

#[tokio::test]
async fn 暗号链_告别自动续上下一场() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;
    let (carol, _carol_ev, _c) = spawn_person("Carol").await;

    // 第一场：Bob 找 Alice，用第一把暗号（带外念的那次）
    alice.expect_secret(b"di-yi-ba").await;
    bob.dial_secret(&alice_addr, Some(b"di-yi-ba"))
        .await
        .expect("第一场谈成");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 道别：两边自动预约下一把
    let who = bob.bye().await.expect("道别");
    assert_eq!(who, "Alice");
    // Alice 侧看到链续上了
    let chained = expect(&mut alice_ev, |ev| {
        if let Event::SecretChained { with } = ev {
            Some(with.clone())
        } else {
            None
        }
    })
        .await;
    assert_eq!(chained, "Bob");
    // 两边都收摊
    expect(&mut alice_ev, left).await;
    expect(&mut bob_ev, left).await;

    // 第二场：Bob 再找 Alice——不带暗号，链上的下一把自动生效
    bob.dial(&alice_addr).await.expect("链上自动续场");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    bob.speak("链上第二场").await.expect("Bob 说");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "链上第二场"));

    // 第二场也道别：又预约下一把。此后 Alice 的切口重新竖起——
    // 没暗号的人此刻进不来（电话模型在切口生效期间收窄）
    bob.bye().await.expect("第二场道别");
    expect(&mut alice_ev, left).await;
    expect(&mut bob_ev, left).await;
    let err = carol.dial(&alice_addr).await;
    assert!(err.is_err(), "切口竖起期间，没暗号的人进不来：{err:?}");
}

/// 从事件里取出 Left（对方名字）。
fn left(ev: &Event) -> Option<String> {
    if let Event::Left { name, .. } = ev {
        Some(name.clone())
    } else {
        None
    }
}

#[tokio::test]
async fn 围坐一圈_三人成圈群聊() {
    let (alice, mut alice_ev, _a2) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (_carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;

    // 全连接三条线：Alice 拨 Bob、Alice 拨 Carol、Bob 拨 Carol。
    // 线是双向的，谁拨都算一条；每人手里两条，三人成网。
    alice.dial(&bob_addr).await.expect("A-B");
    alice.dial(&carol_addr).await.expect("A-C");
    bob.dial(&carol_addr).await.expect("B-C");
    // 每人见到两位
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, met).await;
        expect(ev, met).await;
    }

    // 围坐：Alice 发起凑话（自传染，三人各自凑话、各自铸出同一把钥匙）
    let members = alice.form_circle().await.expect("凑话");
    assert_eq!(members, 3);
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        let n = expect(ev, |e| {
            if let Event::CircleFormed { members } = e {
                Some(*members)
            } else {
                None
            }
        })
            .await;
        assert_eq!(n, 3, "人人收齐三人贡献");
    }

    // 屋里的话：Alice 一句，Bob、Carol 同时听见
    alice.circle_speak("今晚吃火锅").await.expect("群发");
    let (from_b, text_b) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    let (from_c, text_c) = expect(&mut carol_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from_b.as_str(), text_b.as_str()), ("Alice", "今晚吃火锅"));
    assert_eq!((from_c.as_str(), text_c.as_str()), ("Alice", "今晚吃火锅"));

    // Bob 也说一句，Alice 听见（来线署名 = Bob）
    bob.circle_speak("我带毛肚").await.expect("Bob 群发");
    let (from_a, text_a) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from_a.as_str(), text_a.as_str()), ("Bob", "我带毛肚"));

    // 圈外人 Mallory 即使偷到密文也读不懂：他没参与凑话。
    // （这里用「没围圈的人 circle_speak 直接失败」来验状态边界）
    let (mallory, _m_ev, _m) = spawn_person("Mallory").await;
    assert!(mallory.circle_speak("偷听").await.is_err());
}

#[tokio::test]
async fn 介绍人_打洞直连密谈() {
    let teahouse = spawn_teahouse().await;
    // 给介绍人（UDP 那半边）一点开张时间
    tokio::time::sleep(Duration::from_millis(400)).await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // Alice 先报名（当应答方），Bob 后到（当发起方）
    let alice_p = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_p.punch(&t, "huo-guo").await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    bob.punch(&teahouse, "huo-guo").await.expect("打洞后直连");

    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 双向说话——这条线没经过茶馆一个字节，介绍人只递过地址
    bob.speak("dong li de hua").await.expect("Bob 说");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "dong li de hua"));

    alice.speak("liang tou dou tong").await.expect("Alice 说");
    let (from, text) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Alice", "liang tou dou tong"));
}

#[tokio::test]
async fn 茶馆_两对同馆不同房互不串门() {
    let teahouse = spawn_teahouse().await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;
    let (carol, mut carol_ev, _c) = spawn_person("Carol").await;
    let (dave, mut dave_ev, _d) = spawn_person("Dave").await;

    // 两对同时进同一间茶馆，房号不同：房 10 与房 20
    let alice_early = alice.clone();
    let carol_early = carol.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_early.meet_at_teahouse(&t, 10).await;
    });
    let t2 = teahouse.clone();
    tokio::spawn(async move {
        let _ = carol_early.meet_at_teahouse(&t2, 20).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    bob.meet_at_teahouse(&teahouse, 10).await.expect("Bob 进房 10");
    dave.meet_at_teahouse(&teahouse, 20).await.expect("Dave 进房 20");

    let (a_met,) = expect(&mut alice_ev, |ev| met(ev).map(|(n, _, _)| (n,))).await;
    let (b_met,) = expect(&mut bob_ev, |ev| met(ev).map(|(n, _, _)| (n,))).await;
    let (c_met,) = expect(&mut carol_ev, |ev| met(ev).map(|(n, _, _)| (n,))).await;
    let (d_met,) = expect(&mut dave_ev, |ev| met(ev).map(|(n, _, _)| (n,))).await;

    // 各自只见到自己的同房人
    assert_eq!(a_met, "Bob");
    assert_eq!(b_met, "Alice");
    assert_eq!(c_met, "Dave");
    assert_eq!(d_met, "Carol");

    // Alice 说话只应被 Bob 听见（Dave 在另一间房，绝听不到）
    alice.speak("房十的悄悄话").await.expect("Alice 说");
    let (from, text) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Alice", "房十的悄悄话"));
    // Dave 侧不应出现这句话：等一小会儿，确认事件流里只有 Met 没有 Heard
    let mut dave_heard = false;
    let deadline = Instant::now() + Duration::from_millis(400);
    while let Ok(Some(ev)) = timeout(deadline.saturating_duration_since(Instant::now()), dave_ev.recv()).await {
        if matches!(ev, Event::Heard { .. }) {
            dave_heard = true;
        }
    }
    assert!(!dave_heard, "另一间房绝不该听到房十的话");
}

#[tokio::test]
async fn 切口_扫门的烧不掉() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // 扫门的来了：连上、丢半截帧、走人——切口不该被烧掉
    alice.expect_secret(b"lan-tong-qi").await;
    {
        let mut scanner = TcpStream::connect(&alice_addr).await.expect("扫门连上");
        scanner.write_all(&[0x03, 0xff, 0x41]).await.expect("丢半截帧");
        tokio::time::sleep(Duration::from_millis(100)).await;
    } // 此处断开
    expect(&mut alice_ev, |ev| {
        if let Event::MeetFailed { .. } = ev {
            Some(())
        } else {
            None
        }
    })
        .await;

    // 再来一个扫门的：连上就走（连半截帧都没有）——切口也不该被烧掉
    {
        let _scanner = TcpStream::connect(&alice_addr).await.expect("扫门连上");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    expect(&mut alice_ev, |ev| {
        if let Event::MeetFailed { .. } = ev {
            Some(())
        } else {
            None
        }
    })
        .await;

    // 真人来敲门：同一句暗号照样谈得成——切口没被扫描烧掉
    bob.dial_secret(&alice_addr, Some(b"lan-tong-qi"))
        .await
        .expect("切口还在，应谈成");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
}

#[tokio::test]
async fn 茶馆_哑巴连接堵不住门() {
    let teahouse = spawn_teahouse().await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // 一个连上却不报房号的哑巴连接，占着门厅不走
    let _mute = TcpStream::connect(&teahouse).await.expect("哑巴连上");

    // 另一对正常人照常进房密谈——门厅不该被一个哑巴堵死
    let alice_early = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_early.meet_at_teahouse(&t, 606).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    let met_ok = tokio::time::timeout(
        Duration::from_secs(5),
        bob.meet_at_teahouse(&teahouse, 606),
    )
        .await
        .expect("5 秒内应进得了房（门厅被堵死则这里超时）");
    met_ok.expect("Bob 进房");

    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    bob.speak("门厅是通的").await.expect("Bob 说");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "门厅是通的"));
}

#[tokio::test]
async fn 茶馆_线断不挂链_下场免暗号() {
    let teahouse = spawn_teahouse().await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    // 第一场：经茶馆、不带暗号
    let alice_early = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_early.meet_at_teahouse(&t, 707).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    bob.meet_at_teahouse(&teahouse, 707).await.expect("第一场");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 道别：茶馆线挂不住链（房间一次性，锚不存在）——
    // Alice 不该看到 SecretChained，切口也不该被竖起来堵下一场的门
    bob.bye().await.expect("道别");
    let mut saw_chain = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let ev = timeout(left, alice_ev.recv())
            .await
            .expect("事件流不应关闭")
            .expect("事件流不应枯竭");
        match ev {
            Event::SecretChained { .. } => saw_chain = true,
            Event::Left { .. } => break,
            _ => {}
        }
    }
    assert!(!saw_chain, "茶馆线不应预约暗号链");

    // 第二场：不带暗号再进一间房，应照样谈得成
    let alice_again = alice.clone();
    let t2 = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_again.meet_at_teahouse(&t2, 708).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    bob.meet_at_teahouse(&teahouse, 708).await.expect("第二场免暗号");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
}

#[tokio::test]
async fn 隐身_精确交往照常() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;

    // Bob 隐身出生：不应答「有人吗」，但 TCP 耳朵照常开
    let port = free_port();
    let bob_addr = format!("127.0.0.1:{port}");
    let (bob, mut bob_ev) =
        Person::born_hidden("Bob", &bob_addr).await.expect("隐身出生不应失败");

    // 隐身者主动精确找人：照常
    bob.dial(&alice_addr).await.expect("隐身者拨号照常");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    bob.speak("yin shen zhe ye neng shuo").await.expect("隐身者发言");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "yin shen zhe ye neng shuo"));

    // 反过来：拿着确切地址拨隐身者：照常（广播探不到 ≠ 够不着）
    alice.dial(&bob_addr).await.expect("精确拨隐身者照常");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    alice.speak("na zhe di zhi ye zhao de dao ni").await.expect("Alice 发言");
    let (from, text) = expect(&mut bob_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!(
        (from.as_str(), text.as_str()),
        ("Alice", "na zhe di zhi ye zhao de dao ni")
    );
}
