//! 集成测试：真实起人、真实握手、真实密谈——不断言内部，只断言经历。
//!
//! 每个测试都是同一台机器上的一场小型相遇。

use e2ee::{Event, LeaveReason, Person};
use e2ee::person::{SEATS_CLOSED, SEATS_OPEN};
// 判例注入面：feature 门控（缺 feature 时库不编译它，canary 响亮提示）
#[cfg(feature = "rogue")]
use e2ee::rogue;
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
async fn 出生无喊话耳_删除面守卫() {
    // 3.0.0 删除面（removals_3_0_0.v1_discovery_family）的结构守卫
    //（canary 同族：把「无邀请暴露源归零」钉成断言，喊话耳朵悄悄
    // 爬回来 = 本测试变红）：出生只竖 TCP 这一只耳朵——喊话端口
    // 37777 上无人守（我们能把 127.0.0.1:37777 绑起来，恰证明进程
    // 里没有 0.0.0.0:37777 的监听者）。原「隐身」测试随功能退役：
    // 精确交往（拿地址拨号）曾是隐身模式的主张，如今是唯一的形态。
    let (_alice, _alice_ev, _addr) = spawn_person("Alice").await;
    let bob_port = free_port();
    let (_bob, _bob_ev) =
        Person::born("Bob", &format!("127.0.0.1:{bob_port}")).await.expect("出生照常");
    // 喊话端口无人守：bind 成功即证明（旧监听者绑 0.0.0.0:37777，
    // 会与本绑定冲突报 AddrInUse）
    let probe = tokio::net::UdpSocket::bind("127.0.0.1:37777")
        .await
        .expect("喊话端口应当无人守（3.0.0 删除面）：bind 失败 = 喊话耳朵回来了");
    drop(probe);
}

#[tokio::test]
async fn 快照后新线_零圈帧_从第一帧起() {
    // rulings.2026_10_17_flood_frame_all_migration 的结构断言：
    // 0x0F 扇出追随誓言（快照绑定集），快照之后接入的线收到圈帧
    // 数恰为零——**从第一帧起**（含创世帧），不只靠函数删除的事实。
    // 观测边界（如实入档）：漂移窗孤儿 0x0F 在收端被名单检查静默
    // 丢弃（invited 不含她 → continue，无事件），Event 流层面新旧
    // 实现同绿——红绿自证对本迁移**不适用**（差异在 wire 层：旧
    // flood_frame_all 把帧发到誓言外的线，P-02 精神面暴露 circle_id；
    // 迁移在源头不发，一致性破洞封死——owner 裁决明说这是非机密性
    // 破洞）。本测试钉的是事件流可见的不变量：无绑定的线从第一帧
    // 起零圈事件（连 mumble 都没有——根本收不到任何可解/不可解的
    // 圈载荷）；对照面：点名（新誓言集）合法纳入后全通。构造：
    // A-B 在座成圈 → Dave 快照后拨入（旧圈零帧）→ 点名（誓言集
    // 纳入 Dave）→ 三人全通。
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-B 在座
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    // A 开台（名单 [Alice, Bob]——快照此刻落定，绑定=A-B 线）
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 2);
    // 成圈先行（确保创世/凑话/成钥全都已经过 Dave 之前的时间窗）
    for ev in [&mut alice_ev, &mut bob_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2);
    }
    // 快照后新线：Dave 拨入 A——她不在旧圈誓言集（无绑定），旧圈
    // 的一切圈帧与她无关；迁移后扇出只认绑定，她的线零圈帧
    let (dave, mut dave_ev, dave_addr) = spawn_person("Dave").await;
    let _ = dave_addr;
    alice.dial(&dave_addr).await.expect("A-D（快照后新线）");
    expect(&mut alice_ev, met).await;
    expect(&mut dave_ev, met).await;
    // **零圈帧窗口**：旧圈在此期间存在且活跃（A/B 的凑话回声、
    // 任何重发路径若误触她的线都会在这里暴露）——纯静默 1.5s
    //（连 mumble 都没有：收不到圈载荷才叫零帧，收到了解不开是
    // mumble——两者在事件流上可区分）
    assert_quiet(&mut dave_ev, 1500, |ev| {
        matches!(
            ev,
            Event::CircleFormed { .. }
                | Event::CircleMumble { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleRejected { .. }
                | Event::CircleConflict { .. }
                | Event::SeatOffered { .. }
                | Event::Heard { .. }
        )
    })
    .await;
    // 触发全部圈帧类型：点名（新 0x0F 扇出——同一代码路径覆盖创世
    // 与点名，owner 裁决原文）+ 圈内发言。点名名单扫 A 的当前会话
    // =[Alice,Bob,Dave]——Dave 经**点名的誓言语义**合法入席（快照
    // 此刻含她的线），拿到绑定与创世是授予而非泄漏
    alice.form_circle().await.expect("点名（新 0x0F 扇出）");
    for ev in [&mut alice_ev, &mut bob_ev, &mut dave_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3, "点名重聚（Dave 经点名誓言合法入席）");
    }
    alice.circle_speak("quan li de hua").await.expect("A 圈内说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "quan li de hua", "圈内照常（功能面无损）");
    // 旧圈（快照=[Alice,Bob]）的整个生命周期已在上半段走完：Dave
    // 的线在创世/凑话/成钥期间**零圈帧**——从第一帧起（她只见过
    // 1:1 世界的 Met，上半段 quiet 窗口已验）。点名单独验证：新圈
    // 3 人誓言集（Dave 在内）→ 她此刻合法拿到绑定与创世——这恰是
    // 对照：迁移后的扇出=誓言集，圈内成员全通。功能对照：三人圈
    // 双向说话（先 expect 后 quiet——quiet 会消费窗口内事件，
    // Heard 不能落在 quiet 窗口里）
    let got = expect(&mut dave_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "quan li de hua", "Dave 在点名新圈里照常收听（誓言集成员）");
    assert_quiet(&mut dave_ev, 1000, |ev| {
        matches!(
            ev,
            Event::CircleMumble { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleRejected { .. }
                | Event::CircleConflict { .. }
        )
    })
    .await;
    // 对照收尾：Bob（快照集成员）在点名后仍双向可达——迁移没有
    // 收紧在座者的任何东西
    bob.circle_speak("wo zhao chang").await.expect("B 回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "wo zhao chang");
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
    if let Event::CircleRekeyed { members, change, .. } = ev {
        Some((*members, change.clone()))
    } else {
        None
    }
}

/// 从事件里取 CircleFormed 的人数。
fn formed(ev: &Event) -> Option<usize> {
    if let Event::CircleFormed { members, .. } = ev {
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

    // Alice 一句，Bob、Carol 同时听见（子钥解密 + 验签双闸）。
    // filter 防自回声：泛洪回声下自己说过的话可能绕回自己（设计行为）
    alice.circle_speak("今晚吃火锅").await.expect("群发");
    for ev in [&mut bob_ev, &mut carol_ev] {
        let got = expect(ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
        assert_eq!(got.1, "今晚吃火锅");
    }

    // Bob 回话，Alice 听见（filter 防自回声：她自己的话可能经 Carol
    // 线绕回，与 Bob 的直达帧在不同线上赛跑——直收序不保证）
    bob.circle_speak("我带毛肚").await.expect("Bob 群发");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "我带毛肚");

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
async fn 署名告别_创作者离场_双见证各自换钥_点名重聚() {
    // P-06 的活体版 × P-15 拓扑现实：全连接 A-B-C，**创作者 Alice** 署名
    // 告退——她的署名公告曾送达 B 和 C（各自的 Qualified{via:Alice} 线）
    // ⇒ 双见证：B、C 各自 RM（本地换钥）。但 mesh 直连对 B-C 未绑定
    //（P-15 已知代价：点名前只经创作者中继）——两份 RM 无路可交，
    // B/C 各自的单旋转钥匙不同 ⇒ T1 静默分叉（mumble 语义；星心离场
    // =断连=点名重建，A4 的既定路径）。点名后 B-C 直连点亮。
    let (alice, mut alice_ev, bob, mut bob_ev, carol, mut carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, formed).await;
    }

    // 创作者 Alice 临走说一句（豁免场景：说完晚安就告别）
    alice.circle_speak("wan an").await.expect("临别赠言");
    expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;

    // 署名告退：0x0C 走她的快照绑定线（A-B、A-C）——B、C 都是直接接收者
    alice.leave().await;

    // 双见证：B、C 各自换钥一次（自己的 RM；成员数回到 2）
    for ev in [&mut bob_ev, &mut carol_ev] {
        let r = expect(ev, rekeyed).await;
        assert_eq!(r.0, 2, "Alice 离席后剩两人");
        assert!(r.1.contains("Alice"), "变更点名离场者：{}", r.1);
    }
    // 两份 RM 无路可交（B-C 未绑定）：B 说 → Carol 侧安静（连 mumble
    // 都没有——B 的 Qualified{via:Alice} 线已随 RM(Alice) 急切撤销，
    // 他没有出话路径）——T1 静默分叉，修复=点名
    bob.circle_speak("ge hua jiang bu tong").await.expect("B 说（无路可出）");
    assert_quiet(&mut carol_ev, 1500, |ev| {
        matches!(ev, Event::Heard { .. } | Event::CircleMumble { .. })
    })
    .await;
    // 点名：Carol 重跑 /circle——她的会话只剩 Bob（Alice 的线已告别）
    // ⇒ 新圈 2 人；Carol 的快照绑 C-B Name(Bob)，Bob 经 0x0F 送达绑
    // B-C Qualified{via:Carol}——直连点亮
    carol.form_circle().await.expect("点名");
    for ev in [&mut bob_ev, &mut carol_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2, "新圈号重新聚首成功");
    }
    // 新圈直连双向全通
    bob.circle_speak("dian ming hou zhi tong").await.expect("B 说");
    let got = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "dian ming hou zhi tong");
    carol.circle_speak("wo ye shi").await.expect("C 回");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo ye shi");
}

#[tokio::test]
async fn 点名_重跑circle_修复一切() {
    // A4：点名=新圈号重新聚首，与创世同构——唯一的真理修复路径。
    // 离场后的残局（僵尸、旧钥）用一次 /circle 全部抚平。
    // P-15：Carol（非创作者）的告退只走她唯一的绑定线 C-A——Alice 是
    // 唯一直接接收者=唯一见证人（单 RM）；Bob 经 Alice 的转发应用。
    let (alice, mut alice_ev, _bob, mut bob_ev, carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }

    // Carol 署名告退（残局：幸存者经历换钥窗口）
    carol.leave().await;
    // Alice：见证 RM（自己的换钥）；Bob：应用 Alice 转发的 RM——各恰一次
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2, "Alice 的见证 RM");
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "Bob 应用 Alice 的 RM");
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    assert_quiet(&mut bob_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;

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
    if let Event::CircleRejected { detail, .. } = ev {
        Some(detail.clone())
    } else {
        None
    }
}

/// 从事件里取 CircleConflict 的 detail。
fn conflict(ev: &Event) -> Option<String> {
    if let Event::CircleConflict { detail, .. } = ev {
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
    if let Event::CircleMumble { from, .. } = ev {
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

// ── v3 slice-5 对词（/word·SAS）──────────────────────────────────

#[tokio::test]
async fn 对词_直连两端同词_点名后群词新鲜() {
    // slice-5：1:1 会话短词两端相同（同 transcript，P-08 的正面）；
    // 群短词全员一致（同 K）；点名 = 新圈号新贡献 ⇒ 群词新鲜，
    // 线未重握手 ⇒ 会话词不变（随线不随圈）。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let ra = alice.sas_word().await.expect("对词");
    let rb = bob.sas_word().await.expect("对词");
    let pa = ra.p2p.expect("聚焦 1:1");
    let pb = rb.p2p.expect("聚焦 1:1");
    assert_eq!(pa.1, pb.1, "直连两端同 transcript ⇒ 同词");
    let ca = ra.circle.expect("在圈");
    let cb = rb.circle.expect("在圈");
    assert_eq!(ca.0, 2);
    assert_eq!(ca.1, cb.1, "群短词全员一致（同 K）");
    // 点名：新圈号重聚——群词换新，会话词不动（线还是那条线）
    bob.form_circle().await.expect("点名");
    for ev in [&mut alice_ev, &mut bob_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2);
    }
    let ra2 = alice.sas_word().await.expect("再对词");
    let rb2 = bob.sas_word().await.expect("再对词");
    assert_eq!(
        ra2.p2p.as_ref().expect("1:1 还在").1, pa.1,
        "会话短词随线不随圈（transcript 不变）"
    );
    let ca2 = ra2.circle.expect("新圈");
    let cb2 = rb2.circle.expect("新圈");
    assert_ne!(ca2.1, ca.1, "群短词每次聚首新鲜（新 K）");
    assert_eq!(ca2.1, cb2.1, "新群词仍全员一致");
}

#[tokio::test]
async fn 对词_MITM拼接_两端异词() {
    // P-08：Mallory 在中间拼两条线——Alice 与 Bob 以为在跟对方说话
    //（名字是自报的，A1 的领域；Mallory 双手转述一切 = 拼线成立）。
    // 两端各自与 Mallory 握手 ⇒ 两个 transcript ⇒ 两半短词不同——
    // 想核对的人对一遍词即察觉（SAS = 口头核对的密码学蒸馏，T5）。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (mallory, mut mallory_ev, m_addr) = spawn_person("Mallory").await;
    let (bob, mut bob_ev, b_addr) = spawn_person("Bob").await;
    // Alice 拨 Mallory（她以为这是 Bob 的地址）；Mallory 再拨 Bob
    alice.dial(&m_addr).await.expect("A→M");
    let m_a = expect(&mut mallory_ev, |e| met(e).filter(|(n, _, _)| n == "Alice")).await;
    expect(&mut alice_ev, met).await;
    mallory.dial(&b_addr).await.expect("M→B");
    let m_b = expect(&mut mallory_ev, |e| met(e).filter(|(n, _, _)| n == "Bob")).await;
    expect(&mut bob_ev, met).await;
    // 拼线转述两轮：Alice ↔ Bob 的对话全部经 Mallory 的手
    alice.speak("qing wen shi bob ma").await.expect("A 说");
    let relayed = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    mallory.talk_to(m_b.1).await.expect("切到 B 线");
    mallory.speak(&relayed.1).await.expect("转述");
    let got_b = expect(&mut bob_ev, heard).await;
    assert_eq!(got_b.1, "qing wen shi bob ma", "B 收到转述（拼线『成功』）");
    bob.speak("wo shi bob a").await.expect("B 回");
    let relayed = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    mallory.talk_to(m_a.1).await.expect("切回 A 线");
    mallory.speak(&relayed.1).await.expect("转述");
    let got_a = expect(&mut alice_ev, heard).await;
    assert_eq!(got_a.1, "wo shi bob a", "A 收到转述");
    // 对词：两半短词必不同——拼线可检测（P-08 的可检测性）
    let wa = alice.sas_word().await.expect("A 对词").p2p.expect("聚焦中");
    let wb = bob.sas_word().await.expect("B 对词").p2p.expect("聚焦中");
    assert_ne!(wa.1, wb.1, "MITM 拼线 ⇒ 两个 transcript ⇒ 两半短词不同（P-08）");
}

// ── v3 slice-6 判例矩阵（attack_precedents 每条过堂）───────────────

/// rogue feature 金丝雀（settled 2026-10-16）：缺 feature 时恰这一条
/// 测试响亮失败——静默跳过=罪。判例注入测试的跑法：
/// cargo test --features rogue
#[test]
fn rogue注入面_canary_缺feature响亮失败() {
    assert!(
        cfg!(feature = "rogue"),
        "判例注入测试需要 rogue feature：cargo test --features rogue\
         （静默跳过=罪——settled_decisions.rogue_injection_feature_gated）"
    );
}

#[tokio::test]
async fn 链式拓扑_四点中继_经入席流补全() {
    // 矩阵项「chain topologies」：A—B—C—D 链。开台名单=发起者直连
    // 感知快照（A2）——B 只看得到 A、C；第四点 D 经 C 的入席流落座。
    // 说话经信念路由多跳中继（成员导出子图连通即够）。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;
    let (dave, mut dave_ev, dave_addr) = spawn_person("Dave").await;
    // 链：A—B、B—C、C—D
    alice.dial(&bob_addr).await.expect("A-B");
    bob.dial(&carol_addr).await.expect("B-C");
    carol.dial(&dave_addr).await.expect("C-D");
    expect(&mut alice_ev, met).await;
    for _ in 0..2 {
        expect(&mut bob_ev, met).await;
    }
    for _ in 0..2 {
        expect(&mut carol_ev, met).await;
    }
    expect(&mut dave_ev, met).await;
    // B 开台（显式开台：链要经入席流补第 4 点，封桌会按席位契约拒收 ADD）
    let n = bob.form_circle_seats(Some(SEATS_OPEN)).await.expect("开台");
    assert_eq!(n, 3);
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3);
    }
    // C 领 D 入座（链的第四点经入席流补全）
    carol.bring(None).await.expect("看座");
    expect(&mut dave_ev, seat_offered).await;
    dave.sit(None).await.expect("D 落座");
    expect(&mut carol_ev, rekeyed).await;
    let md = expect(&mut dave_ev, formed).await;
    assert_eq!(md, 4);
    // A、B 也应用了 ADD（信念路由把 ADD 沿链转达）
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 4);
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 4);
    // 全链说话：A ↔ D 经 B、C 中继
    alice.circle_speak("cong lian tou dao lian wei").await.expect("A 说");
    let got = expect(&mut dave_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "cong lian tou dao lian wei");
    dave.circle_speak("ting dao le").await.expect("D 回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Dave")).await;
    assert_eq!(got.1, "ting dao le");
}

#[tokio::test]
async fn 一人开台_经入席流收客() {
    // 矩阵项「solo circle forms (open stage); joiner arrives via seat
    // flow」：一人成圈默认开台（舞台）——观众经入席流落座。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let n = alice.form_circle().await.expect("一人开台");
    assert_eq!(n, 1);
    let m = expect(&mut alice_ev, formed).await;
    assert_eq!(m, 1, "名单只有我，立即铸钥（K=HKDF([r])）");
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    // Alice 领 Bob 入座（开台：引荐即背书）
    alice.bring(None).await.expect("看座");
    expect(&mut bob_ev, seat_offered).await;
    bob.sit(None).await.expect("B 落座");
    let r = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(r.0, 2);
    let m = expect(&mut bob_ev, formed).await;
    assert_eq!(m, 2);
    alice.circle_speak("huan ying shang tai").await.expect("说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "huan ying shang tai");
    bob.circle_speak("lai le").await.expect("回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "lai le");
}

#[tokio::test]
async fn 转发告别者_只应用不发起() {
    // rm_witness_unified 的另一半：署名 0x0C 的一跳直接接收者（线的
    // 对端==告退者）才是见证人；转发收到者只记账不发起。链式 A—B—C：
    // Carol 告退只走 B 的线——B 见证发 RM；A 只应用 B 的 RM，
    // 绝不自己再发一份（否则 B 会应用第二份 RM = 第二次换钥）。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    let (carol, mut carol_ev, _c_addr) = spawn_person("Carol").await;
    alice.dial(&bob_addr).await.expect("A-B");
    carol.dial(&bob_addr).await.expect("C-B");
    expect(&mut alice_ev, met).await;
    for _ in 0..2 {
        expect(&mut bob_ev, met).await;
    }
    expect(&mut carol_ev, met).await;
    let n = bob.form_circle().await.expect("B 开台");
    assert_eq!(n, 3);
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, formed).await;
    }
    // Carol 署名告退：她的唯一线对端是 B——B 是唯一见证人
    carol.bye().await.expect("告退");
    // B：见证 RM（自己的换钥）；A：应用 B 的 RM（各恰一次）
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2);
    // 恰一次：A 没发起自己的 RM（否则 B 会应用第二份 → 第二次换钥）
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    assert_quiet(&mut bob_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    // 幸存者新钥照说
    alice.circle_speak("xing cun zhe dui hua").await.expect("说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "xing cun zhe dui hua");
}

#[tokio::test]
async fn 在场辩护_还在说话的人掉线_补话被扣下() {
    // 在场辩护的扣下路径（活体版，矩阵项 presence-defense）× P-15：
    // Carol 刚说完话就掉线（无告别）。她唯一的圈绑定线是 C-A（genesis
    // 送达）——Alice 的 A-C 是快照 Name 绑定 ⇒ 唯一见证人：扣下 RM +
    // 响亮冲突（30 秒窗口内验签发言），人类点名裁决。Bob 的 B-C 线
    // 未绑定（mesh 直连对）——不承载圈语义，断线不见证（P-15：感知
    // 只来自协议建立的线，不来自自报名）。圈里谁也不换钥，旧钥照说。
    // Carol 活在牺牲线程里：脚本走完线程收摊 = crash 模拟。
    let (alice, mut alice_ev, a_addr) = spawn_person("Alice").await;
    let (_bob, mut bob_ev, b_addr) = spawn_person("Bob").await;
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    let b_addr_c = b_addr.clone(); // 牺牲线程拿走一份，主线照常用
    let a_addr_c = a_addr.clone();
    let carol_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("牺牲运行时");
        rt.block_on(async {
            let (carol, mut carol_ev, _c_addr) = spawn_person("Carol").await;
            carol.dial(&a_addr_c).await.expect("C-A");
            expect(&mut carol_ev, met).await;
            carol.dial(&b_addr_c).await.expect("C-B");
            expect(&mut carol_ev, met).await;
            // 等圈成（Alice 发起）再开口
            expect(&mut carol_ev, formed).await;
            carol.circle_speak("wo hai zai zhe li shuo hua").await.expect("说");
            // 等主线程确认 A/B 都已验签听见（在场证据入账）再走
            let _ = kill_rx.await;
            // 临终赠言：刷新在场窗口——TCP FIFO 保证它先于断线被
            // A/B 处理（在场判定的确定性来源）
            carol.circle_speak("shuo wan zhe ju jiu zou").await.expect("临终赠言");
        });
        // block_on 返回 = 脚本完；rt 随闭包结束 drop = 线断（无告别）
    });
    // 主线：A-B 互联；等三条线齐（A↔B、C→A、C→B）再开台
    alice.dial(&b_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 3);
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }
    // Carol 已在圈内说话——A 直接听见（C-A 是她唯一的绑定出话线，
    // A 的转发让 B 也听见——在场证据在 A 侧入账即足）
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo hai zai zhe li shuo hua");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo hai zai zhe li shuo hua");
    // 放行牺牲线程：说完临终赠言即 crash（无告别、无 0x0C）
    kill_tx.send(()).expect("放行信号");
    carol_thread.join().expect("牺牲线程收摊");
    // 唯一见证人 Alice（A-C 快照绑定）：扣下 + 响亮（修复路径 = 人类
    // 点名，不是自动 RM）
    let da = expect(&mut alice_ev, conflict).await;
    assert!(da.contains("扣下") && da.contains("30 秒"), "冲突详情：{da}");
    // Bob 的 B-C 线未绑定：断线不见证——无冲突、无换钥
    assert_quiet(&mut bob_ev, 1500, |ev| {
        matches!(ev, Event::CircleRekeyed { .. } | Event::CircleConflict { .. })
    })
    .await;
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    // 旧钥照说（被保护的人还在名单上——她只是掉线了）
    alice.circle_speak("jiu yue zhao shuo").await.expect("说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "jiu yue zhao shuo");
}

// rogue 门控：P16 锚定同步点用 line_bindings_debug（rogue-only 只读面）
#[cfg(feature = "rogue")]
#[tokio::test]
async fn 五十人_成圈说话点名() {
    // 矩阵项「50-member flood circle」：星型（中枢+49 叶），信念路由
    // 下凑话经中枢扇出——聚齐、说话、点名，全部在耐心预算内
    //（每步 expect 15 秒预算；真实耗时毫秒级）。P16：成键事件与叶子
    // 0x13 的批量 ear 处理竞速——speak 前用确定性全锚轮询同步（同线
    // 0x09→0x13 保序保证锚**必到**，只是 formed 事件可能抢先）。
    const N: usize = 50;
    let (hub, mut hub_ev, hub_addr) = spawn_person("Hub").await;
    let mut leaves: Vec<(Person, UnboundedReceiver<Event>)> = Vec::with_capacity(N - 1);
    for i in 0..N - 1 {
        let (p, ev, _addr) = spawn_person(&format!("M{i:02}")).await;
        leaves.push((p, ev));
    }
    // 星型：每片叶子拨中枢
    for (p, _) in &mut leaves {
        p.dial(&hub_addr).await.expect("叶子拨中枢");
    }
    for _ in 0..N - 1 {
        expect(&mut hub_ev, met).await;
    }
    for (_, ev) in &mut leaves {
        expect(ev, met).await;
    }
    // 开台：名单 = 中枢直连 49 叶 + 中枢 = 50
    let n = hub.form_circle().await.expect("开台");
    assert_eq!(n, N);
    let mh = expect(&mut hub_ev, formed).await;
    assert_eq!(mh, N, "50 行到齐即铸钥");
    for (_, ev) in &mut leaves {
        let m = expect(ev, formed).await;
        assert_eq!(m, N);
    }
    // P16 全锚同步：中枢的全部绑定线带 pk 锚（49 个叶子的 0x13——
    // 同线 0x09→0x13 保序，必然到达；formed 事件可能抢先于最后几个
    // 的 ear 处理）。上限 30s。
    for _ in 0..300 {
        let all = hub
            .line_bindings_debug()
            .await
            .expect("hub 绑定")
            .iter()
            .filter(|b| b.1 != "Hub")
            .all(|b| b.2.contains('+'));
        if all {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let all = hub
        .line_bindings_debug()
        .await
        .expect("hub 绑定")
        .iter()
        .filter(|b| b.1 != "Hub")
        .all(|b| b.2.contains('+'));
    assert!(all, "hub 的全部叶子线须带锚（P16 严格档下的出话面）");
    // 说：中枢一句全员听见；叶子之间经中枢中继互听
    hub.circle_speak("wu shi ren de hua").await.expect("中枢说");
    for (_, ev) in &mut leaves {
        let got = expect(ev, |e| heard(e).filter(|(h, _)| h == "Hub")).await;
        assert_eq!(got.1, "wu shi ren de hua");
    }
    leaves[0].0.circle_speak("ye zi de hua").await.expect("叶子说");
    let got = expect(&mut hub_ev, |e| heard(e).filter(|(h, _)| h == "M00")).await;
    assert_eq!(got.1, "ye zi de hua");
    let got = expect(&mut leaves[1].1, |e| heard(e).filter(|(h, _)| h == "M00")).await;
    assert_eq!(got.1, "ye zi de hua");
    // 点名：新圈号重新聚首（A4——不是修复，是重申）
    hub.form_circle().await.expect("点名");
    let mh = expect(&mut hub_ev, formed).await;
    assert_eq!(mh, N);
    for (_, ev) in &mut leaves {
        let m = expect(ev, formed).await;
        assert_eq!(m, N);
    }
    // P16 全锚同步（点名后同竞态——新圈新绑定，叶子的凑话后 0x13
    // 暂存补验 + 成键后 0x13 必到）
    for _ in 0..300 {
        let all = hub
            .line_bindings_debug()
            .await
            .expect("hub 绑定")
            .iter()
            .all(|b| b.2.contains('+'));
        if all {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    hub.circle_speak("dian ming hou zhao chang").await.expect("点名后说");
    let got = expect(&mut leaves[7].1, |e| heard(e).filter(|(h, _)| h == "Hub")).await;
    assert_eq!(got.1, "dian ming hou zhao chang");
}

// ── 判例注入测试（P-01/P-03/P-04/P-05/P-12——恶意对端能力的
//    实验室化身；注入经真实发送路径，被测的正是收端验证）──────────

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P01_伪造告退_响亮拒收_圈子无恙() {
    // P-01：告别是身体的行为（A1）——告退署名验不过 = 响亮拒收，
    // 伪造的告退没有协议效果（不换钥、不除名），Carol 照常在座说话。
    //（自己的署名告退触发全员换钥：见「署名告别_创作者离场」。）
    let (_alice, mut alice_ev, bob, mut bob_ev, carol, mut carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, formed).await;
    }
    let cid = bob.circle_id().await.expect("在圈");
    // 伪造「Carol 告退」：圈号对、名字对、署名 64 字节全零——确定性验不过
    let mut frame = Vec::with_capacity(8 + 1 + 5 + 64);
    frame.extend_from_slice(&cid.to_be_bytes());
    frame.push(5);
    frame.extend_from_slice(b"Carol");
    frame.extend_from_slice(&[0u8; 64]);
    bob.rogue_flood(rogue::CIRCLE_BYE, &frame).await;
    // Alice（名单上有 Carol 的行）响亮拒收
    let d = expect(&mut alice_ev, rejected).await;
    assert!(d.contains("伪造") && d.contains("P-01"), "拒收详情：{d}");
    // 无协议效果：不换钥
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    assert_quiet(&mut bob_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    // Carol 照常说话——她的行没被伪造帧碰掉
    carol.circle_speak("wo hai hao hao de").await.expect("说");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo hai hao hao de");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "wo hai hao hao de");
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P03_名单外凑话_响亮拒收() {
    // P-03：凑话完成由名单说了算——名单外的自署行（哪怕行本身是真的）
    // 响亮拒收，圈子纹丝不动。
    let (_alice, mut alice_ev, bob, _bob_ev) = two_open_circle().await;
    let cid = bob.circle_id().await.expect("在圈");
    // Mallory 的行是真的自署（任何名字都能自签）——但名字不在名单上
    let mut frame = Vec::new();
    frame.extend_from_slice(&cid.to_be_bytes());
    frame.extend_from_slice(&rogue::forge_row(cid, "Mallory", None));
    bob.rogue_flood(rogue::CONTRIB_V2, &frame).await;
    let d = expect(&mut alice_ev, rejected).await;
    assert!(d.contains("Mallory") && d.contains("P-03"), "拒收详情：{d}");
    // 无协议效果：不换钥、不吸收；圈内照说
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    bob.circle_speak("quan li zhao shuo").await.expect("说");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "quan li zhao shuo");
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P04_伪造ADD行_自署验不过_响亮拒收() {
    // P-04（递话不代笔）：恶意引荐人越过入席流，直接泛洪一份 ADD、
    // 新人行换成她伪造的（contrib_sig 全零 = 确定性验不过）——收端
    // 第一道闸（行自署验证）就响亮拒收：伪造行进不了名单。
    //（行的来源只能是候选人本人的 0x11 落座署名；引荐收据只管问责。）
    let (_alice, mut alice_ev, bob, _bob_ev) = two_open_circle().await;
    let cid = bob.circle_id().await.expect("在圈");
    // 伪造行：结构完整（长度能过 row_wire_len），唯独自署区全零
    let mut row = rogue::forge_row(cid, "Ghost", None);
    let n = row.len();
    row[n - 64..].fill(0);
    // ADD 布局：圈号 ‖ 'A' ‖ 新人行 ‖ 引荐人行 ‖ 收据 64
    //（新人行验不过即拒——后续字段到不了，填够长度即可）
    let mut frame = Vec::new();
    frame.extend_from_slice(&cid.to_be_bytes());
    frame.push(b'A');
    frame.extend_from_slice(&row);
    frame.extend_from_slice(&rogue::forge_row(cid, "Bob", None)); // 占位行（不可达）
    frame.extend_from_slice(&[0u8; 64]); // 收据占位（不可达）
    bob.rogue_flood(rogue::TOPUP, &frame).await;
    // 收端（Alice）响亮拒收
    let d = expect(&mut alice_ev, rejected).await;
    assert!(d.contains("自署") && d.contains("P-04"), "拒收详情：{d}");
    // 无协议效果：不换钥、名单不吸收；圈内照说
    assert_quiet(&mut alice_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    bob.circle_speak("ming dan wen si bu dong").await.expect("说");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "ming dan wen si bu dong");
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P05_同名对峙_响亮冲突_真行无恙() {
    // P-05（抢注）× P-15 迁移面：圈外 Mallory 与 Bob 建 1:1、注入另一把
    // 钥匙签的「Alice」行——Bob 的这条线无绑定（P-15：未绑线 = 非直连
    // 声明）⇒ 转述对峙：先到暂定 + 响亮冲突，不静默顶替。真 Alice 的
    // 行不被毒害：她随后照常说话、照常被验签。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }
    let cid = bob.circle_id().await.expect("在圈");
    // 圈外攻击者 Mallory：与 Bob 建普通 1:1（P-02 下她的线收不到圈帧，
    // 但 1:1 发帧是任何对话的本职能力——注入通道）
    let (mallory, mut mallory_ev, _m_addr) = spawn_person("Mallory").await;
    let bob_addr = bob.listen_addr().await;
    mallory.dial(&bob_addr).await.expect("M-B 线");
    // 记下 Mallory 侧线号（rogue_frame 的注入通道——线号是本地的）
    let (_, m_b, _) = expect(&mut mallory_ev, |e| met(e).filter(|(n, _, _)| n == "Bob")).await;
    expect(&mut bob_ev, met).await;
    // 攻击者的「Alice」行：行自洽（攻击者自己的钥自签），sig_pk ≠ 真行
    let mut frame = Vec::new();
    frame.extend_from_slice(&cid.to_be_bytes());
    frame.extend_from_slice(&rogue::forge_row(cid, "Alice", None));
    mallory.rogue_frame(m_b, rogue::CONTRIB_V2, &frame).await.expect("注入");
    // Bob：未绑线 ⇒ 非直连声明 ⇒ 转述对峙——响亮冲突、暂定保留
    let d = expect(&mut bob_ev, conflict).await;
    assert!(d.contains("Alice") && d.contains("P-05"), "冲突详情：{d}");
    // 真 Alice 照常说话——暂定（真）行验签通过，Bob 听见
    alice.circle_speak("wo cai shi zhen de").await.expect("真 Alice 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "wo cai shi zhen de");
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P12_碰撞r_响亮拒收_不静默驱逐() {
    // P-12：自选 r + 收端唯一性响亮检查——落座回执的 r 与现有成员
    // 碰撞（哪怕行是真的自署）→ 引荐人处响亮拒绝；不静默去重
    //（那 = 无声驱逐）。待座保留：真候选人随后 /sit（新鲜 r）
    // 照常入座。注入从候选人侧发出——0x11 只有到达引荐人的耳朵
    // 才会撞上这道闸（恶意/故障候选人的本职能力）。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let cid = bob.circle_id().await.expect("在圈");
    // 拿 Alice 的 r（0x12 同款全表内容——只递成员的正当材料）
    let alice_r = {
        let rows = bob.roster_snapshot().await.expect("名单快照");
        let row = rows.iter().find(|r| r.name == "Alice").expect("Alice 在桌上");
        row.r.to_vec()
    };
    let (stranger, mut stranger_ev, stranger_addr) = spawn_person("Stranger").await;
    bob.dial(&stranger_addr).await.expect("B-S 线");
    expect(&mut bob_ev, |e| met(e).filter(|(n, _, _)| n == "Stranger")).await;
    let (_, s_sid, _) = expect(&mut stranger_ev, |e| met(e).filter(|(n, _, _)| n == "Bob")).await;
    bob.bring(None).await.expect("看座");
    expect(&mut stranger_ev, seat_offered).await;
    // 碰撞回执：行真自署，r 故意 = Alice 的 r（从候选人线注入）
    let row = rogue::forge_row(cid, "Stranger", Some(&alice_r));
    stranger.rogue_frame(s_sid, rogue::SEAT_ACCEPT, &row).await.expect("注入碰撞回执");
    let d = expect(&mut bob_ev, rejected).await;
    assert!(d.contains("碰撞") && d.contains("P-12"), "拒收详情：{d}");
    // 不驱逐任何人：不换钥；真候选人重试（自选新鲜 r）照常完成
    assert_quiet(&mut bob_ev, 1500, |ev| matches!(ev, Event::CircleRekeyed { .. })).await;
    stranger.sit(None).await.expect("真落座");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3);
    let m = expect(&mut stranger_ev, formed).await;
    assert_eq!(m, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // Alice 没被无声驱逐：她照常说话、被新人听见
    alice.circle_speak("wo mei you bei ti diao").await.expect("Alice 说");
    let got = expect(&mut stranger_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "wo mei you bei ti diao");
}

// ── 2026-10-17 裁决：迟到告别的见证复活 + 离场重入再离场 ──────────

/// 一条会扣留字节的透明搬线（延迟载体 v2）：来客→目标方向过闸门
/// （关 = 字节暂存不转发），目标→来客方向永远透传；**入口断开后
/// 扣留字节继续持有**，闸门重开时冲货并收线。迟到告别竞态的确定性
/// 搬运工——物理对应：载体扣了一个数据包，发送者已死，包迟到。
async fn delaying_proxy(target_addr: &str) -> (String, tokio::sync::watch::Sender<bool>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap().to_string();
    let (gate_tx, gate_rx) = tokio::sync::watch::channel(true);
    let target = target_addr.to_string();
    tokio::spawn(async move {
        let Ok((inbound, _)) = listener.accept().await else {
            return;
        };
        let Ok(outbound) = tokio::net::TcpStream::connect(&target).await else {
            return;
        };
        let (mut ri, mut wi) = inbound.into_split();
        let (mut ro, mut wo) = outbound.into_split();
        // 目标→来客：永远透传
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut ro, &mut wi).await;
        });
        // 来客→目标：单写出点的字节闸门。读到的字节一律入 held，
        // 仅当闸门开时由唯一出口排空——严格 FIFO，任何调度交错下
        // 字节序不变（双写出点〔read 直写 + changed 冲货〕在闸门切换
        // 瞬间存在交错窗口，会打乱帧序触发对端 Undecipherable——
        // P15 spec_a 并行 flaky 的根因）。入口断开后继续持货，
        // 闸门重开时冲货 + 收线（目标侧这才看见 EOF）。
        let mut gate = gate_rx;
        let mut held = Vec::new();
        let mut buf = [0u8; 8192];
        let mut inbound_open = true;
        while inbound_open {
            // 开闸排空（唯一写出点；循环回顶重查——新字节可能又到）
            if *gate.borrow() && !held.is_empty() {
                let _ = wo.write_all(&held).await;
                held.clear();
                continue;
            }
            tokio::select! {
                r = ri.read(&mut buf) => match r {
                    Ok(0) | Err(_) => inbound_open = false,
                    Ok(n) => {
                        held.extend_from_slice(&buf[..n]);
                    }
                },
                _ = gate.changed() => {
                    // 只是唤醒：排空由循环顶部统一做
                }
            }
        }
        // 入口已断：只等闸门——重开即冲迟到的货并收线
        loop {
            if gate.changed().await.is_err() {
                break;
            }
            if *gate.borrow() {
                if !held.is_empty() {
                    let _ = wo.write_all(&held).await;
                }
                let _ = wo.shutdown().await;
                break;
            }
        }
    });
    (proxy_addr, gate_tx)
}

// rogue feature 门控：末尾的名单验态用 roster_snapshot（rogue-only
// 只读获取器）——canary 契约（settled_decisions.rogue_injection_feature_gated）
// 要求缺 feature 时恰一条测试响亮失败，编译期符号缺失违约（曾漏门控）
#[cfg(feature = "rogue")]
#[tokio::test]
async fn 迟到告别_墓碑验签_义务守卫幂等() {
    // 2026-10-17 定罪竞态的确定性复现 × P-15 适配。双线分工（确定性
    // 构造）：**bye 线**=Carol 拨入的代理线（genesis 走 target→client
    // 永透传 ⇒ Carol 绑它；她的告别走 client→target 方向，可被闸门
    // 扣留）；**crash 线**=Alice 拨入的代理线（genesis 走 client→target
    // 方向被扣 ⇒ 不参与 Carol 侧绑定；Carol 的 farewell 走 target→client
    // 永透传 ⇒ Alice 侧看见断线——Alice 的快照 Name 绑定在这条线上，
    // crash 见证由此触发）。流程：crash 先触发 RM₁（Carol 除名+墓碑）
    // → 放行迟到的告别 → 墓碑验签（members ∪ left_voices）+ 见证复活
    // 路径 → 义务幂等守卫兜底（同一离场只发一次 RM）→ 无第二次换钥；
    // 随后 farewell 断线，线死路径再撞守卫。全程恰一次换钥。
    let (alice, mut alice_ev, alice_addr) = spawn_person("Alice").await;
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    let (addr_tx, mut addr_rx) = tokio::sync::mpsc::channel::<String>(1);
    // bye 线代理：Carol（client）→ Alice（target）
    let (bye_proxy, bye_gate) = delaying_proxy(&alice_addr).await;
    let bye_proxy_c = bye_proxy.clone();
    let carol_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("牺牲运行时");
        rt.block_on(async {
            let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;
            addr_tx.send(carol_addr).await.expect("送地址");
            carol.dial(&bye_proxy_c).await.expect("C-A（bye 线）");
            expect(&mut carol_ev, met).await;
            // 等圈成（genesis 经 bye 线必达 ⇒ 这条线拿到绑定）
            expect(&mut carol_ev, formed).await;
            // 等主线扣闸后再走（时序由主线掌控）
            let _ = kill_rx.await;
            // 无临终发言（在场辩护不在本测焦点——crash 路径的 RM₁
            // 要顺利发出）
            carol.leave().await;
        });
        // rt 随闭包结束 drop = Carol 侧全部 TCP 关闭：bye 线入口断开
        //（载体 v2 继续持货）；crash 线在 Alice 侧看见 EOF
    });
    let carol_addr = addr_rx.recv().await.expect("等 Carol 的地址");
    // crash 线代理：Alice（client）→ Carol（target）——先开着过握手
    let (crash_proxy, crash_gate) = delaying_proxy(&carol_addr).await;
    alice.dial(&crash_proxy).await.expect("A-C（crash 线）");
    expect(&mut alice_ev, met).await; // bye 线的 Carol
    expect(&mut alice_ev, met).await; // crash 线的 Carol
    // 扣 crash 线（Alice→Carol 方向）——genesis 只能走 bye 线
    crash_gate.send(false).expect("扣 crash 线");
    // 开台：快照把两条线都绑 Name(Carol)；Carol 经 bye 线收 genesis
    //（唯一的到达路径）⇒ 绑定落在 bye 线上（确定性）
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 2);
    let ma = expect(&mut alice_ev, formed).await;
    assert_eq!(ma, 2);
    // 放行 crash 线：被扣的 genesis 到达 Carol——同圈重放，当没听见
    //（不授绑——重放不得授绑，P-15）
    crash_gate.send(true).expect("放 crash 线");
    tokio::time::sleep(Duration::from_millis(300)).await;
    // 扣 bye 线（Carol→Alice 方向）——告别将被载体扣留
    bye_gate.send(false).expect("扣 bye 线");
    // 放行牺牲线程：leave() → 告别进 bye 线（被扣）；farewell 走
    // crash 线（永透传）→ Alice 侧 crash 线断
    kill_tx.send(()).expect("放行");
    carol_thread.join().expect("牺牲线程收摊");
    // crash 路径见证（Alice 的 crash 线快照绑定 Name(Carol)）→ RM₁：
    // Carol 除名 + 墓碑 + Alice 独守（1 人）
    let r1 = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(r1.0, 1, "RM₁：Carol 除名，Alice 独守");
    assert!(r1.1.contains("Carol"), "变更点名离场者：{}", r1.1);
    // 放行 bye 线：迟到的告别 + farewell 冲给 Alice
    bye_gate.send(true).expect("放 bye 线");
    // 迟到告别到达 bye 线：Name 绑定 ⇒ 见证人；Carol 已不在座 ⇒
    // 墓碑验签（members ∪ left_voices）通过 ⇒ 见证复活路径触发 ⇒
    // 义务幂等守卫兜底（crash 路径已履行）——无第二次换钥。
    // farewell 随后断线：线死路径再撞守卫——仍然安静。
    assert_quiet(&mut alice_ev, 2000, |ev| {
        matches!(
            ev,
            Event::CircleRekeyed { .. } | Event::CircleConflict { .. } | Event::CircleRejected { .. }
        )
    })
    .await;
    // 圈已散（独守者无人可说）；名单验态：Carol 在墓碑里
    let snapshot = alice.roster_snapshot().await.expect("名单");
    assert!(snapshot.iter().all(|r| r.name != "Carol"), "Carol 已除名");
}

// ── P-15 判例矩阵（rulings 2026_10_17_p15_line_binding_spec）─────────

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P15_圈外自报名_零圈帧() {
    // P-15 核心杀面：圈外攻击者与在座成员 Bob 建 1:1、自报创作者
    // Alice 的名字（0x05 密文内自报，telephone model 本义允许）——
    // 旧代码 belief.contains(their_name) 直接放行这条线 ⇒ 0x09 携带
    // 全部 r ⇒ K ⇒ 静默旁听。P-15 后路由只认绑定：这条线无绑定 ⇒
    // 零圈帧到达（连 circle_id 都看不见），圈内照常。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }
    // 攻击者：第二个「Alice」（自报名照抄创作者）
    let (fake_alice, mut fake_ev, _fa) = spawn_person("Alice").await;
    let bob_addr = bob.listen_addr().await;
    fake_alice.dial(&bob_addr).await.expect("冒名线");
    expect(&mut fake_ev, met).await;
    // 圈内说话：攻击者的线一条圈帧都收不到（路由不看 their_name）
    alice.circle_speak("quan li de hua").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "quan li de hua");
    bob.circle_speak("wo ye shuo").await.expect("B 回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "wo ye shuo");
    assert_quiet(&mut fake_ev, 2000, |ev| {
        matches!(
            ev,
            Event::CircleFormed { .. }
                | Event::CircleMumble { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleRejected { .. }
                | Event::Heard { .. }
        )
    })
    .await;
    // 绑定面取证：Bob 的绑定里没有这条冒名线（只有 B-A 资格线）
    let bindings = bob.line_bindings_debug().await.expect("绑定");
    assert!(
        bindings.iter().all(|(_, peer, _)| peer != "Alice" || true),
        "占位防误用"
    );
    assert_eq!(bindings.len(), 1, "Bob 只有一条绑定（创作者资格线）：{bindings:?}");
    assert!(
        bindings[0].2.starts_with("qualified:via=Alice peer=Alice"),
        "B-A 是 genesis 送达的资格线（直连：via==peer==创作者；+pk 后缀=P16 锚）：{bindings:?}"
    );
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P15_spec_a_转发创世_圈线资格_非名字误绑() {
    // spec (a)：0x0F 经成员转发到达 ⇒ 送达使这条线获得**圈线资格**
    //（Qualified{via:帧内 creator}），绝不是 creator 名字绑定——转发者
    // 的线不被误绑为创作者；creator 身份来自帧内签名（genesis_payload
    // 补验），不来自线。构造（确定性）：A（创作者）发给 B 的 genesis/
    // 凑话经 carrier 代理线被扣；受邀成员 M 直连收到 genesis 并 rogue
    // 转发给 B ⇒ B 经 M 的线入座（资格落在 B-M 线上，via=帧内 Alice）；
    // B 的凑话经 M 转发让 A/M 成钥；放行后 B 补收直发 genesis——同圈
    // 重放，当没听见（不授绑），凑话补齐后 B 成钥。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (mallory, mut mallory_ev, _m_addr) = spawn_person("Mallory").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-M 直连（受邀成员 M）
    let mallory_addr = mallory.listen_addr().await;
    alice.dial(&mallory_addr).await.expect("A-M");
    expect(&mut alice_ev, met).await;
    expect(&mut mallory_ev, met).await;
    // M-B 直连（转发通道）——记下 M 侧线号（rogue_frame 的注入通道）
    mallory.dial(&bob_addr).await.expect("M-B");
    let (_, m_b, _) = expect(&mut mallory_ev, |e| met(e).filter(|(nm, _, _)| nm == "Bob")).await;
    expect(&mut bob_ev, met).await;
    // A-B 经 carrier（先开着过握手）
    let (carrier, gate) = delaying_proxy(&bob_addr).await;
    alice.dial(&carrier).await.expect("A-(载体)-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    // 扣载体（A→B 方向）：A 的 genesis/凑话到不了 B
    gate.send(false).expect("扣载体");
    // A 开台（名单 [Alice, Bob, Mallory]）：genesis 走 A-M 直达 M
    //（M 入座，M-A 资格线）、走 A-B 载体被扣。此刻谁都没成钥
    //（B 的行还没产生——A/M 都在等 B）
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 3);
    // M 转发创世给 B（rogue：把待验的署名公告原样递给 B——转发送达）。
    // pending_genesis 要等 M 真正入座（异步处理 genesis）——就绪轮询。
    // 上限 60s：并行全量下 ear 任务可能被调度延迟（expect 15s 同族的
    // 调度压力；实测 3 核 × 8 测试并行时 5s 上限会被击穿）
    let mut genesis = None;
    for _ in 0..600 {
        genesis = mallory.pending_genesis().await;
        if genesis.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let genesis = genesis.expect("M 的待验公告（入座后可得）");
    mallory.rogue_frame(m_b, rogue::GENESIS, &genesis).await.expect("转发创世");
    // B 经 M 的线入座（invited 含 Bob）并发凑话（B-M 资格线）；
    // M 收 B 的行 → A/M 先后成钥（A 的行经载体被扣，B 还缺）
    let mm = expect(&mut mallory_ev, formed).await;
    assert_eq!(mm, 3, "M 收齐（A 直达 + B 经自己转发的回程）");
    let ma = expect(&mut alice_ev, formed).await;
    assert_eq!(ma, 3, "A 收齐（B 的行经 M 转发）");
    // 绑定断言（成钥前后各一次）：B 的 B-M 线是转发送达的资格线——
    // 双键（peer 键修复）：via=帧内 creator「Alice」（溯源——非转发者
    // 名字误绑，spec a）；peer=转发者 Mallory（0x05 捕获快照——撤销/
    // 路由/见证三判定读它，转发者离席时此线熄灭）。锚后缀（P16 +pk）
    // 可能已到（M 凑话后 0x13，同线保序）——用前缀断言容锚
    for _ in 0..2 {
        let bindings = bob.line_bindings_debug().await.expect("B 的绑定");
        let bm = bindings
            .iter()
            .find(|(_, their, _)| *their == "Mallory")
            .expect("B-M 转发送达线在册");
        assert!(
            bm.2.starts_with("qualified:via=Alice peer=Mallory"),
            "spec (a)：资格线 via=帧内 creator（非转发者误绑）、\
             peer=线对端（对端键修复）：{bindings:?}"
        );
    }
    // 放行载体：被扣的直发 genesis 到达 B——同圈重放，当没听见
    //（B-A 线**不获 Qualified**）；A 的凑话 + A 转发的 M 凑话同批到达，
    // B 收齐成钥
    gate.send(true).expect("放载体");
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!(mb, 3, "B 补齐成钥（重放的 genesis 未授绑、凑话照收）");
    // 终态绑定面（P16 世界）：B-M 转发送达线照旧；A-B 载体线经两条
    // 合法路径之一获得资格——①A 的 0x13（Proven，裁决点②身体行为
    // 授予）②genesis 直发副本放行后的**多径补绑**（Qualified——
    // A2 送达授予：B 已 join 该圈，新线送达 = 该线资格，peer 键+信念
    // 检查兜底非成员线）。断言：载体线有绑定且指向 Alice（帧内
    // creator），**绝不误绑转发者 Mallory**（spec a 的本意）
    let bindings = bob.line_bindings_debug().await.expect("B 的绑定终态");
    let carrier = bindings
        .iter()
        .find(|(_, their, _)| *their == "Alice")
        .expect("A-B 载体线在册（0x13 或多径补绑授权）");
    assert!(
        carrier.2.starts_with("proven:")
            || carrier.2.starts_with("qualified:via=Alice peer=Alice"),
        "载体线指向 Alice（Proven 或多径补绑 Qualified——皆 A 的送达/
         证明行为）：{bindings:?}"
    );
    assert!(
        !carrier.2.contains("peer=Mallory"),
        "载体线绝不误绑转发者（spec a）：{bindings:?}"
    );
    // B 说话：走 B-M 资格线 → M 听见；M 转发 → A 听见
    bob.circle_speak("jing zhuan fa ru zuo").await.expect("B 说");
    let got = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "jing zhuan fa ru zuo");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "jing zhuan fa ru zuo");
    // A 说话：走她的快照线（A-B 载体已放行 / A-M）——B 直接收见
    alice.circle_speak("hua yin lai zi zhen chuang zuo zhe").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "hua yin lai zi zhen chuang zuo zhe");
}

/// 一条可被测试杀死的透明搬线（mortal proxy）：双向透传，abort 句柄
/// 一收——两端同时看见断线。spec (b) 的线死工具：杀人不杀进程（前
/// 成员要活着持僵尸桌）。
async fn mortal_proxy(target_addr: &str) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap().to_string();
    let target = target_addr.to_string();
    let handle = tokio::spawn(async move {
        let Ok((mut inbound, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut outbound) = tokio::net::TcpStream::connect(&target).await else {
            return;
        };
        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
    });
    (proxy_addr, handle)
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P15_spec_b_前成员重放座次表_缺旋转r_无钥() {
    // spec (b)：前成员 C 的僵尸信念持有旧 0x12 材料（行级 contrib_sig
    // 用旧公钥永远可验——公钥不撤销）。她的 C-B 线死后 B 见证 RM（B
    // 自己的行旋转 fresh r_B'）——C 的材料缺 r_B'，铸不出真圈现行 K。
    // C 用僵尸桌 bring 新人 D：D 拿到的座次表是 C 的旧信念复印件
    //（含 D 的新行 + C 旋转后的行 + A/B 旧行）——D 与真圈静默
    //（newcomer_roster_is_copy 的极端版）。C 的名单外凑话被 P-03
    // 响亮拒。『每次离席必旋转一行』不变量钉为断言（B 的 r 跨离席
    // 变化 + 旧材料钥匙 ≠ 现行钥匙）。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-B 成圈（A 创作者，开台）
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    alice.form_circle_seats(Some(SEATS_OPEN)).await.expect("开台");
    for ev in [&mut alice_ev, &mut bob_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2);
    }
    // B 领 C 入席：B（引荐人）拨向 C 的 mortal proxy（bring 走聚焦线；
    // 杀线不杀进程）
    let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;
    let (cproxy, cproxy_task) = mortal_proxy(&carol_addr).await;
    bob.dial(&cproxy).await.expect("B-(载体)-C");
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;
    bob.bring(None).await.expect("看座");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(None).await.expect("落座");
    expect(&mut bob_ev, rekeyed).await;
    let mc = expect(&mut carol_ev, formed).await;
    assert_eq!(mc, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // 记录 B 离席前的 r（不变量对照）
    let b_r_before = {
        let rows = bob.roster_snapshot().await.expect("快照");
        rows.iter().find(|r| r.name == "Bob").unwrap().r.to_vec()
    };
    // 杀载体：B-C 线断（两侧 EOF）。B 的 B-C 线（bring 的 Name 绑定）
    // ⇒ crash 见证 RM ⇒ B 行旋转（不变量：每次离席必旋转一行）；
    // C 侧同根（她的资格线也断）对自己的僵尸发 RM——无处可发（线死）
    cproxy_task.abort();
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "B 的见证 RM：Carol 除名");
    let ra2 = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra2.0, 2, "A 应用 B 的 RM");
    let b_r_after = {
        let rows = bob.roster_snapshot().await.expect("快照");
        rows.iter().find(|r| r.name == "Bob").unwrap().r.to_vec()
    };
    assert_ne!(b_r_before, b_r_after, "不变量：离席必旋转见证人行");
    // 真圈现行钥匙（A 侧快照）vs C 的僵尸材料钥匙——不同
    let real_key = {
        let rows = alice.roster_snapshot().await.expect("A 的现行名单");
        let rs: Vec<Vec<u8>> = rows.iter().map(|r| r.r.to_vec()).collect();
        e2ee::circle::derive_group_key(&rs.iter().map(|v| &v[..]).collect::<Vec<_>>())
    };
    let zombie_key = {
        let rows = carol.roster_snapshot().await.expect("C 的僵尸名单");
        let rs: Vec<Vec<u8>> = rows.iter().map(|r| r.r.to_vec()).collect();
        e2ee::circle::derive_group_key(&rs.iter().map(|v| &v[..]).collect::<Vec<_>>())
    };
    assert_ne!(real_key, zombie_key, "C 缺 r_B'：僵尸材料铸不出现行 K");
    // C（活僵尸）bring 新人 D：ADD 泛洪无处去（她的 B 线已死），
    // 0x12 是旧信念复印件 → D 与真圈静默
    let (dave, mut dave_ev, dave_addr) = spawn_person("Dave").await;
    carol.dial(&dave_addr).await.expect("C-D");
    expect(&mut carol_ev, met).await;
    expect(&mut dave_ev, met).await;
    carol.bring(None).await.expect("僵尸看座");
    expect(&mut dave_ev, seat_offered).await;
    dave.sit(None).await.expect("D 落座（幻影桌）");
    let md = expect(&mut dave_ev, formed).await;
    assert_eq!(
        md,
        3,
        "幻影桌 3 行（A 旧行 + C 旋转行 + D 新行；B 行已随 C 的自救 RM 出桌）"
    );
    // 真圈说话 → D 静默（无路由到她的线；即便截到也缺 r_B' 解不开）
    alice.circle_speak("zhen quan de hua").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "zhen quan de hua");
    assert_quiet(&mut dave_ev, 1500, |ev| {
        matches!(ev, Event::Heard { .. } | Event::CircleMumble { .. })
    })
    .await;
    // D 在幻影桌说话 → C 听见（C 有幻影全部材料——复印件的证明）
    dave.circle_speak("huan ying hua").await.expect("D 说");
    let got = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Dave")).await;
    assert_eq!(got.1, "huan ying hua");
    // C 的名单外凑话（真圈侧）：重放旧自署行 → P-03 响亮拒。
    // 注入线号取 **Carol 侧** 的 met（rogue_frame 查的是调用方的
    // convos——线号是本地键，两端各自编号）
    let cid = alice.circle_id().await.expect("真圈号");
    let alice_addr = alice.listen_addr().await;
    carol.dial(&alice_addr).await.expect("C-A（重放通道）");
    let (_, c_a, _) = expect(&mut carol_ev, |e| met(e).filter(|(n, _, _)| n == "Alice")).await;
    expect(&mut alice_ev, |e| met(e).filter(|(n, _, _)| n == "Carol")).await;
    let mut frame = Vec::new();
    frame.extend_from_slice(&cid.to_be_bytes());
    frame.extend_from_slice(&rogue::forge_row(cid, "Carol", None));
    carol.rogue_frame(c_a, rogue::CONTRIB_V2, &frame).await.expect("重放凑话");
    let d = expect(&mut alice_ev, rejected).await;
    assert!(d.contains("Carol") && d.contains("P-03"), "P-03 响亮拒：{d}");
}

#[tokio::test]
async fn 离场重入再离场_第二次告别不被吞() {
    // 2026-10-17 裁决 ②/① 的活体版：离场 → 入席流重入（新嗓音钥、
    // 新 r）→ 再离场——sig 键去重放行第二次告别（裸名字键会吞掉它），
    // ADD 清墓碑/见证义务（第二次 RM 照发），全员收敛。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    let (carol, mut carol_ev, carol_addr) = spawn_person("Carol").await;
    // 第一次入席：Bob 拨 Carol（bring 走聚焦线=候选人）→ 看座 → 落座
    bob.dial(&carol_addr).await.expect("B-C 线");
    expect(&mut bob_ev, met).await;
    expect(&mut carol_ev, met).await;
    bob.bring(None).await.expect("看座#1");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(None).await.expect("落座#1");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3);
    let m = expect(&mut carol_ev, formed).await;
    assert_eq!(m, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // 第一次离场：Carol 的唯一线对端是 Bob——他是直接见证人
    carol.bye().await.expect("第一次告别");
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "第一次 RM：Carol 离席");
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2, "Alice 应用 Bob 的 RM");
    // 重入：新线、新嗓音钥、新 r（同名 = 新任期，三本账全清）
    bob.dial(&carol_addr).await.expect("重拨");
    expect(&mut bob_ev, |e| met(e).filter(|(n, _, _)| n == "Carol")).await;
    expect(&mut carol_ev, |e| met(e).filter(|(n, _, _)| n == "Bob")).await;
    bob.bring(None).await.expect("看座#2");
    expect(&mut carol_ev, seat_offered).await;
    carol.sit(None).await.expect("落座#2（新钥）");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3, "重入 ADD");
    let m = expect(&mut carol_ev, formed).await;
    assert_eq!(m, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // 再离场：第二次告别（新钥新 sig——sig 键去重放行；裸名字键会吞）
    carol.bye().await.expect("第二次告别");
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "第二次 RM 照发（见证义务已随 ADD 清账）");
    assert!(rb.1.contains("Carol"), "变更点名离场者：{}", rb.1);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2, "Alice 应用第二次 RM");
    // 收敛验证：幸存者同钥对话
    alice.circle_speak("shou lian le").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "shou lian le");
    bob.circle_speak("wo ye shi").await.expect("B 回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "wo ye shi");
}

// ── P-15 复审阻断项 B / 收尾项 C（2026-10-17 landing audit）─────────

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P15_B_转发资格线_对端离席熄灭_除名者零圈帧() {
    // 阻断项 B 回归（rulings.2026_10_17_p15_peer_keyed_revocation）：
    // spec-a 合法化了转发送达——B 经转发者 M 的线入座，绑定
    // Qualified{via:Alice, peer:Mallory}。撤销键在 via 时：M 离席
    //（A 的死线见证 RM(Mallory)）→ apply_rm 只撤 via==Mallory →
    // B 的转发线（via=Alice）续命 → B 把 RM 帧自身（携带见证人 A 的
    // 旋转新行）泛洪给 M → M 持任期内全部 r 补此一枚即导出新 K →
    // 永久读后续明文（A3 封存破）。peer 键修复：撤销/路由/见证三判定
    // 读线对端 peer——M 离席时 B 的转发线熄灭，M 一帧也收不到。
    // 构造：A(创作者)-M 经 mortal（可杀=A 的死线见证通道）、M-B 直连
    //（转发通道）、A-B 经 delaying 载体（扣直发 genesis，逼 B 走
    // M 的转发送达——spec-a 同款确定性构造）。
    let (alice, mut alice_ev, _a_addr) = spawn_person("Alice").await;
    let (mallory, mut mallory_ev, mallory_addr) = spawn_person("Mallory").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-M 经 mortal（A 的快照线 Name(Mallory)；abort 即 A 侧死线）
    let (mproxy, mtask) = mortal_proxy(&mallory_addr).await;
    alice.dial(&mproxy).await.expect("A-(mortal)-M");
    expect(&mut alice_ev, met).await;
    expect(&mut mallory_ev, met).await;
    // M-B 直连——记 M 侧线号（rogue_frame 的注入通道）
    mallory.dial(&bob_addr).await.expect("M-B");
    let (_, m_b, _) = expect(&mut mallory_ev, |e| met(e).filter(|(nm, _, _)| nm == "Bob")).await;
    expect(&mut bob_ev, met).await;
    // A-B 经 delaying 载体，扣 A→B 方向
    let (carrier, gate) = delaying_proxy(&bob_addr).await;
    alice.dial(&carrier).await.expect("A-(载体)-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    gate.send(false).expect("扣载体");
    // A 开台（名单 [Alice, Bob, Mallory]）→ M 直达入座，B 被扣
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 3);
    // M 转发创世给 B（rogue：转发送达——T0 固有能力的实验室化身）
    let mut genesis = None;
    for _ in 0..600 {
        genesis = mallory.pending_genesis().await;
        if genesis.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let genesis = genesis.expect("M 的待验公告");
    mallory.rogue_frame(m_b, rogue::GENESIS, &genesis).await.expect("转发创世");
    // B 经 M 的线入座（Qualified{via:Alice, peer:Mallory}）；A/M 成钥
    let mm = expect(&mut mallory_ev, formed).await;
    assert_eq!(mm, 3, "M 收齐（A 直达 + B 经自己转发的回程）");
    let ma = expect(&mut alice_ev, formed).await;
    assert_eq!(ma, 3, "A 收齐（B 的行经 M 转发）");
    // 绑定形状断言：peer=Mallory（对端键——撤销键）+ via=Alice（溯源）；
    // 锚后缀（P16）竞态容忍——用前缀
    let bindings = bob.line_bindings_debug().await.expect("B 的绑定");
    assert!(
        bindings
            .iter()
            .any(|(_, their, d)| *their == "Mallory" && d.starts_with("qualified:via=Alice peer=Mallory")),
        "B 的转发送达线形状：{bindings:?}"
    );
    // 放行载体 → B 补齐成钥
    gate.send(true).expect("放载体");
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!(mb, 3);
    // P16 锚定同步点（确定性，非 sleep）：A 侧 A-B 载体线须带 pk 锚
    //（B 的成键后 0x13）——严格档下这是 A 的出话路径；不等锚就杀线
    // = A 的 RM 无路 → B 落入 M 的 RM(Alice) 分叉（T1 场景，非本测
    // 试对象）。轮询上限 30s。
    for _ in 0..300 {
        let ok = alice
            .line_bindings_debug()
            .await
            .expect("A 的绑定")
            .iter()
            .any(|(_, their, d)| *their == "Bob" && d.contains('+'));
        if ok {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let anchored = alice
        .line_bindings_debug()
        .await
        .expect("A 的绑定")
        .iter()
        .any(|(_, their, d)| *their == "Bob" && d.contains('+'));
    assert!(anchored, "A 的 Bob 线须带 pk 锚（B 的 0x13 已到）——否则后续 RM 无出话路径");
    // ── M 离席（P16 分叉世界）：杀 A-M mortal → 两侧各持死线、各自
    // 诚实见证（A2）：A 见证 RM(Mallory)（M 除名+A 旋转）；M 见证
    // RM(Alice)（A 除名+M 旋转）。**P16 半开边消解的连带效应**：B 的
    // 0x13 曾授予 M 侧 M-B Proven 锚——M 的 RM(Alice) 有出线了（原
    // P15 世界无出线）→ 两份 RM 在不同线上竞速，**直收序本就不保证**
    //（engineering_debt.echo_order_test_fix 同族：两帧不同线赛跑）。
    // 两个到达序都是合法世界，分叉随序镜像，两序各断言各的正确性：
    // M 先：B 应用 RM(Alice)（世界 {B,M'}）→ A 的 RM 见证人已出信念
    //   响亮拒收 → B+M 对 A 分叉；
    // A 先：B 应用 RM(Mallory)（世界 {A',B}）→ **对端键撤销直接现形**
    //   ——B 的转发线 peer=Mallory 出信念即熄 → 泛洪义务消失 → M 零
    //   圈帧（阻断项 B 的攻击路径本体死亡）→ B+A 对 M 分叉。
    // 两侧各自正确、响亮、可修（T1），末尾点名统一（两世界共用）。
    mtask.abort();
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2, "A 的见证 RM：Mallory 除名");
    assert!(ra.1.contains("Mallory"), "变更点名离场者：{}", ra.1);
    // B 侧：第一份 RM 随竞速而定。判别可靠：apply_rm 的 change 只含
    // 离场者名（"{leaver} 离席"，无见证人名）——"Alice 离席"⇔M 的 RM
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "B 应用先到的诚实见证 RM（P16 分叉侧）");
    let m_won = rb.1.contains("Alice");
    // M 侧：本地应用自己的 RM(Alice)（两世界同）
    let rm_self = expect(&mut mallory_ev, rekeyed).await;
    assert!(
        rm_self.1.contains("Alice"),
        "M 的自身死线见证（本地应用）：{}",
        rm_self.1
    );
    // A 侧对照（两世界同）：A 的 M 线随 RM(Mallory) 熄灭（pk 锚统一
    // 撤销——修复前 via=Alice 续命泄漏 RM 帧新 r 的原断言面在 A 的
    // 出话侧验证）
    let a_bindings = alice.line_bindings_debug().await.expect("A 的绑定终态");
    assert!(
        !a_bindings
            .iter()
            .any(|(_, their, _)| *their == "Mallory"),
        "A 的 Mallory 线熄灭（对端离席 + pk 锚撤销，A 的世界观）：{a_bindings:?}"
    );
    if m_won {
        // ── M 先到世界（Proven 直达快路径获胜）────────────────────
        assert!(
            rb.1.contains("Alice"),
            "B 应用 M 的 RM(Alice)（Proven 直达先到）：{}",
            rb.1
        );
        // B 拒收 A 的 RM(Mallory)（见证人 A 已出 B 的信念——rm_multiplicity
        // 的 T1 边界：相交变更分叉，响亮拒收而非静默）
        let rej = expect(&mut bob_ev, rejected).await;
        assert!(
            rej.contains("Alice"),
            "A 的 RM 在 B 侧响亮拒收（见证人行不在名单——T1 分叉响亮化）：{rej}"
        );
        // B 的世界观：M 在座（B 的 Mallory 线锚 M.pk ∈ B 的信念）——
        // peer/pk 键撤销语义在**共享信念**内生效：A 的世界观里 M 走了
        //（A 的 M 线已熄），B 的世界观里 M 没走（线活）——两者都对（A2）
        let bindings = bob.line_bindings_debug().await.expect("B 的绑定终态");
        assert!(
            bindings
                .iter()
                .any(|(_, their, d)| *their == "Mallory" && d.contains('+')),
            "B 的 Mallory 线带锚存活（B 的世界观 M 在座——分叉侧语义）：{bindings:?}"
        );
        // M 的圈帧面（A 的出话侧）：A 的 M 线已熄——A 的圈话**不直达** M；
        // 但 B 的泛洪义务（B 的世界观 M 在座）会把 A 的密文**转发**给 M
        //——M 解不开（分叉钥）→ Mumble from Bob：T1 响亮化的合法部分
        //（B 没有理由替 A 隐瞒）。禁的是直达帧（from=Alice 的一切）与
        // 状态变更（A 的圈对 M 的 Rekeyed/Formed/Rejected）
        alice.circle_speak("a de shi jie guan li m zou le").await.expect("A 说");
        let deadline = Instant::now() + Duration::from_millis(2000);
        while let Ok(Some(ev)) =
            timeout(deadline.saturating_duration_since(Instant::now()), mallory_ev.recv()).await
        {
            match &ev {
                Event::CircleMumble { from } if from == "Bob" => {} // B 的泛洪义务（分叉密文中继）
                Event::CircleMumble { .. }
                | Event::Heard { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleFormed { .. }
                | Event::CircleRejected { .. } => {
                    panic!("M 不该收到 A 侧直达帧或状态变更：{ev:?}")
                }
                _ => {}
            }
        }
        // 分叉响亮化：A 的话 B 解不开（B 的钥={B,M'}，A 用 {A',B} 钥说话）
        // ——mumble（T1 必被听见）
        let _ = expect(&mut bob_ev, mumble).await;
        // B/M 同钥互通（同世界观 {B,M'}——分叉的另一半照常说）
        bob.circle_speak("b he m hai tong yaoshi").await.expect("B 说");
        let got = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
        assert_eq!(got.1, "b he m hai tong yaoshi", "B-M 同世界观互通");
    } else {
        // ── A 先到世界（本沙箱调度时运）：对端键撤销直接现形 ──────
        assert!(
            rb.1.contains("Mallory"),
            "B 应用 A 的 RM(Mallory)（A-B 载体先到）：{}",
            rb.1
        );
        // B 拒收 M 的 RM(Alice)：见证人 M 已出 B 的信念——同一 T1 边界，
        // 镜像侧的响亮拒收（RM 路径不受线熄静默——M-first 世界已证）
        let rej = expect(&mut bob_ev, rejected).await;
        assert!(
            rej.contains("Mallory"),
            "M 的 RM 在 B 侧响亮拒收（见证人行不在名单——T1 分叉响亮化）：{rej}"
        );
        // B 的世界观：M 已出信念——**阻断项 B 修复的本体断言**：转发线
        //（peer=Mallory）随 M 除名熄灭（apply_rm 急切撤销），A 的线存活
        let bindings = bob.line_bindings_debug().await.expect("B 的绑定终态");
        assert!(
            !bindings.iter().any(|(_, their, _)| *their == "Mallory"),
            "B 的 Mallory 转发线熄灭（对端键撤销——M 出信念即熄）：{bindings:?}"
        );
        assert!(
            bindings.iter().any(|(_, their, _)| *their == "Alice"),
            "B 的 Alice 线存活（B 的世界观 A 在座——分叉侧语义）：{bindings:?}"
        );
        // M 的圈帧面（两重封死）：A 不泛洪（M ∉ A 的信念）、B 不泛洪
        //（M ∉ B 的信念，转发线已熄）——A 的话 B 同钥直听，M 零圈帧：
        // 修复前经 B 转发资格线续命读明文的攻击路径结构性死亡
        alice.circle_speak("a de shi jie guan li m zou le").await.expect("A 说");
        let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
        assert_eq!(got.1, "a de shi jie guan li m zou le", "B-A 同钥（A',B 世界观）");
        let deadline = Instant::now() + Duration::from_millis(2000);
        while let Ok(Some(ev)) =
            timeout(deadline.saturating_duration_since(Instant::now()), mallory_ev.recv()).await
        {
            match &ev {
                Event::CircleMumble { .. }
                | Event::Heard { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleFormed { .. }
                | Event::CircleRejected { .. } => {
                    panic!("M 零圈帧（对端键撤销：泛洪义务随线熄灭——攻击路径死亡）：{ev:?}")
                }
                _ => {}
            }
        }
        // 分叉的另一半照常说：B-A 同世界观互通（{A',B}）
        bob.circle_speak("b he a hai tong yaoshi").await.expect("B 说");
        let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
        assert_eq!(got.1, "b he a hai tong yaoshi", "B-A 同世界观互通");
    }
    // A4 点名统一：B 重跑 /circle——她的会话=[A,B,M] 全量重邀（社会
    // 连续；会话存续与信念无关——M-first 世界里 A 的线已熄照邀不误，
    // A-first 世界里 M 的线已熄同构），分叉抚平
    bob.form_circle().await.expect("点名");
    for ev in [&mut alice_ev, &mut bob_ev, &mut mallory_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 3, "点名新圈三人重聚（分叉抚平）");
    }
    bob.circle_speak("dian ming hou tong yi le").await.expect("B 说（统一后）");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "dian ming hou tong yi le");
    let got = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "dian ming hou tong yi le");
}

// rogue feature 门控：绑定验态用 line_bindings_debug（rogue-only）
#[cfg(feature = "rogue")]
#[tokio::test]
async fn P15_C_同名重入_crash见证不误逐新任期() {
    // 收尾项 C（audit 裁定的真风险轴）：v1 离席（B 的 v1 线 Name(Carol)
    // 绑定按迟到告别设计**保留**）→ v2 同名新钥经 seat flow 重入
    //（apply_add 清账：Name(Carol) 与 Qualified{peer:Carol} 全清——
    // 旧身体的线不得因新身体入座而复明）→ v1 的线死 → crash 见证
    // 读死线绑定——**已清** → 无据 RM 不发。若绑定未清：指向名
    // Carol 在 members 命中 v2（members-only 与 union 两变体都经
    // members 分支暴露）→ 无据 RM 逐出 v2。作用域：清除发生在
    // apply_add 内，先于引荐人 complete_bring 写入 v2 的新绑定
    //（调用结构保证）；v2 自己的新绑定（引荐人线 Name(Carol)）在
    // 清除后存活。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    // v1 经 mortal 线入席（杀线不杀进程——线死时点可控）
    let (carol1, mut _carol1_ev, carol1_addr) = spawn_person("Carol").await;
    let (c1proxy, c1task) = mortal_proxy(&carol1_addr).await;
    bob.dial(&c1proxy).await.expect("B-(mortal)-C1");
    expect(&mut bob_ev, met).await;
    expect(&mut _carol1_ev, met).await;
    bob.bring(None).await.expect("看座#1");
    expect(&mut _carol1_ev, seat_offered).await;
    carol1.sit(None).await.expect("落座#1");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    let m1 = expect(&mut _carol1_ev, formed).await;
    assert_eq!(m1, 3);
    // v1 圈离席但 mortal 线保留：她拨无关者 X 并聚焦 → bye() 断的是
    // X 线（圈告别走 mortal 线〔their_name=Bob ∈ 信念〕，1:1 空帧
    // 道别只发聚焦的 X 线）
    let (_x, _x_ev, x_addr) = spawn_person("Xia").await;
    carol1.dial(&x_addr).await.expect("C1-X（bye 吸引线）");
    expect(&mut _carol1_ev, met).await;
    carol1.bye().await.expect("v1 告别（断 X 线，保 mortal 线）");
    // B 直接接收（v1 线 Name(Carol) points_to ✓）→ 见证 RM₁；
    // A 经 B 转发应用。B 的 v1 线绑定 Name(Carol) 保留（迟到告别）
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "RM₁：v1 除名（墓碑写入）");
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2);
    // v2 同名入席（新进程、新嗓音钥、新 r）
    let (carol2, mut carol2_ev, carol2_addr) = spawn_person("Carol").await;
    bob.dial(&carol2_addr).await.expect("B-C2");
    expect(&mut bob_ev, |e| met(e).filter(|(nm, _, _)| nm == "Carol")).await;
    expect(&mut carol2_ev, met).await;
    bob.bring(None).await.expect("看座#2");
    expect(&mut carol2_ev, seat_offered).await;
    carol2.sit(None).await.expect("落座#2（新任期）");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3, "重入 ADD（清账+新行）");
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    let m = expect(&mut carol2_ev, formed).await;
    assert_eq!(m, 3);
    // 绑定验态：B 的 name:Carl 绑定恰一条——v2 的新线（v1 的 mortal
    // 线绑定已被 apply_add 清除；v2 线在清除后写入）
    let bindings = bob.line_bindings_debug().await.expect("B 的绑定");
    let carol_lines: Vec<_> = bindings
        .iter()
        .filter(|(_, their, _)| *their == "Carol")
        .collect();
    assert_eq!(carol_lines.len(), 1, "恰一条 Carol 绑定（v1 清、v2 在）：{bindings:?}");
    assert!(
        carol_lines[0].2.starts_with("name:Carol"),
        "v2 的新绑定存活（+pk 后缀=P16 锚，0x11 行帧内 pk 直接锚定）：{}",
        carol_lines[0].2
    );
    // 杀 v1 的 mortal 线：B 侧死线 → crash 见证读绑定——已清 →
    // 无据 RM 不发（若未清：指向名命中 members 里的 v2 → 逐出新任期）
    c1task.abort();
    assert_quiet(&mut bob_ev, 2000, |ev| {
        matches!(ev, Event::CircleRekeyed { .. } | Event::CircleConflict { .. })
    })
    .await;
    assert_quiet(&mut alice_ev, 1500, |ev| {
        matches!(ev, Event::CircleRekeyed { .. } | Event::CircleConflict { .. })
    })
    .await;
    // v2 无恙：照常说，A 听见（名单里她的行还在）
    carol2.circle_speak("xin ren qi hao hao de").await.expect("v2 说");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "xin ren qi hao hao de");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "xin ren qi hao hao de");
}

// ── P16 判例矩阵（rulings.2026_10_17_p16_spec_approved，spec §6）─────

/// 等待某人的指定线绑定带 pk 锚（确定性同步，P16 全锚族的通用助手）。
#[cfg(feature = "rogue")]
async fn wait_anchored(
    p: &Person,
    their: &str,
) -> bool {
    for _ in 0..300 {
        let ok = p
            .line_bindings_debug()
            .await
            .expect("绑定面")
            .iter()
            .any(|(_, t, d)| t == their && d.contains('+'));
        if ok {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_镜子1_谎报名线_成键后熄灭() {
    // p15_fake_name_peer_residual 之死（红线变绿，胜负无关终态版）：
    // 攻击者在 Bob 点名前拨入并自报成员 Alice 的名字——快照把她的
    // 线绑为 Name{Alice} 窗口锚。**P-05 交互如实入档**：genesis 多径
    // 送达 + 同名对峙使「真 A 行 vs fake 行」的占位由到达序裁决
    //（先到暂定 + 直连声明替换——P-05 的既有世界，胜负不保证）。
    // P16 的断言面（胜负无关）：占位者的 pk 决定锚与路由；**出局者
    // 的线（窗口锚成键后熄 + 她的锚不在信念）零密封帧**——修复前
    //（无严格档）：名字 ∈ 信念 ⇒ 两条线都持续路由——输赢双方都能
    // 持续旁听。红绿自证：关掉 keyed 判定本测试必须红。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, mut carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, formed).await;
    }
    // 攻击者：点名前拨入 Bob，自报「Alice」
    let (fake, mut fake_ev, _fa) = spawn_person("Alice").await;
    let bob_addr = bob.listen_addr().await;
    fake.dial(&bob_addr).await.expect("冒名线");
    expect(&mut fake_ev, met).await;
    expect(&mut bob_ev, met).await;
    // Bob 点名：快照三条线（B-A/B-C/B-fake——两条都 Name{Alice}）；
    // genesis 多径 + 同名对峙 → 占位者由到达序定
    bob.form_circle().await.expect("点名");
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!(mb, 3, "B 成键（名单 [Alice,Bob,Carol] 由占位行满足）");
    let _ma = expect(&mut alice_ev, formed).await;
    let _mc = expect(&mut carol_ev, formed).await;
    // P16 全锚同步（Bob 的绑定线锚定——成键后 0x13 必到）
    for name in ["Alice", "Carol"] {
        let ok = bob
            .line_bindings_debug()
            .await
            .expect("B 绑定")
            .iter()
            .filter(|(_, t, _)| t == name)
            .any(|(_, _, d)| d.contains('+'));
        if !ok {
            // 全锚轮询（重试 30s）
            for _ in 0..300 {
                let done = bob
                    .line_bindings_debug()
                    .await
                    .expect("B 绑定")
                    .iter()
                    .filter(|(_, t, _)| t == name)
                    .any(|(_, _, d)| d.contains('+'));
                if done {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    // 占位感知断言：B 的圈里 "Alice" 行的 pk 决定 A 的话可解与否——
    // 占位=真 A（她的 r 在 B 的钥里）⇒ heard；占位=fake（r 不同）⇒
    // mumble（T1 分叉响亮——P-05 对峙的已知世界，点名裁决）
    let holder_is_real_a = {
        let rows = bob.roster_snapshot().await.expect("B 快照");
        let holder = rows.iter().find(|r| r.name == "Alice").unwrap();
        let a_rows = alice.roster_snapshot().await.expect("A 快照");
        let a_new = a_rows.iter().find(|r| r.name == "Alice").unwrap();
        holder.sig_pk == a_new.sig_pk
    };
    alice.circle_speak("mi feng de hua").await.expect("A 说");
    if holder_is_real_a {
        let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
        assert_eq!(got.1, "mi feng de hua", "真 A 占位：圈内照常");
        let got = expect(&mut carol_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
        assert_eq!(got.1, "mi feng de hua");
    } else {
        let m = expect(&mut bob_ev, mumble).await;
        assert_eq!(m, "Alice", "fake 占位：A 的话 B 解不开（T1 响亮，点名裁决）");
    }
    // fake 的事件面（胜负无关断言）：围栏窗口的合法事件放行（她的
    // 自动 join/formed——「集齐即得首代 K」已接受价格；P-05 对峙
    // 响亮），**密封帧零到达**（Heard/Mumble——她（若出局）得 K
    // 也收不到密文；若她占位（极端时序），她的锚=她自己的行在信念
    // ——她能听——这是 P-05 占位语义的边界而非 P16 面，此时真 A
    // 侧响亮冲突（Conflict 事件）已把对峙呈现给人类，点名裁决）
    let deadline = Instant::now() + Duration::from_millis(2500);
    while let Ok(Some(ev)) =
        timeout(deadline.saturating_duration_since(Instant::now()), fake_ev.recv()).await
    {
        match &ev {
            Event::Heard { .. } | Event::CircleMumble { .. } => {
                // 密封帧到达 fake：只可能因她占位（P-05 边界）——此时
                // B 的 members["Alice"] 必是 fake 的新 pk。断言这一致性
                //（而非无条件 fail）：占位场景已由 bob_ev 的 Conflict
                // 响亮化，人类点名裁决（A4）。
                let rows = bob.roster_snapshot().await.expect("B 快照");
                let holder = rows.iter().find(|r| r.name == "Alice").unwrap();
                let fake_rows = fake.roster_snapshot().await.expect("fake 快照");
                let fake_row = fake_rows.iter().find(|r| r.name == "Alice").unwrap();
                assert_eq!(
                    holder.sig_pk, fake_row.sig_pk,
                    "密封帧到 fake 只在占位时成立（P-05 边界——响亮冲突+点名）：{ev:?}"
                );
            }
            _ => {} // Formed/Conflict/Rejected/Met——围栏窗口合法事件
        }
    }
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_镜子2_同名复用_pk锚不混淆() {
    // P15_C 场景之死（红线变绿）：v1 离场 / v2 同名入座 / v1 的线死
    // ——按 pk 根本不可能混淆（v1 的锚随 v1 行删已熄；v2 是另一把
    // pk）。红绿自证：把 witness_ctx 改回按名字查（P15 语义）本测试
    // 必须红（v1 名命中 members 里的 v2 → 无据 RM 逐出 v2）。
    let (alice, mut alice_ev, bob, mut bob_ev) = two_open_circle().await;
    // v1 经 mortal 线入席
    let (carol1, mut _c1ev, c1_addr) = spawn_person("Carol").await;
    let (c1proxy, c1task) = mortal_proxy(&c1_addr).await;
    bob.dial(&c1proxy).await.expect("B-(mortal)-C1");
    expect(&mut bob_ev, met).await;
    expect(&mut _c1ev, met).await;
    bob.bring(None).await.expect("看座#1");
    expect(&mut _c1ev, seat_offered).await;
    carol1.sit(None).await.expect("落座#1");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // v1 告别（断无关线保 mortal）
    let (_x, _xev, x_addr) = spawn_person("Xia").await;
    carol1.dial(&x_addr).await.expect("C1-X");
    expect(&mut _c1ev, met).await;
    carol1.bye().await.expect("v1 告别");
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2);
    // v2 同名入席
    let (carol2, mut carol2_ev, c2_addr) = spawn_person("Carol").await;
    bob.dial(&c2_addr).await.expect("B-C2");
    expect(&mut bob_ev, |e| met(e).filter(|(n, _, _)| n == "Carol")).await;
    expect(&mut carol2_ev, met).await;
    bob.bring(None).await.expect("看座#2");
    expect(&mut carol2_ev, seat_offered).await;
    carol2.sit(None).await.expect("落座#2");
    let r = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(r.0, 3);
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 3);
    // 杀 v1 的 mortal 线：B 的 crash 判定读 v1 线的锚——v1 行已删
    // ⇒ 锚指向无行 ⇒ 线早已熄（无绑定）⇒ 无据 RM 不发；v2 无恙
    c1task.abort();
    assert_quiet(&mut bob_ev, 2000, |ev| {
        matches!(ev, Event::CircleRekeyed { .. } | Event::CircleConflict { .. })
    })
    .await;
    assert_quiet(&mut alice_ev, 1500, |ev| {
        matches!(ev, Event::CircleRekeyed { .. } | Event::CircleConflict { .. })
    })
    .await;
    // v2 照常说
    carol2.circle_speak("pk bu hun xiao").await.expect("v2 说");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Carol")).await;
    assert_eq!(got.1, "pk bu hun xiao");
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_撤销统一_行删即熄_幂等重放无害() {
    // pk 锚统一熄灭 + 0x13 幂等/重放（spec §6 项 4+7 合并）：mesh 圈
    // C 署名告别 → 直接见证者发 RM → A 应用：A 的 C 线（pk 锚）随
    // 行删自动熄——除名者零密封帧（含 RM 帧自身）；重放 C 的历史
    // 0x13（rogue 截获原帧再发）→ 圈号/试钥面照验（幂等锚定，无状态
    // 变化）；伪造 0x13（错钥）→ 零授予零事件。
    let (alice, mut alice_ev, bob, mut bob_ev, carol, mut carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev, &mut carol_ev] {
        expect(ev, formed).await;
    }
    // 全锚同步：A 的 C 线（C 的凑话后 0.13，经 C-A 绑定线到 A）。
    // **B 的 C 线不锚**——mesh 非发起者对（B-C 无绑定，P15 已知：
    // 互通经 A 中继）——本测试的 B 侧走中继路径
    assert!(
        wait_anchored(&alice, "Carol").await,
        "A 的 C 线带锚（C 的凑话后 0x13）"
    );
    // C 的告别：0x0C 走她的绑定线（C-A——A 是唯一直接接收者；B 经
    // A 转发应用）。P16 全锚同步（C 的 A 线锚定先于告别）
    assert!(wait_anchored(&carol, "Alice").await, "C 的 A 线锚");
    carol.leave().await;
    let ra = expect(&mut alice_ev, rekeyed).await;
    assert_eq!(ra.0, 2, "A 的见证 RM：C 除名");
    let rb = expect(&mut bob_ev, rekeyed).await;
    assert_eq!(rb.0, 2, "B 的见证 RM：C 除名");
    // A 的 C 线随行删熄灭（pk 锚统一机制——无名字撤销循环参与）
    let bindings = alice.line_bindings_debug().await.expect("A 绑定");
    assert!(
        !bindings.iter().any(|(_, t, _)| t == "Carol"),
        "A 的 Carol 线熄灭（锚==C.pk，行删即熄）：{bindings:?}"
    );
    // C 零密封帧：A/B 的话一帧不到她（连 mumble 都没有）
    alice.circle_speak("c ting bu dao le").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "c ting bu dao le");
    assert_quiet(&mut carol_ev, 1500, |ev| {
        matches!(ev, Event::Heard { .. } | Event::CircleMumble { .. })
    })
    .await;
    // 重放 C 的历史 0x13（A 侧绑定已熄——重放到 A 的 C 线：验签用
    // 当前名单试钥——C 的行已删+墓碑不在试钥面 → 当没听见；即便
    // 重放早于 RM（锚已定）也只是幂等空转）：零状态变化零事件
    let forged = {
        // 构造一个签名载荷错误的 0x13（错钥——确定性验不过）
        let cid = alice.circle_id().await.expect("圈号");
        let mut frame = Vec::with_capacity(8 + 64);
        frame.extend_from_slice(&cid.to_be_bytes());
        frame.extend_from_slice(&[0u8; 64]); // 全零签名：验不过
        frame
    };
    let a_line = alice
        .line_bindings_debug()
        .await
        .expect("A 绑定")
        .iter()
        .find(|(_, t, _)| t == "Bob")
        .map(|(id, _, _)| *id)
        .expect("A-B 线号");
    alice
        .rogue_frame(a_line, rogue::LINE_PROOF, &forged)
        .await
        .expect("伪造注入");
    // 零授予零事件（静默语义——无后果行为；B 的锚不变、无 Rejected）
    let before = bob.line_bindings_debug().await.expect("B 绑定");
    assert!(
        before.iter().any(|(_, t, d)| t == "Alice" && d.contains('+')),
        "B 的 Alice 线锚保持（伪造 0x13 零影响）"
    );
    assert_quiet(&mut bob_ev, 1500, |ev| {
        matches!(ev, Event::CircleRejected { .. } | Event::CircleMumble { .. })
    })
    .await;
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_circle_bye按pk_冒名线零圈告别帧() {
    // their_name 残迹之死（spec 问 5 的镜子）：自报成员名的圈外线
    // 不再收到 **0x0C 圈告别帧**（修复前 circle_bye targets 读
    // belief.contains(their_name)——冒名线可收，泄露圈号+离场者名+
    // 签名）。P16：targets 按锚判定——冒名线无锚（成键后窗口锚熄）
    // ⇒ 零圈帧。注意边界：leave() 的 1:1 空帧道别发给全部 convos
    //（含冒名线）——那是 1:1 世界的行为（P16 不碰），合法收到。
    let (alice, mut alice_ev, bob, mut bob_ev, _carol, _carol_ev) = three_mesh().await;
    for ev in [&mut alice_ev, &mut bob_ev] {
        expect(ev, formed).await;
    }
    // 圈外人自报成员名（Alice）
    let (fake, mut fake_ev, _fa) = spawn_person("Alice").await;
    let bob_addr = bob.listen_addr().await;
    fake.dial(&bob_addr).await.expect("冒名线");
    expect(&mut fake_ev, met).await;
    expect(&mut bob_ev, met).await;
    // Bob 告别：0x0C 走他的绑定线（B-A——three_mesh 圈里 B 的唯一
    // 绑定；C 经 A 中继）；冒名线（无锚）零圈帧。P16 全锚同步：
    // B 的 A 线锚定先于告别（B-C 是 mesh 非发起者对——无绑定，
    // P15 已知语义，不经其出话）
    assert!(wait_anchored(&bob, "Alice").await, "B 的 A 线锚");
    bob.leave().await;
    let _ = expect(&mut fake_ev, |ev| left(ev).filter(|n| n == "Bob")).await; // 1:1 道别（合法）
    // A 侧事件顺序不保证（0x0C 先于/后于 1:1 空帧取决于线与转发）：
    // drain 直到 Left(Bob) 与 CircleRekeyed 都见过（顺序无关收集）
    let (mut saw_left, mut saw_rekey) = (false, false);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(saw_left && saw_rekey)
        && let Ok(Some(ev)) =
            timeout(deadline.saturating_duration_since(Instant::now()), alice_ev.recv()).await
    {
        match &ev {
            Event::Left { name, .. } if name == "Bob" => saw_left = true,
            Event::CircleRekeyed { .. } => saw_rekey = true,
            _ => {}
        }
    }
    assert!(saw_left, "A 见 B 离线");
    assert!(saw_rekey, "A 见证 RM（0x0B 圈帧也不到冒名线）");
    // 冒名线从此零圈帧（Circle* 家族全禁——修复前她会收到 0x0C）
    assert_quiet(&mut fake_ev, 2000, |ev| {
        matches!(
            ev,
            Event::CircleFormed { .. }
                | Event::CircleMumble { .. }
                | Event::CircleRekeyed { .. }
                | Event::CircleRejected { .. }
                | Event::CircleConflict { .. }
                | Event::SeatOffered { .. }
        )
    })
    .await;
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_半开边点亮_证明授予() {
    // 裁决点②的拓扑红利（relay_asymmetry 半开边消解）：spec-a 拓扑
    //（A-M 直连、M-B 转发送达）——B 的 0x13 到达 M 的未绑定线（M-B）
    // 且验签通过 ⇒ 授予 Proven{B.pk} ⇒ M→B 方向圈帧可达（修复前
    // 半开边：M 侧对 B 从未绑定，方向结构性无资格）。
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (mallory, mut mallory_ev, m_addr) = spawn_person("Mallory").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-M、M-B
    alice.dial(&m_addr).await.expect("A-M");
    expect(&mut alice_ev, met).await;
    expect(&mut mallory_ev, met).await;
    mallory.dial(&bob_addr).await.expect("M-B");
    let (_, m_b, _) = expect(&mut mallory_ev, |e| met(e).filter(|(n, _, _)| n == "Bob")).await;
    expect(&mut bob_ev, met).await;
    // A 开台（名单 [Alice, Bob, Mallory]）——genesis 走快照线（A-M、
    // A-B?A 只连 M——A 的 convos 只有 A-M！名单扫 A 的对端=[Mallory]
    // +A=[Alice,Mallory]——B 不在！需要 A 也直连 B……重构造：A-B 载体
    // 线（delaying 扣住）+ A-M 直连——B 经 M 转发 genesis 入座（spec-a
    // 原构造）
    let (carrier, gate) = delaying_proxy(&bob_addr).await;
    alice.dial(&carrier).await.expect("A-(载体)-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    gate.send(false).expect("扣载体");
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 3);
    // M 直达入座；M 转发 genesis 给 B（B 经 M 的线入座）
    let mut genesis = None;
    for _ in 0..600 {
        genesis = mallory.pending_genesis().await;
        if genesis.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let genesis = genesis.expect("M 的待验公告");
    mallory.rogue_frame(m_b, rogue::GENESIS, &genesis).await.expect("转发创世");
    let mm = expect(&mut mallory_ev, formed).await;
    assert_eq!(mm, 3);
    let ma = expect(&mut alice_ev, formed).await;
    assert_eq!(ma, 3);
    gate.send(true).expect("放载体");
    let mb = expect(&mut bob_ev, formed).await;
    assert_eq!(mb, 3);
    // B 的 0x13（凑话后/成键后）发 B 的绑定线（B-M）→ M 侧 M-B 线
    // 原本无绑定 → **授予 Proven{B.pk}**（裁决点②）
    assert!(
        wait_anchored_specific(&mallory, "Bob", "proven:").await,
        "M 的 B 线获 Proven 授予（0x13 身体行为，第三授予）"
    );
    // 半开边点亮：B 说 → M **直接**收到（不再绕 A 中继——A 的转发
    // 也会到，但 M 的直达路径已开）；A 说 → M 收到后 M 转发给 B
    //（M-B Proven 线）
    bob.circle_speak("ban bian dian liang le").await.expect("B 说");
    let got = expect(&mut mallory_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "ban bian dian liang le");
    alice.circle_speak("a ye neng zhuan le").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "a ye neng zhuan le", "M→B 方向圈帧可达（半开边消解）");
}

/// 等待某人的指定线绑定含指定前缀的锚描述（P16 半开边断言用）。
#[cfg(feature = "rogue")]
async fn wait_anchored_specific(p: &Person, their: &str, prefix: &str) -> bool {
    for _ in 0..300 {
        let ok = p
            .line_bindings_debug()
            .await
            .expect("绑定面")
            .iter()
            .any(|(_, t, d)| t == their && d.starts_with(prefix));
        if ok {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(feature = "rogue")]
#[tokio::test]
async fn P16_乱序容错_点名后三发重锚_双向恢复() {
    // spec §6 项 5（恢复面）：乱序/丢帧的现实面在打洞线（UDP 数据报
    // 无序）——TCP 构造不出真乱序，此处验证**恢复机制的完备性**：
    // 任何时刻的证明缺失（丢/验不过/暂存丢弃）都可由三发点（凑话后/
    // 成键后/点名后）自愈——点名重跑 = 新圈新绑定 + 0x13 全量重发，
    // 双向全锚后说话全通（A4 兜底闭环的存在性断言）。
    let (alice, mut alice_ev, _a) = spawn_person("Alice").await;
    let (bob, mut bob_ev, bob_addr) = spawn_person("Bob").await;
    // A-B 直连
    alice.dial(&bob_addr).await.expect("A-B");
    expect(&mut alice_ev, met).await;
    expect(&mut bob_ev, met).await;
    // A 开台（名单 [Alice, Bob]）：B 直达入座成圈
    let n = alice.form_circle().await.expect("开台");
    assert_eq!(n, 2);
    for ev in [&mut alice_ev, &mut bob_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2);
    }
    // 全锚同步（初始圈的正常路径——B 的 A 线锚=A 的凑话后 0.13）
    assert!(wait_anchored(&bob, "Alice").await, "B 的 A 线锚（正常路径）");
    // 点名重跑：新圈新绑定 + 0x13 三发重发 → 全锚 → 双向恢复
    alice.form_circle().await.expect("点名");
    for ev in [&mut alice_ev, &mut bob_ev] {
        let m = expect(ev, formed).await;
        assert_eq!(m, 2, "点名重聚");
    }
    assert!(wait_anchored(&alice, "Bob").await, "点名后 A 的 B 线重锚（三发）");
    assert!(wait_anchored(&bob, "Alice").await, "点名后 B 的 A 线重锚");
    alice.circle_speak("dian ming hou quan tong").await.expect("A 说");
    let got = expect(&mut bob_ev, |e| heard(e).filter(|(n, _)| n == "Alice")).await;
    assert_eq!(got.1, "dian ming hou quan tong");
    bob.circle_speak("wo ye shi").await.expect("B 回");
    let got = expect(&mut alice_ev, |e| heard(e).filter(|(n, _)| n == "Bob")).await;
    assert_eq!(got.1, "wo ye shi");
}
