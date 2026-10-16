//! 集成测试：真实起人、真实握手、真实密谈——不断言内部，只断言经历。
//!
//! 每个测试都是同一台机器上的一场小型相遇。

use e2ee::{Event, LeaveReason, Person};
use e2ee::person::{SEATS_CLOSED, SEATS_OPEN};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{timeout, Instant};

/// 等到事件流里出现满足条件的事件（跳过无关事件），超时则失败。
/// 时限 15 秒：cargo test 多线程并行下多个 current_thread 运行时
/// 抢核，5 秒会偶发饿死（事件本身毫秒级，只有调度压力才会用满）。
async fn expect<T>(
    events: &mut UnboundedReceiver<Event>,
    cond: impl Fn(&Event) -> Option<T>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(15);
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
    // :0 = 系统分配（free_port 的先占后放在并发测试下有被抢窗口，
    // 且 born 本身就会绑定——直接交给内核最稳）
    let (p, e) = Person::born(name, "127.0.0.1:0").await.expect("出生不应失败");
    let addr = p.listen_addr().await;
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

#[tokio::test]
async fn 开放台_不续链_陌生来客不被锁() {
    // 台席：关闭续链（--no-chain 的库面）——对陌生人常开的接收方
    let (desk, mut desk_ev, desk_addr) = spawn_person("Desk").await;
    desk.set_chaining(false).await;

    // 用户1：直连、留言、道别（道别会试图预约下一把暗号）
    let (u1, mut u1_ev, _u1a) = spawn_person("User1").await;
    u1.dial(&desk_addr).await.expect("u1 拨通");
    expect(&mut desk_ev, met).await;
    expect(&mut u1_ev, met).await;
    u1.speak("fankui yi").await.expect("u1 留言");
    u1.bye().await.expect("u1 道别");
    expect(&mut desk_ev, left).await; // 台席看到收摊；其间不该有 SecretChained

    // 用户2：不带暗号的陌生来客——必须照样进得来（默认续链会把他锁在门外）
    let (u2, mut u2_ev, _u2a) = spawn_person("User2").await;
    u2.dial(&desk_addr).await.expect("陌生来客应照样拨通（台席没被上锁）");
    expect(&mut desk_ev, met).await;
    expect(&mut u2_ev, met).await;
    u2.speak("fankui er").await.expect("u2 留言");
    let (from, text) = expect(&mut desk_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("User2", "fankui er"));

    // 用户3 接着来：依然敞开（不止容纳一位）
    let (u3, _u3_ev, _u3a) = spawn_person("User3").await;
    u3.dial(&desk_addr).await.expect("台席应一直敞开");
}

#[tokio::test]
async fn 文件_无损往返_二进制字节一毫不差() {
    let (_alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    bob.dial(&alice_addr).await.expect("拨通");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 约 110KB：跨多个 32KB 块；掺进非法 UTF-8 与边角字节——
    // 文件走的是字节的车道，文字的安检（from_utf8_lossy）不该碰它一个字节
    let mut data = Vec::new();
    for i in 0..40_000u32 {
        data.push((i % 256) as u8);
        if i % 7 == 0 {
            data.push(0xFF);
        }
    }
    // 破 UTF-8 序列：过长 C0、代理区 ED A0 80、越界 F5 80 80、截断 E2 82
    data.extend_from_slice(&[0xC0, 0xAF, 0xED, 0xA0, 0x80, 0xF5, 0x80, 0x80, 0xE2, 0x82]);
    bob.send_file("bi-ji.dat", &data).await.expect("发文件");

    let (name, got) = expect(&mut alice_ev, |ev| {
        if let Event::FileArrived { name, data, .. } = ev {
            Some((name.clone(), data.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!(name, "bi-ji.dat");
    assert_eq!(got.len(), data.len(), "长度必须一致");
    assert_eq!(&got[..], &data[..], "字节必须一毫不差（无损，非法 UTF-8 原样到达）");

    // 文件之后聊天照常——两条车道互不干扰
    bob.speak("wen zi hai neng shuo").await.expect("聊天照常");
    let (from, text) = expect(&mut alice_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("Bob", "wen zi hai neng shuo"));
}

#[tokio::test]
async fn 文件_超上限当场拒绝() {
    let (_alice, _alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, _bob_ev, _b) = spawn_person("Bob").await;
    bob.dial(&alice_addr).await.expect("拨通");

    let huge = vec![0u8; e2ee::person::MAX_FILE_BYTES + 1];
    let err = bob.send_file("ju-wen-jian.bin", &huge).await.expect_err("超限必须拒绝");
    assert!(err.to_string().contains("32MB"), "错误应说明上限：{err}");

    // 空名同样拒绝
    let err = bob.send_file("  ", b"x").await.expect_err("空名必须拒绝");
    assert!(err.to_string().contains("文件名"), "错误应指向文件名：{err}");
}

#[tokio::test]
async fn 文件_打洞线上也照传() {
    let teahouse = spawn_teahouse().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    let alice_p = alice.clone();
    let t = teahouse.clone();
    tokio::spawn(async move {
        let _ = alice_p.punch(&t, "wen-jian").await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    bob.punch(&teahouse, "wen-jian").await.expect("打洞");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 40KB = 两块：验证 32KB 的块装得进 UDP 数据报（这是块大小的设计依据）
    let data: Vec<u8> = (0..40_000u32).map(|i| (i.wrapping_mul(31) % 256) as u8).collect();
    bob.send_file("tu-pian.bin", &data).await.expect("打洞线上发文件");
    let got = expect(&mut alice_ev, |ev| {
        if let Event::FileArrived { data, .. } = ev {
            Some(data.clone())
        } else {
            None
        }
    })
        .await;
    assert_eq!(&got[..], &data[..], "打洞线上字节同样一毫不差");
}

#[tokio::test]
async fn 常备门禁_瞎拨锁不住_真钥照常进() {
    let (desk, mut desk_ev, desk_addr) = spawn_person("Desk").await;
    desk.expect_secret_stable(b"ji-qi-yao-0123456789abcdef0123456789abcdef")
        .await;

    // 恶意者轮番瞎拨：错钥、再错钥、不带钥——每一次都响亮失败，
    // 但门禁从不被烧掉（一次性切口扛不住这种消耗，常备扛得住）
    let (m1, _m1_ev, _a) = spawn_person("Mallory1").await;
    assert!(m1.dial_secret(&desk_addr, Some(b"cuo-de")).await.is_err());
    expect(&mut desk_ev, |ev| matches!(ev, Event::MeetFailed { .. }).then_some(())).await;

    let (m2, _m2_ev, _b) = spawn_person("Mallory2").await;
    assert!(m2.dial_secret(&desk_addr, Some(b"hai-shi-cuo")).await.is_err());
    expect(&mut desk_ev, |ev| matches!(ev, Event::MeetFailed { .. }).then_some(())).await;

    let (m3, _m3_ev, _c) = spawn_person("Mallory3").await;
    assert!(m3.dial(&desk_addr).await.is_err(), "不带钥在武装台席必须被拒");
    expect(&mut desk_ev, |ev| matches!(ev, Event::MeetFailed { .. }).then_some(())).await;

    // 真用户带对钥：三番瞎拨之后照样进得来，密谈照常
    let (user, mut user_ev, _u) = spawn_person("User").await;
    user.dial_secret(&desk_addr, Some(b"ji-qi-yao-0123456789abcdef0123456789abcdef"))
        .await
        .expect("真钥必须进得来——瞎拨锁不住台");
    expect(&mut desk_ev, met).await;
    expect(&mut user_ev, met).await;
    user.speak("fankui").await.expect("真用户发言");
    let (from, text) = expect(&mut desk_ev, |ev| {
        if let Event::Heard { name, text, .. } = ev {
            Some((name.clone(), text.clone()))
        } else {
            None
        }
    })
        .await;
    assert_eq!((from.as_str(), text.as_str()), ("User", "fankui"));

    // 成功之后门禁仍在：再瞎拨一次照样被拒（常备从不消耗）
    let (m4, _m4_ev, _d) = spawn_person("Mallory4").await;
    assert!(m4.dial_secret(&desk_addr, Some(b"you-cuo")).await.is_err());
}

#[tokio::test]
async fn 聊天带垫_长短行照常说听() {
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, _b) = spawn_person("Bob").await;

    bob.dial(&alice_addr).await.expect("拨通");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;

    // 长短悬殊的几行都该原样到达：垫由收方丢弃，正文一个字不少、
    // 也不该多出前缀或垫的字节（帧长与话长的关联就此被垫糊掉）
    let long_line = "zhe ju hua te bie chang".repeat(60);
    for line in ["a", "zhong deng chang du de yi ju hua", long_line.as_str()] {
        bob.speak(line).await.expect("说");
        let (from, got) = expect(&mut alice_ev, |ev| {
            if let Event::Heard { name, text, .. } = ev {
                Some((name.clone(), text.clone()))
            } else {
                None
            }
        })
            .await;
        assert_eq!((from.as_str(), got.as_str()), ("Bob", line));
    }

    // 反方向同样
    alice.speak("hui hua").await.expect("回话");
    let got = expect(&mut bob_ev, |ev| {
        if let Event::Heard { text, .. } = ev {
            Some(text.clone())
        } else {
            None
        }
    })
        .await;
    assert_eq!(got, "hui hua");
}

// ── v3 名单即圈测试矩阵（P3 slice-1/2 + 判例库集成面）──────────

/// 从事件里取 CircleRekeyed。
fn rekeyed(ev: &Event) -> Option<(usize, String)> {
    if let Event::CircleRekeyed { members, change } = ev {
        Some((*members, change.clone()))
    } else {
        None
    }
}

/// 从事件里取 CircleFormed 的人数。
fn formed(ev: &Event) -> Option<usize> {
    if let Event::CircleFormed { members } = ev {
        Some(*members)
    } else {
        None
    }
}

/// 从事件里取 Heard 的（说话人, 文本）。
fn heard(ev: &Event) -> Option<(String, String)> {
    if let Event::Heard { name, text, .. } = ev {
        Some((name.clone(), text.clone()))
    } else {
        None
    }
}

/// 三人全连接开台成圈。名单=[Alice,Bob,Carol]，返回三套人 + 事件流。
async fn three_mesh() -> (
    Person,
    UnboundedReceiver<Event>,
    Person,
    UnboundedReceiver<Event>,
    Person,
    UnboundedReceiver<Event>,
) {
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, b_addr) = spawn_person("Bob").await;
    let (carol, mut carol_ev, c_addr) = spawn_person("Carol").await;
    alice.dial(&b_addr).await.expect("A-B");
    alice.dial(&c_addr).await.expect("A-C");
    bob.dial(&c_addr).await.expect("B-C");
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, met).await;
        expect(ev, met).await;
    }
    // Alice 开台：邀请名单 = [Alice, Bob, Carol]
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 3, "名单=发起者+两条线");
    (alice, alice_ev, bob, bob_ev, carol, carol_ev)
}

#[tokio::test]
async fn 围坐一圈_三人成圈群聊() {
    // 全连接：名单到齐 → 铸钥 → 群聊双向可达。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, mut carol_ev) = three_mesh().await;

    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3, "名单上的名字全部自署到场即铸钥");
    }

    // Alice 一句，Bob、Carol 同时听见（子钥解密 + 验签双闸）
    alice.circle_speak("今晚吃火锅").await.expect("群发");
    for ev in [&mut bob_ev, &mut carol_ev] {
        let got = expect(ev, heard).await;
        assert_eq!((got.0.as_str(), got.1.as_str()), ("Alice", "今晚吃火锅"));
    }

    // Bob 回话，Alice 听见
    bob.circle_speak("我带毛肚").await.expect("Bob 群发");
    let got = expect(&mut alice_ev, heard).await;
    assert_eq!((got.0.as_str(), got.1.as_str()), ("Bob", "我带毛肚"));

    // 圈外人 Mallory 没围圈：不能说（状态边界）
    let (mallory, _m_ev, _m) = spawn_person("Mallory").await;
    assert!(mallory.circle_speak("偷听").await.is_err());
}

#[tokio::test]
async fn 名单圈_星型拓扑_成员导出子图连通() {
    // 星型：hub=Bob，Alice/Carol 只与 Bob 相连。信念路由下
    // 泛洪经 hub 转达——成员导出子图连通即可成圈。
    let (alice, mut alice_ev, _a2) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (carol, mut carol_ev, _c2) = spawn_person("Carol").await;

    alice.dial(&bob_addr).await.expect("A-B");
    carol.dial(&bob_addr).await.expect("C-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;

    let n = bob.form_circle().await.expect("枢纽发起");
    assert_eq!(n, 3, "名单=[Bob,Alice,Carol]");
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3, "泛洪下名单名字到齐即成");
    }

    // 星型群聊：Alice 的话经 Bob 转达 Carol（信念路由转发）
    alice.circle_speak("xing xing quan").await.expect("说");
    for ev in [&mut bob_ev, &mut carol_ev] {
        let got = expect(ev, heard).await;
        assert_eq!(got.0, "Alice");
        assert_eq!(got.1, "xing xing quan");
    }
}

#[tokio::test]
async fn 一人成圈_开台等入席流() {
    // 一人成圈默认开台（舞台）：立即成钥（K=HKDF([r])）。
    // 新人经入席流（slice-4）落座——本版先钉「开台存在且不误吸收」。
    let (alice, mut alice_ev, _a2) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;

    let n = alice.form_circle().await.expect("一人也能成圈");
    assert_eq!(n, 1, "一人圈=开台");
    let m = expect(&mut alice_ev, formed).await;
    assert_eq!(m, 1, "名单只有我，立即铸钥");

    // Bob 拨入——v3 圈不自动吸收新线（入座必须走入席流）
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    assert!(bob.circle_speak("hai bu zai quan li").await.is_err(), "不在名单上不能说");
}

#[tokio::test]
async fn 名单未到齐_不开讲_名单外不吸收() {
    // P-03 的完成语义：凑话完成由名单说了算——名单外的人后到，
    // 信念路由下进不来。名单内的人没到齐，永不铸钥。
    let (alice, mut alice_ev, _a2) = spawn_person("Alice").await;
    let (_bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (carol, mut carol_ev, _c) = spawn_person("Carol").await;

    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    // 此时 Carol 尚未连入——名单=[Alice, Bob]
    alice.form_circle().await.expect("开台");
    // A/B 成圈（2 人名单到齐）
    let ma = expect(&mut alice_ev, formed).await;
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!((ma, mb), (2, 2));

    // Carol 事后拨入 Bob——她不在名单上：信念路由下零圈帧到达她
    carol.dial(&bob_addr).await.expect("C-B");
    expect(&mut bob_ev, met).await;
    assert!(carol.circle_speak("wo ye yao").await.is_err(), "名单外不能说");

    // A 说话：B 听见；Carol 侧 2 秒内不得出现任何圈内事件
    alice.circle_speak("ming dan de ren cai neng shuo").await.expect("说");
    let got = expect(&mut bob_ev, heard).await;
    assert_eq!(got.1, "ming dan de ren cai neng shuo");
    let mut leaked = false;
    let deadline = Instant::now() + Duration::from_millis(2000);
    while let Ok(Some(ev)) =
        timeout(deadline.saturating_duration_since(Instant::now()), carol_ev.recv()).await
    {
        if matches!(
            ev,
            Event::CircleFormed { .. } | Event::CircleMumble { .. } | Event::CircleRekeyed { .. }
        ) {
            leaked = true;
        }
    }
    assert!(!leaked, "信念路由（P-02）：圈外线收到零圈帧");
}

#[tokio::test]
async fn 署名告别_双见证人收敛() {
    // P-06 的活体版：全连接 A-B-C，Carol 署名告退——她的两条线的
    // 对端（Alice、Bob）都是一跳直接接收者=见证人，各自发 RM
    //（删同一人+各旋转各自的行，T2 不相交）→ 双向收敛。
    let (alice, mut alice_ev, bob, mut bob_ev, carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }

    // Carol 临走说一句（在场辩护的豁免场景：说完晚安就告别）
    carol.circle_speak("wan an").await.expect("临别赠言");
    expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;

    // 署名告退：0x0C 泛洪出去，Alice/Bob 各自见证、各自补话
    carol.leave().await;

    // 每位幸存者应用两份 RM（自己的 + 对端见证人的）→ 两次换钥
    for ev in [&mut alice_ev, &mut bob_ev] {
        let r1 = expect(ev, rekeyed).await;
        assert_eq!(r1.0, 2, "Carol 离席后剩两人");
        assert!(r1.1.contains("Carol"), "变更点名离场者：{}", r1.1);
        let r2 = expect(ev, rekeyed).await;
        assert_eq!(r2.0, 2, "第二份 RM 收敛后仍是两人：{}", r2.1);
    }

    // 收敛后双向群聊（两份 RM 应用顺序无关，终态同钥——交换代数）
    alice.circle_speak("sheng cun zhe dui hua").await.expect("幸存者 A 说");
    let got = expect(&mut bob_ev, heard).await;
    assert_eq!(got.1, "sheng cun zhe dui hua");
    bob.circle_speak("sheng cun zhe hui hua").await.expect("幸存者 B 说");
    let got = expect(&mut alice_ev, heard).await;
    assert_eq!(got.1, "sheng cun zhe hui hua");
}

#[tokio::test]
async fn 点名_重跑circle_修复一切() {
    // A4：点名=新圈号重新聚首，与创世同构——唯一的真理修复路径。
    // 离场后的残局（僵尸、旧钥）用一次 /circle 全部抚平。
    let (alice, mut alice_ev, _bob, mut bob_ev, carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }

    // Carol 署名告退（残局：幸存者经历换钥窗口）
    carol.leave().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, rekeyed).await;
        expect(ev, rekeyed).await;
    }

    // 点名：Alice 重跑 /circle——此时她的会话只剩 Bob
    let n = alice.form_circle().await.expect("点名");
    assert_eq!(n, 2, "点名名单=当前在座者 [Alice, Bob]");
    let ma = expect(&mut alice_ev, formed).await;
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!((ma, mb), (2, 2), "新圈号重新聚首成功");

    // 新圈照常群聊（社会连续、密码学新鲜）
    alice.circle_speak("dian ming hou zhao chang shuo").await.expect("说");
    let got = expect(&mut bob_ev, heard).await;
    assert_eq!(got.1, "dian ming hou zhao chang shuo");
}

#[tokio::test]
async fn 圈内署名_防冒充边界() {
    // 圈外人无法冒充圈内人说话：没有名单行里的嗓音钥就签不出
    // 能过双闸的帧（伪造层面的验证在单元测试；此处钉状态边界）。
    let (_alice, mut alice_ev, bob, mut bob_ev, _carol, _carol_ev) = three_mesh().await;
    expect(&mut alice_ev, formed).await;
    expect(&mut bob_ev, formed).await;
    let (mallory, _m_ev, _m2) = spawn_person("Mallory").await;

    assert!(mallory.circle_speak("jia mao").await.is_err());
    bob.circle_speak("zhen de").await.expect("圈内能说");
    let got = expect(&mut alice_ev, heard).await;
    assert_eq!((got.0.as_str(), got.1.as_str()), ("Bob", "zhen de"));
}

// ── v3 slice-4 入席流测试（0x10 → 0x11 → ADD → 0x12）──────────────

/// 从事件里取 SeatOffered（引荐人名）。
fn seat_offered(ev: &Event) -> Option<String> {
    if let Event::SeatOffered { from, .. } = ev {
        Some(from.clone())
    } else {
        None
    }
}

/// 从事件里取 CircleRejected 的 detail。
fn rejected(ev: &Event) -> Option<String> {
    if let Event::CircleRejected { detail } = ev {
        Some(detail.clone())
    } else {
        None
    }
}

/// 从事件里取 CircleConflict 的 detail。
fn conflict(ev: &Event) -> Option<String> {
    if let Event::CircleConflict { detail } = ev {
        Some(detail.clone())
    } else {
        None
    }
}

/// 断言事件流在窗口期内**没有**指定类别的圈事件（安静窗口）。
async fn assert_quiet(
    events: &mut UnboundedReceiver<Event>,
    ms: u64,
    pat: impl Fn(&Event) -> bool + Send,
) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while let Ok(Some(ev)) =
        timeout(deadline.saturating_duration_since(Instant::now()), events.recv()).await
    {
        assert!(!pat(&ev), "不该出现的事件：{ev:?}");
    }
}

/// 起一个开台双人圈（A-B 互联，A 开台）。返回两套人+事件流。
async fn two_open_circle() -> (
    Person,
    UnboundedReceiver<Event>,
    Person,
    UnboundedReceiver<Event>,
) {
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    alice
        .form_circle_seats(Some(SEATS_OPEN))
        .await
        .expect("开台");
    let ma = expect(&mut alice_ev, formed).await;
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!((ma, mb), (2, 2));
    (alice, alice_ev, bob, bob_ev)
}

#[tokio::test]
async fn 入席流_完整走位_新人解密验签() {
    // P-07：0x10 看座 → 0x11 落座署名 → ADD 泛洪 → 0x12 座次表——
    // 新人拿到全表持钥入圈，解密 + 验签全通（三端互说）。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;

    // 新人拨向引荐人 Bob（Bob 聚焦 Carol 后 /bring）
    bob.dial(&carol_addr).await.expect("B-C 线");
    // 等两条 Met 都到齐（Bob 主动 → Bob 聚焦 Carol；Carol 侧也 Met）
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;

    // Bob 领人（无拟名 = 无门直接自动）
    let seats = bob.bring(None).await.expect("看座");
    assert_eq!(seats, SEATS_OPEN);
    // Carol 收到看座事件
    let from = expect(&mut carol_ev, seat_offered).await;
    assert_eq!(from, "Bob");
    // Carol 落座（默认自报名）
    carol.sit(None).await.expect("落座");

    // Bob 应用 ADD（换钥事件），Carol 收 0x12 建圈（CircleFormed 3 人）
    let rk = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rk.0, 3, "三人桌");
    assert!(rk.1.contains("Carol"), "变更点名新人：{}", rk.1);
    let mc = expect(&mut carol_ev, formed).await;
    assert_eq!(mc, 3);
    // Alice 收到转发的 ADD 也换钥
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);

    // 三端互说（新人解密+验签；老人经信念路由转发收到新人的话）
    carol.circle_speak("xin ren bao dao").await.expect("新人说");
    let got_a = expect(&mut alice_ev, heard).await;
    assert_eq!((got_a.0.as_str(), got_a.1.as_str()), ("Carol", "xin ren bao dao"));
    let got_b = expect(&mut bob_ev, heard).await;
    assert_eq!((got_b.0.as_str(), got_b.1.as_str()), ("Carol", "xin ren bao dao"));

    alice.circle_speak("lao ren hui hua").await.expect("老人说");
    let got_c = expect(&mut carol_ev, heard).await;
    assert_eq!((got_c.0.as_str(), got_c.1.as_str()), ("Alice", "lao ren hui hua"));
}

#[tokio::test]
async fn 入席流_期望门_拟名不符响亮暂停_重试完成() {
    // 期望门：/bring 拟名 与 0x11 自署名不匹配 → CircleConflict +
    // 回执丢弃 + 待座保留；重跑 /bring（不带拟名）+ 再 /sit → 完成。
    let (_alice, _alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;

    bob.dial(&carol_addr).await.expect("B-C 线");
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;

    // Bob 期望带来的是 Alice——但 Carol 自报名是 Carol
    bob.bring(Some("Alice")).await.expect("看座");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(None).await.expect("落座");

    // Bob：响亮冲突（拟名不符），不是静默
    let detail = expect(&mut bob_ev, conflict).await;
    assert!(detail.contains("Alice"), "冲突应点名拟名：{detail}");
    // Carol：没有 CircleFormed（回执被丢弃——她还在待座状态）

    // 重跑：不带拟名 → 无门 → 完成
    bob.bring(None).await.expect("重跑看座");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(None).await.expect("再落座");
    let rk = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rk.0, 3, "重试后三人桌");
    let mc = expect(&mut carol_ev, formed).await;
    assert_eq!(mc, 3);
}

#[tokio::test]
async fn 入席流_覆盖落座名_别名路由照常() {
    // /sit 覆盖落座名（「这桌叫我阿花」）：名单行名 ≠ 0x05 自报名，
    // 引荐人靠线别名把圈帧送达这条线；新人的署名按覆盖名归因。
    let (_alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (carol, mut carol_ev, carol_addr) = spawn_person("花小妹").await;

    bob.dial(&carol_addr).await.expect("B-C 线");
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;

    bob.bring(None).await.expect("看座");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(Some("阿花")).await.expect("落座为阿花");

    expect(&mut bob_ev, rekeyed).await;
    let mc = expect(&mut carol_ev, formed).await;
    assert_eq!(mc, 3);

    // 阿花说话：全员听见，归因到覆盖名
    carol.circle_speak("zhe zhuo jiao wo a hua").await.expect("说");
    let got_b = expect(&mut bob_ev, heard).await;
    assert_eq!((got_b.0.as_str(), got_b.1.as_str()), ("阿花", "zhe zhuo jiao wo a hua"));
    let got_a = expect(&mut alice_ev, heard).await;
    assert_eq!(got_a.0, "阿花");

    // 老人说 → 阿花经别名线收到
    bob.circle_speak("hua xiao mei ting de dao ma").await.expect("回话");
    let got_c = expect(&mut carol_ev, heard).await;
    assert_eq!((got_c.0.as_str(), got_c.1.as_str()), ("Bob", "hua xiao mei ting de dao ma"));
}

/// 三人封桌圈（全连接，默认封桌）。
async fn three_closed_circle() -> (
    Person,
    UnboundedReceiver<Event>,
    Person,
    UnboundedReceiver<Event>,
    Person,
    UnboundedReceiver<Event>,
) {
    let (alice, mut alice_ev, bob, mut bob_ev, carol, mut carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3);
    }
    (alice, alice_ev, bob, bob_ev, carol, carol_ev)
}

#[tokio::test]
async fn 封桌bring_引荐人分叉_双向mumble_点名重聚() {
    // 5214bb6 联立解：封桌 /bring —— 引荐人照常应用（发送原子），
    // 其余成员响亮拒收（席位契约）→ fork → 双向 mumble → 点名裁决。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, mut carol_ev) =
        three_closed_circle().await;
    let (newbie, mut newbie_ev, newbie_addr) = spawn_person("Newbie").await;

    // 新人拨向引荐人 Bob
    bob.dial(&newbie_addr).await.expect("B-N 线");
    expect(&mut bob_ev, met).await;
    expect(&mut newbie_ev, met).await;

    // Bob 封桌 bring（客户端警告但照发——库面返回 seats）
    let seats = bob.bring(None).await.expect("看座照发");
    assert_eq!(seats, SEATS_CLOSED);
    expect(&mut newbie_ev, seat_offered).await;
    newbie.sit(None).await.expect("落座");

    // Bob 应用 ADD（fork 成 4 人桌）；Newbie 拿 0x12 建圈
    let rk = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rk.0, 4);
    let mn = expect(&mut newbie_ev, formed).await;
    assert_eq!(mn, 4);

    // Alice、Carol：响亮拒收（席位契约），不换钥
    let da = expect(&mut alice_ev, rejected).await;
    assert!(da.contains("封桌"), "拒收应说明席位契约：{da}");
    let dc = expect(&mut carol_ev, rejected).await;
    assert!(dc.contains("封桌"));

    // 双向 mumble：Bob 说话 → Alice 解不开（响亮分歧）
    bob.circle_speak("fen cha hou de hua").await.expect("分叉侧说");
    let _ma = expect(&mut alice_ev, |ev| mumble(ev)).await;
    // Alice 说话 → Bob 解不开（反方向 mumble）
    alice.circle_speak("jiu yao ce de hua").await.expect("旧钥侧说");
    let _mb = expect(&mut bob_ev, |ev| mumble(ev)).await;
    // Alice↔Carol 照常（他们的信念没变；filter 防自回声）
    let got_c = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got_c.1, "jiu yao ce de hua");

    // 点名重聚（留线）：Alice 重跑 /circle——她的会话是 Bob、Carol
    // （Newbie 没有到她的线）→ 新圈 3 人；Bob 收 0x0F 放下分叉
    // （Newbie 被甩在旧桌——她对圈的唯一连接是 Bob 的线）
    alice.form_circle().await.expect("点名");
    let ma2 = expect(&mut alice_ev, formed).await;
    assert_eq!(ma2, 3);
    let mb2 = expect(&mut bob_ev, formed).await;
    assert_eq!(mb2, 3);
    let mc2 = expect(&mut carol_ev, formed).await;
    assert_eq!(mc2, 3);
    // 新桌说话全通
    bob.circle_speak("dian ming hou quan tong").await.expect("说");
    let got_a = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got_a.1, "dian ming hou quan tong");
    let got_c2 = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got_c2.1, "dian ming hou quan tong");
    // Newbie 的裁决：不出现 CircleFormed（她没被带进新圈）。
    // 注意 mumble/heard 可能合法出现：点名前的 fork 期，Bob 按信念
    // 路由把旧圈帧转发给她（transitional window——她听见密文尾巴），
    // 这是泛洪义务的正确行为，不是泄漏。
    let deadline = Instant::now() + Duration::from_millis(1500);
    while let Ok(Some(ev)) =
        timeout(deadline.saturating_duration_since(Instant::now()), newbie_ev.recv()).await
    {
        assert!(
            !matches!(ev, Event::CircleFormed { .. }),
            "Newbie 不该被带进新圈：{ev:?}"
        );
        assert!(
            !matches!(
                ev,
                Event::Heard { ref text, .. } if text == "dian ming hou quan tong"
            ),
            "新圈的话不该到达 Newbie：{ev:?}"
        );
    }
}

/// 从事件里取 CircleMumble。
fn mumble(ev: &Event) -> Option<String> {
    if let Event::CircleMumble { from } = ev {
        Some(from.clone())
    } else {
        None
    }
}

#[tokio::test]
async fn 封桌重复bring_只在引荐人自己的分叉上() {
    // 封桌 DoS 面（5214bb6）：第一次 fork 后，后续 /bring 都作用于
    // 引荐人自己的分叉信念；其余成员的对话完全不受影响（除了响亮事件）。
    let (alice, mut alice_ev, bob, mut bob_ev, carol, mut carol_ev) =
        three_closed_circle().await;
    let (n1, mut n1_ev, n1_addr) = spawn_person("N1").await;
    let (n2, mut n2_ev, n2_addr) = spawn_person("N2").await;

    bob.dial(&n1_addr).await.expect("B-N1");
    expect(&mut bob_ev, met).await;
    expect(&mut n1_ev, met).await;
    bob.bring(None).await.expect("第一次看座");
    expect(&mut n1_ev, seat_offered).await;
    n1.sit(None).await.expect("N1 落座");
    expect(&mut bob_ev, rekeyed).await; // Bob 分叉成 4 人
    expect(&mut n1_ev, formed).await;
    let d1 = expect(&mut alice_ev, rejected).await; // 其余成员第一次响亮拒收
    assert!(d1.contains("封桌"));
    expect(&mut carol_ev, rejected).await;

    // 第二次 bring：作用于 Bob 自己的分叉信念（加 N2）
    bob.dial(&n2_addr).await.expect("B-N2");
    expect(&mut bob_ev, met).await;
    expect(&mut n2_ev, met).await;
    bob.bring(None).await.expect("第二次看座");
    expect(&mut n2_ev, seat_offered).await;
    n2.sit(None).await.expect("N2 落座");
    expect(&mut bob_ev, rekeyed).await; // Bob 分叉成 5 人
    expect(&mut n2_ev, formed).await;
    expect(&mut alice_ev, rejected).await; // 第二次响亮拒收（事件照响）
    expect(&mut carol_ev, rejected).await;

    // Alice↔Carol 的对话完全不受影响（同钥照说照听）。
    // （filter 对端名：泛洪回声下自己说过的话可能绕回自己——
    // 自回声 heard 是设计行为，别把它误当对端的话）
    alice.circle_speak("wo men mei shou ying xiang").await.expect("说");
    let got = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "wo men mei shou ying xiang");
    carol.circle_speak("wo ye shi").await.expect("回话");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo ye shi");

    // 引荐人的分叉桌照常工作：Bob 与 N2 互说（N1 因二次旋转被
    // T2 相交变化甩出——她的钥停在 r₁；这是点名恢复的领域）
    bob.circle_speak("fen cha zhuo shuo hua").await.expect("Bob 说");
    let got = expect(&mut n2_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "fen cha zhuo shuo hua");
    // N1 侧 mumble（Bob 的新钥她解不开）——响亮、可恢复
    let _m = expect(&mut n1_ev, |ev| mumble(ev)).await;
}

#[tokio::test]
async fn 并发双ADD_同引荐人_有序投递下全员收敛() {
    // 同引荐人先后带入两个新人：Bob 的行旋转两次（r0→r1→r2），
    // 但 ADD 经单源有序投递（TCP）全员按同序应用——包括 N1 自己
    // 也收到 Bob 转发的 ADD(N2) 并跟上 r_b2（开台：引荐即背书）。
    // 终态全员同钥，先入座者不被甩出（T2：链式交换按序收敛）。
    // 真正的相交 fork 需要 ADD 乱序到达（多路径）——那属于 slice-6
    // rogue 注入测试的领域。
    let (_alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (n1, mut n1_ev, n1_addr) = spawn_person("N1").await;
    let (n2, mut n2_ev, n2_addr) = spawn_person("N2").await;

    bob.dial(&n1_addr).await.expect("B-N1");
    expect(&mut bob_ev, met).await;
    expect(&mut n1_ev, met).await;
    bob.bring(None).await.expect("带 N1");
    expect(&mut n1_ev, seat_offered).await;
    n1.sit(None).await.expect("N1 落座");
    expect(&mut bob_ev, rekeyed).await;
    expect(&mut n1_ev, formed).await;

    bob.dial(&n2_addr).await.expect("B-N2");
    expect(&mut bob_ev, met).await;
    expect(&mut n2_ev, met).await;
    bob.bring(None).await.expect("带 N2");
    expect(&mut n2_ev, seat_offered).await;
    n2.sit(None).await.expect("N2 落座");
    expect(&mut bob_ev, rekeyed).await;
    expect(&mut n2_ev, formed).await;

    // N1 也应用了转发的 ADD(N2)（换钥跟上 Bob 的第二次旋转）
    let _ = expect(&mut n1_ev, rekeyed).await;
    // Alice 同样跟上
    let _ = expect(&mut alice_ev, rekeyed).await;
    let _ = expect(&mut alice_ev, rekeyed).await;

    // 全员同钥：N1 说 → 全员听见（无 mumble、无点名必要；filter 防自回声）
    n1.circle_speak("da jia dou ting de dao").await.expect("N1 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "N1")).await;
    assert_eq!(got.1, "da jia dou ting de dao");
    let got2 = expect(&mut n2_ev, |e| heard(e).filter(|(n, _)| n == "N1")).await;
    assert_eq!(got2.1, "da jia dou ting de dao");
    let got3 = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "N1")).await;
    assert_eq!(got3.1, "da jia dou ting de dao");

    // 点名照常可用（A4：不是修复，是重申）——4 人重聚
    bob.form_circle().await.expect("点名");
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!(mb, 4);
    let m1 = expect(&mut n1_ev, formed).await;
    assert_eq!(m1, 4);
    let m2 = expect(&mut n2_ev, formed).await;
    assert_eq!(m2, 4);
    n1.circle_speak("dian ming hou zhao chang").await.expect("N1 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "N1")).await;
    assert_eq!(got.1, "dian ming hou zhao chang");
}

#[tokio::test]
async fn 拒入者_零圈帧_骨架无r() {
    // P-09：拒绝入座的候选人只持骨架（名字+公钥，无 r）——信念路由
    // 下连圈帧密文都不会到达她的线（引荐人的信念没有她）。
    let (_alice, _alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (_stranger, mut stranger_ev, stranger_addr) = spawn_person("Stranger").await;

    bob.dial(&stranger_addr).await.expect("B-S 线");
    expect(&mut bob_ev, met).await;
    expect(&mut stranger_ev, met).await;
    bob.bring(None).await.expect("看座");
    expect(&mut stranger_ev, seat_offered).await;
    // Straner 不 /sit——拒绝入座

    // 圈内说话：Stranger 一条圈帧也收不到（骨架无 r + 零密文）
    bob.circle_speak("quan li de hua").await.expect("说");
    assert_quiet(&mut stranger_ev, 1500, |ev| {
        matches!(
            ev,
            Event::CircleFormed { .. }
                | Event::CircleMumble { .. }
                | Event::CircleRekeyed { .. }
                | Event::Heard { .. }
        )
    })
    .await;
}
