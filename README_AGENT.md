# ═══════════════════════════════════════════════════════════════
# README_AGENT — machine-facing project brief
#
# FILE CLASS : machine-facing brief. Audience: LLM agents / coding copilots.
# HUMANS     : stop here. Use README.md / README_ZH.md instead.
# FORMAT     : yaml-dialect. indent=2; keys are stable IDs; `#` comments are
#              NORMATIVE (constraints), not decoration. File body is pure YAML.
#              STRUCTURAL INVARIANT: the whole file MUST parse as ONE valid
#              YAML document.
# SCOPE      : as-built facts describe HEAD (post-P2, pre-3.0.0). group_v3,
#              sas, seat_flow and pending_work carry OWNER-SETTLED DESIGNS not
#              yet written — every such block is status-tagged. NEVER document
#              planned work as built; NEVER re-propose anything listed in
#              settled_decisions (rejected items = permanent rejection).
#
# THE ONE RULE ABOVE ALL RULES: 本项目的第一性原理是对人类社会物理对话的
# 抽象（见 theory 四公理）。任何设计决策、任何代码变更、任何你（AI）想提的
# 新特性，都必须能从公理推导出来；推导不出的，默认拒绝，交 owner 裁决。
# ═══════════════════════════════════════════════════════════════

project:
  id: E2EE-Experiment
  lang: rust
  edition: "2024"
  nature:
    - p2p
    - e2ee-chat
    - single-binary
    - serverless
    - cli
  core_metaphor: executable == person        # NOT marketing — drives every decision below
  theory: conversation-physics               # NEW v3: see theory block — the project now HAS an explicit theory
  maturity: early-development                 # post-P2 HEAD; v3 axiom rebuild settled, not yet implemented
  ui_language: zh                            # user-facing strings are chinese; keep style
  comment_language: zh-with-person-metaphors # see glossary at bottom

doc_index:                                  # entry alignment across the 3 docs
  zh: README_ZH.md
  en: README.md
  agent: README_AGENT.md                     # this file
  rule: "README.md / README_ZH.md describe the P2 world and legitimately lag this file; they MUST be rewritten together with the 3.0.0 release (removals_3_0_0 + pending_work.P4)."

doc_protocol:                               # how to maintain THIS file
  update_triggers:
    - new wire frame kind or changed frame layout
    - owner ruling in review/conversation → append to settled_decisions with date
    - a pending_work item lands → move its facts into as-built sections, drop it from the board
    - a new falsified/rejected idea → append to settled_decisions.rejected
    - a new attack discovered & adjudicated → append to attack_precedents with an ID (P-nn)
    - an axiom-based ruling on any design question → record under theory.rulings
  yaml_selfcheck: "python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))' — MUST succeed"

# ───────────────────────────────────────────────────────────────
# THEORY — 对话物理学（v3 的第一因，2026-10-15 定稿）
# 这是整个项目的宪法。机制可以死，公理不可以。
# ───────────────────────────────────────────────────────────────

theory:
  origin: >-
    人类社会几千年博弈没有推翻的对话机制，就是经过最长安全审计的协议。
    本项目不做"发明"，做"翻译"：把物理对话的硬约束翻译成网络协议。
    翻译必须忠实——网络世界比物理世界多出来的每一种能力，要么主动废除，
    要么诚实声明为边界。这就是全部设计方法论。

  axioms:                                   # 公理：从物理对话提炼的硬约束
    A1_embodiment:                          # 公理 I：具身
      statement: 人 = 身体。在场、发声、离席都是身体的行为。身体不可分身、不可复制、死后不可再发言。
      network_translation: 人 = 进程实例。嗓音（署名钥）与生俱来、随进程死亡、一次性。
      forbids: [跨圈持续身份, TOFU, 账号体系, 可转让的发言权]
      generates: [一次性署名钥, 署名发言, 署名告别, 贡献自签（名单每行都是本人身体的印）]
    A2_locality:                            # 公理 II：局部性
      statement: 一切感知只来自直接物理信道。你能确认在场的人，就是你的感官够得着的人。不存在全球在场目录。
      network_translation: 节点的一切知识来自直连线。路由表 = 世界观 = 同一张表（信念名单）。
      forbids: [全局名册, epoch 共识, 官方成员表, 任何形式的"全局状态同步机"]
      generates: [信念路由, 见证人（死线持有者是离席的唯一合法证明人）, 分歧是常态而非故障, 创世名单=发起者感知快照]
    A3_irrevocability:                      # 公理 III：不可撤回
      statement: 说出的话、被听见的秘密，物理上收不回。你能改变的只有未来。
      network_translation: 换气（rekey）只封存未来。旧钥下的明文是历史，不是泄漏。
      generates: [离开者读不了新话（换气+断递）, 前向保密, 妥协=身体被占有（换气把绑架者关在未来之外）]
      ruling: PCS 缺口不是 bug——物理世界也不存在"让他忘掉已经听见的"操作。正面入档，见 known_limitations。
    A4_restartability:                      # 公理 IV：可重启
      statement: 无论对话损坏到什么程度，重新介绍、重新对暗语永远可用。这是物理世界从不失效的修复原语。
      network_translation: 点名（re-gather）与创世同构，永远可用，覆盖一切故障类别。
      generates: ["响亮分歧的修复路径", "分歧不可预防但必被听见、必可修复的全部承诺"]

  asymmetry_table:                          # 物理/网络不对称账本——完备性检验
    # 每一条"网络比物理多出来的能力"，协议必须主动废除（前四行）或诚实声明（后两行）。
    # 新特性设计时必须过这张表：你的机制让网络多出了什么能力？废除了吗？声明了吗？
    - { physical: 身体不可复制,        network: 进程可克隆,           countermeasure: "嗓音钥一次性，进程即身体（A1）" }
    - { physical: 声音不可完美模仿,    network: 字节可完美复制,       countermeasure: "验证只在直连上发生 + SAS 人类信道核对（A2）" }
    - { physical: 桌子稀缺,            network: "『房间』无限复制",   countermeasure: "席位（名单）重建稀缺性；封闭桌=物理边界（A1+A2）" }
    - { physical: 距离有成本,          network: 距离归零,             countermeasure: "不废除——载体可见元数据=桌子的物理被茶馆继承，诚实声明" }
    - { physical: 遗忘是默认,          network: 机器永不遗忘,         countermeasure: "零持久化+退出即焚——把网络世界修回物理（A4 的对偶）" }
    - { physical: 感知即时同步,        network: 消息异步无全序,       countermeasure: "不废除——响亮分歧（T1）+ 点名（A4）" }

  derivation_map:                           # 推导表：每个机制从哪条公理来
    PAKE_暗号: "接头暗号的抽象（A2：秘密经人类信道携带）"
    SAS_对词: "口头核对的密码学蒸馏（不对称表第 2 行）——无 SAS 则机密性定理无法陈述"
    泛洪分发: "A2 禁止目录 → 传播只能口耳相传（流言的物理）"
    贡献式密钥日程: "A2（无权威选钥）+ 对称性 → 密钥从平等贡献合成；A3 → 换气只换未来"
    署名发言双门: "A1（嗓音）+ A3（有后果的行为不可抵赖）"
    引荐收据: "A1 递话不代笔：名单行有本人签名，引荐人只能递复印件"
    见证人补话: "A2：离席只对死线持有者可观察"
    默认封闭席位: "不对称表第 3 行；一人成圈默认开台（舞台是刻意的开放，宣言而非默认）"
    响亮分歧: "A2（分歧必然）+ A3 对偶（分歧必被听见——AEAD 失败是感知，不是错误）+ A4（点名是修复）"
    点名_重新聚首: "A4：与创世同构；它是唯一的真理路径，覆盖一切修复场景"

  rulings:                                  # 公理裁决记录（向后一致性的证据）
    2026_10_15_bye_must_sign: "0x0C 告退该不该签名？A1：告别是身体的行为 → 必须签。判例 P-01 的理论根据。"
    2026_10_15_epoch_must_die: "epoch 字段该不该存在？A2：没有全局时钟序列 → epoch 是目录的残留 → 死。防重放由署名 counter 承担（A1：嗓音有时间性）。"
    2026_10_15_no_voting: "表决该不该有？A2：无全序 → 表决不可实现 → 快照+收据。"
    2026_10_15_sync_layer_dies: "P2 的同步机（epoch/initiator 选举/validity 规则/级联定时器）整体违反 A2。不是换更好的，是不需要。"
    2026_10_15_no_backoff_for_pake: "PAKE 猜错要不要退避？不要——SPAKE2 结构上每猜一次必须到场答一次（离线爆破数学上不存在），错误敲门在 Event 流里是响的，听见敲错门由人类感知免费完成。加退避器=检测式编码，违反哲学。暗号长短是宾客自己的选择。"

  decision_procedure:                       # 编码模型遇到任何设计问题时的裁决流程
    step_1: "找公理依据：提案的每个机制，问它由哪条公理生成。答不出 → 默认拒绝。"
    step_2: "过不对称表：你的机制让网络多出了什么物理世界没有的能力？废除或声明，二选一。"
    step_3: "查 settled_decisions：已裁决事项重提案 = 立即拒绝（rejected 清单是永久性的）。"
    step_4: "过判例库：新功能必须对 attack_precedents 每一条给出不被攻破的论证或测试。"
    step_5: "静默=罪：一切失败必须响亮化（mumble/事件/拒绝），没有静默裁决这条路。"
    step_6: "以上全部通过仍拿不准 → 停下来问 owner，不要自行发明语义。"

  theorems:                                 # 定理层（研究轨；Tamarin 形式化见 research_track）
    T1_响亮分歧: "两诚实节点信念不同 ⇒ 贡献集不同 ⇒ K₁≠K₂（HKDF 可区分性）⇒ 互验必失败。A2+密钥日程直接可证。"
    T2_交换代数: "不相交的变更（不同人走、不同人补）任意顺序应用终态相同（集合代数）；相交变更必落入 T1。补话快速路径的全部合法性来源。"
    T3_嗓音完整性: "验证通过的帧 ⇒ 归因到某聚首时刻被见证的进程实例（A1）。"
    T4_同意有界准入: "任何成员资格可回溯到签名链：创世快照，或某在座成员署名的引荐收据。不存在无邀请路径。"
    T5_SAS条件机密性: "抗（被动）窃听者机密性以 SAS 仪式为前提；无 SAS 则 MITM 拼接不可检测，定理无法陈述。"
    conjecture_trilemma: >-
      无身份 + 无服务器 + 收敛，三者不可兼得——收敛需要全局状态的函数，而 A2 禁止任何
      节点访问全局状态。我们只承诺响亮分歧，这是存在性辩护而非退让。以猜想形态入档
      （引 FLP/DLS/90s-GCS 谱系），不做定理承诺。

# ───────────────────────────────────────────────────────────────
# ARCHITECTURE (as-built at HEAD, post-P2)
# ───────────────────────────────────────────────────────────────

architecture:
  binary_count: 1                            # INVARIANT: deliverable is exactly one executable
  deps:
    - tokio
    - snow                                   # features: hfs + pqclean_kyber1024 (vendored fork, see memory_hygiene)
    - thiserror
    - if-addrs
    - spake2                                 # PAKE (pairing secret)
    - getrandom
    - sha2                                   # group key schedule
    - hkdf                                   # group key schedule
    - chacha20poly1305                       # group AEAD
    - zeroize                                # P1: wipe-on-drop for self-held secrets
    - ed25519-dalek                          # P2: one-time circle voice keys (fast + zeroize features)
  planned_deps_v3: []                        # v3 needs NO new crypto dep — seat flow uses existing ed25519
  lifecycle: single-startup-person           # INVARIANT: no role subcommands; one process is born with both ears and mouth
  birth_modes: "born (v1: visible, answers /shout — REMOVED in 3.0.0) | --no-chain (never offers/accepts a next secret — the open-desk shape) | --guard <machine-secret> (EVERY arrival must pass PAKE; arming NEVER consumed; armed open-desk = --guard KEY --no-chain) | --hidden (DEPRECATED alias, dies with shout in 3.0.0)"
  other_job: "--courier — the same binary as a teahouse: room numbers are GUEST-CHOSEN; wires same-room arrivals, relays opaque bytes; no identity, no storage, no plaintext"
  commands:                                  # in-session, parsed by CLI handle_command (src/main.rs)
    dial:  { syntax: "/dial ADDR:PORT [secret]", behavior: "outbound connect + hybrid NN handshake as initiator; optional secret → PAKE first; auto-uses chained secret for that addr (burned only when actually verified); 5s connect timeout" }
    meet:  { syntax: "/meet COURIER ROOM [secret]", behavior: "enter a teahouse room; roles assigned by courier (later arrival initiates); optional secret → PAKE inside the room; teahouse lines never chain" }
    punch: { syntax: "/punch INTRODUCER TAG [secret]", behavior: "register the same tag with the introducer; he exchanges public endpoints; both sides hole-punch; noise handshake runs over the punched udp line; best-effort — on failure the user re-routes via /meet MANUALLY" }
    await:  { syntax: "/await SECRET", behavior: "arm the password challenge for the NEXT inbound arrival (one-shot; spent only when PAKE masks are actually exchanged)" }
    talk:  { syntax: "/talk id|name-prefix", behavior: "switch input focus; ambiguous prefix lists candidates" }
    list:  { syntax: "/list", behavior: "enumerate live conversations, mark focused" }
    card:  { syntax: "/card", behavior: "print one 'name  ip:port' line per interface — SURVIVES removals (card only lists own addresses, touches no network)" }
    shout: { syntax: "/shout", behavior: "v1: udp-broadcast who-is-there on the lan — STILL IN CODE, REMOVED in 3.0.0 (removals_3_0_0)" }
    circle: { syntax: "/circle", behavior: "P2 as-built: form a flood circle over ALL current conversations (counting-based completion — see attack_precedents P-03); v3: becomes 开台 (genesis, roster-based — see group_v3) and RE-RUNNING it = 点名 roll call" }
    gsay: { syntax: "/gsay TEXT", behavior: "speak in the circle (signed speech 0x0A); v3 gains a circle-target argument for nested circles" }
    file:  { syntax: "/file PATH", behavior: "hand a file to the focused conversation: 0x06 head + 0x07 chunks; lossless; 32MB cap both ways; received files auto-save to ./received/" }
    bye:   { syntax: "/bye", behavior: "offer next secret (chainable lines only), then circle_bye (0x0C) if in a circle, then empty-plaintext goodbye to focused conversation" }
    quit:  { syntax: "/quit | Ctrl-D", behavior: "circle_bye to all circles (chainable lines offer secrets), goodbye to all conversations, exit 0" }
  planned_commands_v3:
    word: { syntax: "/word", behavior: "SAS 对词——1:1 聚焦时显示会话短词；在圈内时显示群短词（K 派生）。MITM 把聚首劈成两半 ⇒ 两半短词不同 ⇒ 人类对词即察觉。见 group_v3.sas" }
    bring: { syntax: "/bring NAME (拟名)", behavior: "领人入座：向聚焦会话对端发 0x10 看座，走 seat_flow。名字如何确认属于人类信道——协议只认署名。" }
  sessions: multiple-concurrent-1to1
  focus_model: "typed lines go to the focused conversation; focus set by /dial or /talk; incoming conversations never steal focus"
  symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node — extends to circles (ownerless)
  packaging: workspace-three-gates            # root pkg = rust lib `e2ee` + bin; ffi/ member = cdylib gate — see ffi block
  api_style: "library NEVER prints nor reads stdin nor touches the fs; actions in = metaphor-verb methods; experiences out = Event stream (tokio mpsc); Event/LeaveReason/PersonError are non_exhaustive; errors carry semantics, CLI renders prose"
  modules:
    src/lib.rs:     "crate root: pub mod circle + courier + discover + person + punch + wire; re-exports Person/Events/Event/LeaveReason/PersonError/RosterEntry/Discovered"
    src/person.rs:  "core person model; handshake<S> GENERIC over AsyncRead+AsyncWrite (tcp and UdpPunch alike); ear() dispatches on frame_kind; ZERO printing, ZERO stdin, ZERO fs. P2 adds: Gathering (voice keypair + members map + epoch), 0x09 self-contagion, 0x0A dual-gate verify (subkey decrypt + verify_strict), 0x0B apply_topup/issue_topup_rm, 0x0C unsigned bye, belief-free flood (all convos minus source — P-02 debt), counting completion (P-03 debt)"
    src/punch.rs:   "introducer client + punch transport: UdpPunch (frame-per-datagram, echo swallowing); INTRO2 source-checked"
    src/circle.rs:  "group key schedule: derive_group_key (HKDF over byte-sorted contributions, takes &[&[u8]] borrows), derive_sender_subkey, seal/open (ChaCha20Poly1305, nonce = counter ‖ sender-tag); unit tests for order-independence and subkey isolation"
    src/courier.rs: "teahouse: serve() wires same-room arrivals, splices raw bytes; serve_introducer on the same udp port; JOIN frame bounded 10s"
    src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; clean EOF vs mid-frame break"
    src/discover.rs: "v1: lan shout + card. 3.0.0: shout listener + shout() REMOVED; card() (own-interface listing) survives"
    src/main.rs:    "thin CLI renderer; safe() sanitizes ALL remote-controlled strings; harden_process() = RLIMIT_CORE=0 + swap warning (P7 adds PR_SET_DUMPABLE)"
    ffi/src/lib.rs: "cdylib gate: 12 exported e2ee_* symbols; per-handle tokio runtime"
    include/e2ee.h: "C header — MUST change together with ffi/src/lib.rs (frozen ABI)"
    tests/conversation.rs: "25 integration tests, real persons on ephemeral ports (4 new in P2: star topology, solo circle, voluntary-leave rekey, signature forgery rejection)"
    tests/pq.rs:       "hybrid suite round-trip"

# ───────────────────────────────────────────────────────────────
# CRYPTO (as-built)
# ───────────────────────────────────────────────────────────────

crypto:
  suite_no_secret: "Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s"       # hybrid X25519+Kyber1024
  suite_with_secret: "Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s" # PAKE output as PSK
  pake: "SPAKE2 (spake2 crate, Ed25519Group) — masks exchanged, secret never crosses the wire; wrong secret → loud failure; burn semantics: secret spent iff masks were exchanged; stable guard never consumed. STRUCTURAL NOTE: online-only guessing (each guess = a visible arrival event) — no backoff needed, no backoff wanted (theory.rulings)"
  static_keys: none                          # INVARIANT: ephemeral per-connection only
  identity_layer: none                       # INVARIANT: no auth / TOFU / trust store / accounts — circle voice keys are session-scoped attribution capability, NOT identity (A1)
  auth_model: telephone                      # secrecy by default; PAKE = optional per-meeting human-carried secret; SAS (v3) = optional human verification ritual
  pfs: implicit                              # fresh keys per connection ⇒ forward secrecy for free
  group_layer: "P2: flood circle as-built (see group_v2_as_built) → v3 axiom rebuild (see group_v3)"
  pq_hybrid: implemented                     # X25519 AND Kyber1024, both must break; NEVER pure-PQ
  voice_keys: "P2: per-gathering throwaway ed25519 keypair; sk lives in RAM only (SigningKey, zeroize feature), dies with the process; pk rides the 0x09 contribution; speech/bye/topup are signed with it (A1)"

memory_hygiene:                              # P1 as-built (unchanged by P2/v3)
  status: implemented
  threat_tiers:
    T0_same_machine_software: "root / kernel / ptrace / /proc/pid/mem — OUT OF SCOPE (telephone-model boundary, documented)"
    T1_secondary_persistence: "swap-to-disk, zram writeback, core dumps, hibernate images — OUR battlefield; fully closeable in software"
    T2_opportunistic_physical: "live DMA (thunderbolt/usb4), debug ports — OS/IOMMU domain; app can only shrink the exposure window"
    T3_physical_extraction: "cold boot (dram remanence), chip-off/JTAG reconfig — hardware domain; app-level unsolvable; shrink the window only"
  in_memory_encryption: "REJECTED — NEVER re-propose (settled_decisions.rejected). Circular argument: the key that decrypts must itself live in memory. Without a hardware anchor this is security theater (a costume, not a lock)."
  goal: exposure-minimization                 # minimize TIME WINDOW and PHYSICAL EXTENT of secrets in RAM, weld T1 shut
  implemented_layers:
    snow_fork: "vendored fork at vendor/snow (patch.crates-io) — SymmetricStateData h/ck wipe on drop + local HKDF/key copies wiped; HandshakeState.psks Zeroizing; Dh25519/P256 privkey Drop-wipe; Kyber1024 secret-key volatile wipe (fork relaxes upstream's unsafe_code=forbid). Cipher internals already wipe via upstream chain. Behavior bit-identical: snow's own 66-test suite ships in the fork copy and passes"
    own_secrets: "person.rs all self-held secrets wrapped Zeroizing: expecting/stable/chained/pending secrets, Gathering (my_r/contribs/key/voice sk), InboundFile.buf, Event::FileArrived.data, ear()/hello()/send_typed()/send_chat() plaintext frame buffers, pake() ignition key"
    cli_harden: "main.rs harden_process(): RLIMIT_CORE=0 (unix) + /proc/swaps unencrypted-partition warning. Library never touches process policy. P7 UPGRADE: RLIMIT_CORE does NOT bind piped core collectors (core_pattern starting with '|') — the STRUCTURAL fix is prctl(PR_SET_DUMPABLE, 0) (same class as ssh-agent); RLIMIT_CORE stays as belt-and-suspenders. See attack_precedents P-10."
    circle_borrow: "circle.rs derive_group_key takes &[&[u8]] (borrows, never moves contributions)"
  mlock_evaluation:
    verdict: deferred-with-reason
    why: "mlock operates at 4KB page granularity while our keys are 32B heap objects sharing pages with ordinary data; RLIMIT_MEMLOCK-bound (64KB default); Rust allocator gives no page-aligned key placement. zeroize + swap/coredump closure deliver the T1 story. Do NOT cargo-cult mlock back in."
  honest_boundary: "T2/T3 remain out of reach; kill -9 / power loss skips Drop (residue until reboot — inherent to zeroize-class defenses, documented); the promise stands as 'zero persistence + active memory hygiene'"
  coercion_stance: "physical-coercion defense is INHERENT — zero-persistence + wipe-on-drop + ephemeral keys compose into 'close the program and nothing remains'. No duress mode, no panic wipe, no hidden state to find. NEVER propose a duress/panic-wipe feature (settled_decisions.kept.coercion_defense_inherent)."

# ───────────────────────────────────────────────────────────────
# WIRE PROTOCOL (as-built + settled v3 additions)
# ───────────────────────────────────────────────────────────────

wire_protocol:
  transport: "tcp (or a punched udp line — same framing, one frame per datagram)"
  frame: "{u16_be length}{payload}"
  frame_cap: 65535
  handshake_msgs: 2                          # ~1.6KB each (Kyber1024); ≈3.2KB per handshake
  handshake_padding: "0..=255B random noise payload per handshake message — blurs the fixed hybrid fingerprint; receivers ignore; no negotiation"
  pake_msgs: 2                               # before noise iff secret present
  meet_timeouts: "every meeting stage bounded at 10s; courier JOIN likewise; accept loop never blocks"
  first_app_frame: "kind-tagged encrypted NAME frame (0x05) — names NEVER cross the wire in plaintext"
  app_frame_kinds:                           # first byte of every encrypted payload
    chat: "0x00 legacy bare text — still PARSED, no longer emitted"
    offer_secret: "0x01 + 32B next-secret"
    ack_secret: "0x02"
    contrib: "0x03 + 32B contribution — v1 circles; SUPERSEDED by 0x09"
    circle_chat: "0x04 + nonce ‖ group-AEAD — v1 circles; SUPERSEDED by 0x0A"
    name: "0x05 + utf8 name"
    file_head: "0x06 + name_len ‖ name ‖ total_bytes(u64) — >32MB declined loudly"
    file_chunk: "0x07 + seq(u32) ‖ data — 8–32KB randomized chunks"
    padded_chat: "0x08 + text_len ‖ text ‖ random pad — what send_chat emits; P7: bucketed padding (length-inference measurement pending, see P-11)"
    # ── circle frames, P2 as-built ──
    contrib_v2: "0x09 + circle_id(8B) ‖ size ‖ name ‖ r(32B) ‖ sig_pk(32B) — declared contribution. v3 ADDS contrib_sig(64B) over (circle_id‖name‖r‖sig_pk): the row becomes SELF-CERTIFIED (P-05 fix); without it the row is unsigned debt"
    circle_signed: "0x0A + circle_id(8B) ‖ sender_tag(4B) ‖ counter(8B) ‖ group-AEAD ‖ sig(64B) — signed circle speech, dual-gate (subkey decrypt + verify_strict)"
    topup: "0x0B — P2 as-built: circle_id ‖ epoch(u32) ‖ change-spec ‖ fresh_r(32B) ‖ initiator_sig(64B). v3 RESTRUCTURES (see group_v3.topup): epoch DIES, initiator election DIES, witness signs for RM, introducer signs for ADD"
    circle_bye: "0x0C + circle_id(8B) ‖ name — P2 as-built is UNSIGNED (forgeable, P-01). v3 ADDS bye_sig(64B) over (circle_id‖name)"
    # ── v3 frame kinds (settled design, NOT implemented — see group_v3) ──
    genesis: "0x0F + circle_id(8B) ‖ seats(u8: 0=closed 封桌 / 1=open 开台) ‖ count(u8) ‖ invited names ‖ creator_sig(64B) — 开台公告：署名的邀请名单=发起者感知快照（事实陈述，非权力凭证）。凑话完成条件=名单上的名字全部自署到场（计数死，P-03 fix）。点名=新 circle_id 的重新 0x0F，社会连续、密码学新鲜"
    seat_offer: "0x10 + circle_id(8B) ‖ seats ‖ member names+pk pairs (NO r values!) — 看座：1:1 帧（非圈帧），引荐人把席位骨架递给候选人。红线：绝不携带任何 r——拒入者拿到 r 即可推导群钥（P-09）"
    seat_accept: "0x11 + name ‖ r(32B) ‖ sig_pk(32B) ‖ contrib_sig(64B) — 落座署名：候选人自选 r、自签整行，经 1:1 递回引荐人。r 自选（128bit+ 碰撞概率可忽略）+ 收端唯一性响亮检查（P-12 fix）"
    seat_roster: "0x12 + FULL member table (each row: name ‖ r ‖ sig_pk ‖ contrib_sig) — 座次表：1:1 帧，ADD 应用后引荐人把全表递给新人（新人由此持全部 r 计算群钥）。只递给已成成员的人"
    # 0x0D/0x0E 曾预留给见证/分叉通告——死于理论重建（同步机整体退役）；不得复活
  unknown_kinds: "silently skipped — forward compatibility (this is what makes the v3 additions rollout-safe for old peers)"
  hangup: "empty plaintext frame | tcp close"
  teahouse_control:                          # plaintext frames on the courier line, before the splice
    join: "'JOIN <room>' — first frame from each arrival; rooms guest-chosen; same-number collisions = cross-wired pairs"
    paired: "'PAIRED R' to the first arrival, 'PAIRED I' to the second — roles for the noise handshake"
  introducer_and_punch:
    introducer: "courier::serve_introducer — udp side of the teahouse port; INTRO1 tag / INTRO2 R|I peer; one-shot tags, ttl 90s, MAX_WAITING_TAGS 1024. P7: tag entropy MUST be 128-bit random (structural unguessability, replaces any backoff idea)"
    punch: "register w/ retry (3s), connect to peer's public endpoint, exchange PUNCH frames (~150ms apart, 8s window); INTRO2 accepted ONLY from the introducer's own address"
    transport: "UdpPunch: one frame per datagram, punch-echo discarded, max frame 65507B; no liveness detection — a vanished peer hangs the line until /quit"
    fallback: "on punch failure the USER re-routes via /meet manually — no automatic fallback"
  lan_discovery:                             # v1 as-built — ENTIRE SUBTREE REMOVED in 3.0.0 (removals_3_0_0)
    status: still-in-code-removal-pending
    v1_behavior: "udp 37777; E2EEPROBE1 broadcast probe, E2EEREPLY1 name+port reply; hidden mode = listener never spawned"

# ───────────────────────────────────────────────────────────────
# FILE TRANSFER (as-built, unchanged by v3)
# ───────────────────────────────────────────────────────────────

file_transfer:
  status: implemented-v1
  capability: "lossless binary handover on ANY conversation path (direct / teahouse / punched line)"
  lossless: "from_utf8_lossy NEVER applies to file payloads; receiver asserts byte-equality in tests incl. invalid-UTF-8"
  chunking: "randomized 8–32KB; receiver size-agnostic; no negotiation"
  cap: "MAX_FILE_BYTES = 32MB, enforced symmetrically"
  in_flight: "ONE transfer per conversation per direction; stalled half-file never blocks the next"
  robustness: "seq must match next_seq exactly; mismatch voids the transfer silently"
  zero_persistence: "reassembly in RAM only; the LIBRARY never touches the fs; bytes surface as Event::FileArrived (Zeroizing)"
  memory_note: "the 32MB RAM residency is a T1 exposure — documented, accepted"
  ffi_status: "FileArrived/FileDeclined map to tag 0 in e2ee_poll; an e2ee file API is the natural add-only extension"

# ───────────────────────────────────────────────────────────────
# GROUP — P2 as-built (诚实债务版：代码是什么就写什么)
# ───────────────────────────────────────────────────────────────

group_v2_as_built:
  status: implemented-P2-2026-10-14          # flood+signing+topup+solo landed; SUPERSEDED by group_v3 (axiom rebuild)
  one_line: "connected-is-enough circles; contributions flood and self-declare; speech signed by one-time keys; membership change rekeys in ONE top-up frame"
  what_shipped:
    flood: "0x09 arrives on ANY line → build Gathering on first arrival → flood own contrib (self-contagion); dedup by r_i"
    speech: "0x0A dual-gate: subkey-decrypt THEN verify_strict against the pk from the gathering registry; CircleBadSignature on failure"
    topup: "0x0B RM path: smallest-r initiator NAMED in frame, epoch serialization, initiator swaps own r to fresh_r; apply_topup re-derives K"
    bye: "0x0C unsigned, triggers topup RM on receipt + dead-line observation"
    solo: "empty-convos guard dropped — a circle of one is a channel"
    admission_by_counting: "form_circle: expected = convos.len() + 1 contributions — COUNT not ROSTER (P-03 debt)"
    forwarding_by_convos: "flood_frame excludes ONLY the source convo — ALL other conversations receive circle frames (P-02 debt)"
  honest_debt: "every mechanism listed in attack_precedents (P-01..P-08) is LIVE in this as-built. P2 is a faithful implementation of the pre-theory design; v3 exists because the pre-theory design itself was wrong (A2 violation), not because P2 implemented it badly."
  test_status: "31 total: 25 integration (incl. 4 new v2: star topology, solo circle, voluntary-leave rekey, signature forgery rejection) + 5 unit + 1 pq — all green; FFI 12 symbols + contract + ctypes green"

# ───────────────────────────────────────────────────────────────
# GROUP v3 — 公理重建（settled design 2026-10-15, not yet implemented）
# 这是 theory 块的实例化。一切机制可回溯公理；一切裁决见 theory.rulings。
# ───────────────────────────────────────────────────────────────

group_v3:
  status: settled-design-not-implemented     # implement per pending_work.P3 (core) + P4 (removal face); P6 nesting after
  one_line: >-
    圈不是对象，是 N 个局部信念的对齐。协议的职责不是维持一致（那是同步机，
    违反 A2），而是制造四种感知：听见（AEAD）、嗓音（签名）、在场（直连）、
    见证（死线报告）。分歧不可预防但必被听见（T1），修复永远可得（点名，A4）。

  the_circle_object:                         # 名单（member table）是圈的唯一状态
    roster_row: "name ‖ r(32B) ‖ sig_pk(32B) ‖ contrib_sig(64B over circle_id‖name‖r‖sig_pk) — 每行自签（A1：身体的印），任何人伪造不了任何一行"
    roster_is_belief: "每个成员的名单是她的局部信念（A2）——没有权威副本；名单即路由表即世界观"
    scaling_note: "~50 members ≈ 3.5KB/roster — negligible"

  genesis:                                   # 开台（0x0F）
    mechanism: "/circle 扫描发起者当前会话 → 署名邀请名单（名字）+ seats 标志 → 0x0F 走一跳（名单=发起者直连对端，天然 direct-lock）"
    completion: "名单上的名字全部自署到场（0x09+contrib_sig）——不是计数（P-03 fix）。陌生人注入的贡献名字不在表上 → 拒收 + 响亮事件"
    seats_default: "多人成圈默认封桌 closed（私人圆桌）；一人成圈默认开台 open（舞台，等人来的宣言）。/circle 可显式覆盖"
    genesis_signature_ruling: "0x0F 的署名是收据不是权力——创世快照只发生一次，内容是事实（我设桌时谁在座）；之后的一切无主（踢人要见证+在场自辩，封闭桌无人能单方面开）。见 theory.rulings"

  speech:                                    # 发言（0x0A）——P2 机制不变
    dual_gate: "subkey-decrypt + verify_strict — 嗓音完整性（T3）"
    counter_semantics: "per-sender 单调 counter，跨聚首不重置；去重=最高水位+有界乱序窗口（规约定死，不留实现自由度）；防重放由 counter 承担（epoch 死后这是唯一时间性来源，A1）"

  routing:                                   # 信念路由——转发按名单，不按会话集
    rule: "圈帧只走『对方名字在我当前信念名单上』的线 − 来源线 − 除名者的线"
    kills: "P-02（圈外旁听：成员的圈外 1:1 线再也收不到任何圈帧，连 circle_id 都看不见）；P-04 补话断递（除名者连 topup 都收不到，fresh_r 无从获取）"
    topology_constraint: "连通性收紧为『成员导出子图连通』（创世时发起者的星形天然满足；星心离场=断连=点名重建——现状同样脆，现在写成显式约束）"
    carrier_visibility: "茶馆转发的是密文，但 circle_id 是明文（路由需要）——载体知道圈的存在/流量/拓扑。这是桌子的物理（不对称表第 4 行），诚实入档，不修"

  rekey:                                     # 换气——点名为真理，补话为捷径
    roll_call:                               # 点名（A4：唯一的真理路径）
      mechanism: "任何成员任何时候发起 = 新 circle_id 的重新 0x0F（与创世完全同构，代码路径复用）。同时是离开修复、分裂修复、怀疑修复、纯换钥——一个机制覆盖全部"
      social_continuity: "密码学上是新圈；社会上是同一张桌重新点名"
      trigger: "仅手动（再敲 /circle）。协议绝不自动建议点名——『建议』本身就是一个检测器，违反『协议不监视，人类做验证』（A2）。机器对已听见的失败（mumble）的响应仅限响亮呈现"
    topup:                                   # 补话（快速路径——人类也有捷径：有人离席，剩下的人不会重新自我介绍）
      rm_layout: "0x0B + circle_id ‖ 'RM' ‖ leaver_name ‖ fresh_r(32B) ‖ witness_sig(64B)"
      rm_witness: "见证人=死线持有者（A2：离席只对死线持有者可观察），不再是 smallest-r 选举人——她自己供应 fresh_r 并署名。最小 r 占位攻击（P-06 前身）随选举机制整体死亡"
      rm_presence_defense: "在场辩护：RM 点名者的近期已验签发言（counter 水位内）与 RM 矛盾 → 扣下 topup + 响亮冲突 + 人类裁决。『还在说话即不可被判离开』"
      rm_false_witness_honesty: "沉睡成员可被伪证 RM（见证人撒谎『我的线死了』）→ 临时驱逐，成员回来发言即 mumble → 点名回归。物理等价：你离席接电话时有人散布你走了——桌子重排，你回来发现桌变了。响亮、可恢复、入 known_limitations"
      add_layout: "0x0B + circle_id ‖ 'ADD' ‖ newcomer_row(name‖r‖sig_pk‖contrib_sig) ‖ introducer_sig(64B)"
      add_no_fresh_r: "ADD 不需要 fresh_r——没有离开者需要遗忘；新人自选的 r 就是新熵（K_new 含它），旧窃听者无从推导"
      add_semantics_ruling: "递话不代笔（settled 2026-10-15）：引荐人递的是新人自署的整行，伪造不了任何字段；她的签名只是引荐收据（问责：出问题时『谁带来的人』有答案）"
      exchange_algebra: "不相交变更序无关（T2：集合加减排代换天然可交换）；一切不可交换情形（同人二次变更、见证冲突、部分投递）必然退化为 mumble——响亮，交给点名。级联定时器、发起人选举、validity 规则：整体删除，不是换更好的，是不需要"
      pure_rekey: "无纯换钥 topup——密钥保鲜=点名（谁、何时、为什么换，是全桌的事，不是一个人的事）"
    transitional_window: "RM/ADD 应用不一致期间，滞后成员仍在旧钥下发言；刚拿到名单者可解滞后发言——物理等价：你走向桌子时有人正把话说完，你听见了尾巴。入档为已知边界，不修"

  admission:                                 # 入席（seat flow：0x10 → 0x11 → 0x0B ADD → 0x12）
    step_1_offer: "引荐人向候选人（1:1）发 0x10 看座：席位骨架（名字+pk，绝无 r——拒入者拿 r 即可算 K，P-09）"
    step_2_accept: "候选人自选 r（128bit+，碰撞可忽略）、生成嗓音钥、自签整行（0x11 落座署名）递回"
    step_3_add: "引荐人泛洪 0x0B ADD（新人自署行 + 引荐收据）。封闭桌：本地拒收 + 响亮（席位契约，不是表决）；开放台：任何成员的 ADD 有效（引荐即背书）"
    step_4_roster: "ADD 应用后，引荐人向新人（1:1）递 0x12 座次表（全表含 r）——新人由此计算群钥。座次表只递成员"
    r_collision: "收端唯一性检查：r 与现有任何成员碰撞 → 响亮拒绝（P-12：r 碰撞若静默去重=无声驱逐；自选 r 下碰撞概率≈0，恶意碰撞被响亮拦截）"
    name_conflict: "同名到达 → 不再 first-seen-wins（那是抢注，A1+A2 违反）：先到暂定、直接对端的见证帧裁决、冲突响亮。见 theory.rulings"
    knows_vs_receipt: "知道了就是同意——签名是这张同意的收据。引荐发生在人类信道（你怎么知道这个圈的，是你的事）；入座发生在协议信道（谁签你进来的，是全桌的事）。系统遗忘、人类记忆——这次的记忆是问责线索，rekey 后连收据都忘掉"

  sas:                                       # 对词（settled 2026-10-15：群 SAS + 1:1 SAS）
    what: "/word——1:1 聚焦时显示会话短词（Noise transcript hash 派生，双方各看各的屏幕）；在圈内时显示群短词（HKDF(K, \"circle-sas\") 派生）"
    why: "SAS 不是检测，是口头核对的密码学蒸馏（不对称表第 2 行）。MITM 把聚首劈成两半 ⇒ 两个 K ⇒ 两半短词不同 ⇒ 想核对的人对一遍词即察觉。无身份层、零持久化、可选仪式——完全活在 telephone model 里"
    theorem_prerequisite: "T5：无 SAS 则『抗窃听者机密性』无法陈述（MITM 拼两条线自投贡献直接入圈持钥）。SAS 存在，定理才有前提"
    no_persistence: "短词即时显示、永不落盘；不对账也没关系——它和暗号一样，是给愿意谨慎的人的工具"

  nested_circles:                            # 堂与桌（P6，v3 之后）——纯簿记，无新密码学
    slots: "Inner grows TWO named gathering slots (hall 堂 + table 桌); each has its own 8B circle_id; all v3 mechanisms apply unchanged to each"
    line_reuse: "the SAME noise line carries frames of both circles (kinds disambiguate) — zero extra link cost; belief routing applies per-circle"
    compartmentalization: "one table compromised ⇒ that table's key leaks with it; hall unaffected (a member of both still holds hall key — inherent); other tables unaffected"
    carrier_policy: "非本圈成员的线是走廊：堂帧经非成员载体照常放行（成员子图连通的投递路径）；桌的存在/频率对堂内可见=剧场物理（大厅里本来就看得见桌子），入档不修"
    honesty: "serverless nested circles have no known prior art (closest: draft-ietf-mls partial fanout) — experimental label is EARNED, keep it"

  ownerless:                                 # 无主（settled social contract）
    rule: "NO circle owner, NO admin role, NO kick command — symmetry forbids it; a one-voice hall is UNCONSTRUCTIBLE"
    ejecting_a_jerk: "everyone drops their lines to him + one topup — collective action, no privilege"
    starting_over: "open a new circle, tell wanted members over 1:1 — the address book lives outside the system, as always"

# ───────────────────────────────────────────────────────────────
# REMOVALS 3.0.0 — 删除面（含原 2.0.0 未执行项；settled 2026-10-14/15）
# ───────────────────────────────────────────────────────────────

removals_3_0_0:
  version_window: "HEAD is post-P2 unreleased; the next release IS 3.0.0 (breaking: 0x09/0x0B/0x0C layout changes) — no 2.0.0 release is cut"
  v1_discovery_family:                       # 原 removals_2_0_0，至今未执行，并入 3.0.0
    shout: "udp lan discovery (probe/reply + listener + /shout) — was the ONLY unsolicited exposure source; circles absorb audience-finding"
    hidden_mode: "born_hidden / --hidden / e2ee_person_create_hidden — existed only to dodge shout probes; ffi symbol stays as deprecated alias (ABI frozen add-only)"
    lan_broadcast: "the loudspeaker /say design — cancelled before birth (settled_decisions.rejected)"
    listen_toggle: "the deaf/undeaf switch — evaporates with the loudspeaker"
  sync_machinery:                            # v3 死刑——理论裁决（A2），不是实现取舍
    epoch_field: "epoch(u32) in 0x0B — 目录的残留；防重放由署名 counter 承担；或降级为纯显示代际标签"
    initiator_election: "smallest-r initiator + 3s cascade + 'initiator named in frame' — 选举机制整体退役；见证人取代"
    validity_rules: "first-arriver-wins / same-leaver-no-op / serialize-by-arrival — 全部退役；T2 交换代数 + T1 响亮分歧取代"
    fork_announcement_frame: "0x0E 分叉通告（曾计划的 prev-hash 链）——死于重建：分叉不预防、不通告，分叉必被听见（mumble），修复必可得（点名）"
    counting_admission: "expected = convos.len()+1 — 计数死；名单上的名字到齐才是完成（P-03）"
    unsigned_bye: "0x0C 无签名形态死——告别是身体的行为（A1），必须署名（P-01）"
    all_convos_flooding: "flood to all convos minus source — 信念路由取代（P-02/P-04）"
  ffi_compatibility: "e2ee_person_create_hidden stays as deprecated alias; circle v3 events extend the enum add-only; 0x09/0x0B/0x0C layout changes are wire-breaking = exactly why this is 3.0.0"

# ───────────────────────────────────────────────────────────────
# INVARIANTS — any change violating these MUST be rejected
# 公理是宪法（theory 块）；本节是宪法+工程不变式的合并执行清单。
# ───────────────────────────────────────────────────────────────

invariants:
  - "AXIOM SUPREME: 具身（A1）——一切有后果的行为必须用本人的一次性嗓音钥署名：说话、告别、见证、引荐、创世公告。无签名=哑巴，拒收。"
  - "AXIOM SUPREME: 局部性（A2）——不存在全局名册/epoch 共识/官方成员表/任何同步机。名单是信念，路由表=世界观。分歧是常态，响亮是义务。"
  - "AXIOM SUPREME: 不可撤回（A3）——换气只封存未来；旧钥明文是历史不是泄漏；PCS 缺口是物理的价，不假装修掉了。"
  - "AXIOM SUPREME: 可重启（A4）——点名与创世同构、永远可用；任何损坏的修复路径必须存在且是这一个。"
  - "zero-persistence: no fs writes ever — no logs, config, cache, keys, history, state files"
  - "plaintext-containment: plaintext allowed only in stdin-buffer, stdout-write, RAM; never in errors/args/env; on the wire, names ride inside noise frames"
  - "e2ee-only-transit: TALK crosses the wire exclusively inside noise transport frames (teahouse JOIN/PAIRED control-plane is plaintext by design and always was)"
  - "no-identity-layer: never add authentication, TOFU, key continuity, accounts, fingerprint checking. Voice keys are session-scoped attribution capability, NOT identity; they never persist, never chain across circles, never accumulate trust"
  - "symmetry: never introduce server/client asymmetry or a node with more power than a peer. COROLLARY — circles are OWNERLESS; no admin role, no kick privilege"
  - "ephemeral-keys: every connection = fresh keypair; zero key reuse across anything"
  - "seat_offer-carries-no-r: 0x10 看座绝不携带任何 r 值——拒入者拿到 r 即可推导群钥（P-09）；座次表 0x12 只递已成成员"
  - "secret-chain-red-line: NEVER persist a rolling secret to disk"
  - "loud-failure: 一切失败响亮化——mumble、事件、响亮拒绝。静默裁决不存在（decision_procedure step_5）"
  - "no-auto-suggestion: 协议不监视、不建议——点名仅手动触发；机器只响亮呈现已听见的失败，不主动诊断"

# ───────────────────────────────────────────────────────────────
# SETTLED DECISIONS — owner rulings. HIGHEST authority beside
# axioms; re-proposing a rejected item = reject
# ───────────────────────────────────────────────────────────────

settled_decisions:
  kept:
    axiom_rebuild: "2026-10-15 推倒同步层、按四公理重建（v3）——『我们宁可推翻重来，但理论不能变：基于人类社会物理交流的抽象』。epoch/initiator/validity 全家退役是公理裁决"
    conversation_physics: "四公理+不对称表+推导表 = 项目宪法；编码模型 decision_procedure 逐条过"
    member_table_is_the_circle: "名单（每行自署）是圈的唯一状态；创世=署名邀请名单；信念路由；点名=新 circle_id 重聚"
    roll_call_truth_path: "点名为唯一真理路径（A4）；补话为见证人捷径；无纯换钥 topup；点名仅手动触发（no-auto-suggestion）"
    sas_both_levels: "2026-10-15：群 SAS（/word 群短词）+ 1:1 SAS（会话短词）——T5 定理前提；无身份、零持久化、可选仪式"
    seats_default: "2026-10-15：多人圈默认封桌；一人圈默认开台（舞台）；/circle 可覆盖"
    add_relay_not_ghostwrite: "2026-10-15：递话不代笔——ADD 携带新人自选 r 的自署行；pk 由新人自己签；引荐收据=问责线"
    knows_means_receipt: "『知道了就是同意——签名是这张同意的收据』：引荐在人类信道，入座在协议信道，rekey 后收据也遗忘"
    relay_circle: "flood circles are THE answer to >8 groups — connected-is-enough; the key schedule is kept unchanged"
    leave_rekeys: "departures MUST rekey — witness topup, not full re-gathering"
    one_time_signing: "ed25519 one-time voice keys — approved 2026-10-13; the only acceptable attribution under flooding"
    compromised_device: "a seized device that stays in the circle keeps reading — INHERENT to group keys; the coercion answer is inherent: close the program and nothing remains"
    coercion_defense_inherent: "owner ruling 2026-10-14: zero-persistence + wipe-on-drop + ephemeral keys compose into 'the user merely closes the program'; NEVER propose a duress/panic-wipe feature"
    small_circle_markers: "relayed-contribution markers unnecessary in small circles — 'people naturally know'"
    nested_circles: "halls-and-tables (大圈套小圈) — pure nesting, no speaker variant (P6)"
    solo_circle: "a circle of one is allowed — circles absorb the entire 'broadcast/audience' concept (open stage)"
    ownerless: "'there was never an owner — speech among equals is how reality works'"
    android_abi: "prebuilt libraries for all android ABIs (pending_work.P5)"
  rejected:                                  # do NOT re-propose — 永久性
    in_memory_encryption: "encrypting memory with a key that must itself live in memory is circular; a costume, not a lock"
    mls_adoption: "MLS/TreeKEM — credential framework collides with no-identity-layer; dependency weight breaks single-binary"
    sync_layer_repairs: "对同步机的任何修补（prev-hash 链、0x0E 分叉通告、更好的 validity 规则、更聪明的选举）——同步机本身违反 A2，修它=保留它"
    first_seen_wins: "先到先得占有名字=抢注，A1+A2 违反；取代：先到暂定+见证裁决+冲突响亮"
    epoch_field: "全局时钟序列的残留；防重放归 counter"
    pake_backoff: "PAKE 猜错退避=检测式编码；SPAKE2 结构上在线限定+响亮失败，人类感知即检测（theory.rulings 2026_10_15_no_backoff_for_pake）"
    loudspeaker_broadcast: "the /say loudspeaker — who broadcasts? 'only one voice' unenforceable; solo circles absorb the use case"
    lan_discovery_family: "shout + hidden + lan broadcast removed — redundant; removes the only unsolicited exposure source"
    random_backoff_recircle: "random-delay full re-gathering on leave — replaced by witness topup + roll call"
    listen_switch: "the 'hear broadcasts?' toggle — evaporated with the loudspeaker"
    per_listener_choice_in_rooms: "'paired rooms hearing the hall' sub-options — mooted"
    auto_suggest_roll_call: "检测异常 mumble 密度后建议点名——『建议』是检测器，违反 no-auto-suggestion；点名永远是人类的手势"

# ───────────────────────────────────────────────────────────────
# ATTACK PRECEDENTS — 判例库（已裁决攻击；新特性必须逐条过堂）
# 格式：攻击 → 根因（公理违反）→ 修复（P3 slice）→ 必写测试
# ───────────────────────────────────────────────────────────────

attack_precedents:
  P_01_forgeable_bye:
    attack: "任何成员在任何线上伪造 0x0C『Alice 告退』→ 触发 topup RM Alice = 无特权踢人；全系统唯一不可归因的有后果行为"
    root_cause: "A1 违反——告别是身体的行为，却无签名"
    fix: "0x0C + bye_sig(64B)（P3 slice-2）"
    test: "伪造他人 bye → CircleBadSignature 响亮拒绝；自己的 bye → 全员触发换气"
  P_02_out_of_circle_eavesdrop:
    attack: "成员 M 与圈外人 S 有普通 1:1；圈帧按会话集泛洪到达 S 的线；0x09 明文携带全部 r_i → S 收齐即 K = HKDF(sorted r) 整圈旁听（读权限全拿，写权限没有）"
    root_cause: "A2 违反——转发不问信念名单；『秘密在场』这个物理世界不存在的状态成为合法状态"
    fix: "信念路由（P3 slice-1）：圈帧只走名单上的线"
    test: "成员与圈外人的 1:1 线上抓包/断言：零圈帧；旁听者持全部截获帧无法解密 0x0A（无 roster 无法派生子钥路径断言）"
  P_03_counting_admission_injection:
    attack: "凑话窗口内陌生人注入 0x09 → 计数 +1 → 直接入圈，读写全拿，无需引荐"
    root_cause: "A2 违反——邀请名单只决定数量不决定名字"
    fix: "0x0F 名单 + 名字到齐完成（P3 slice-1）"
    test: "名单外名字的贡献 → 响亮拒收；名单全员到齐 → 铸钥成功"
  P_04_introducer_ghostwrite:
    attack: "ADD 携带引荐人提供的 pk → 引荐人从此可冒充被引入者发言，直到 re-gather，被冒充者无自证手段"
    root_cause: "A1 违反——代笔；归因键由他人选"
    fix: "递话不代笔：ADD 携带新人自署行（P3 slice-4）"
    test: "引荐人伪造 (name, r, pk) 行 → contrib_sig 验证失败响亮"
  P_05_name_squatting:
    attack: "真 Alice 贡献到达前，攻击者注入 (name=Alice, pk=攻击者) → first-seen-wins → 攻击者署名的话被全圈当 Alice 的"
    root_cause: "A1+A2 违反——先到先得占有名字是抢注"
    fix: "0x09 行自签 + 同名见证裁决（P3 slice-1/4）"
    test: "抢注尝试 → 响亮冲突事件，不静默成功"
  P_06_split_brain:
    attack: "恶意成员把 T1(RM Bob) 只发给半圈、T2(RM Carol) 只发给另外半圈 → 两子集各自内部一致、互相永久 mumble（epoch 分裂脑，选择性注入天然可行）"
    root_cause: "同步机整体违反 A2——无全序网络里 serialize-by-arrival = 以到达序为隐式全序"
    fix: "删同步机；T2 交换代数（不相交变更序无关）+ T1（冲突必 mumble）+ 点名（P3 slice-2/3）"
    test: "不相交双 topup 任意顺序 → 终态相同（交换性断言）；同名冲突双 topup → mumble 不静默分裂"
  P_07_newcomer_no_bootstrap:
    attack: "(非攻击，规约缺口) ADD 半落地：收端存在，发端无路径；且无任何机制把成员表递给新人——她入座了却既听不懂（无 r）也验不了（无 pk 表）"
    root_cause: "名单不是协议对象——四个洞（旁听/计数/分叉/引导）同一缺失物的四张面孔"
    fix: "seat flow 0x10/0x11/0x12（P3 slice-4）"
    test: "新人走完整 seat flow → 解密+验签通过；跳步 → 响亮失败"
  P_08_carrier_mitm:
    attack: "茶馆拼接两条线对两端各做一次裸 NN——完美位置 MITM，无 TOFU/指纹/SAS 则不可检测；聚首被劈两半，攻击者自投贡献入圈持钥"
    root_cause: "不对称表第 2 行——声音可模仿/线路可拼接，人类核对仪式缺位"
    fix: "SAS 双层（P3 slice-5）：两半 K 不同 → 短词不同 → 人类对词即察觉"
    test: "MITM 拼接的聚首 → 两端 /word 显示不同短词（确定性断言）"
  P_09_declining_candidate_leak:
    attack: "看座帧若携带 r 值 → 拒绝入座的候选人持全部 r → 永久旁听（直到下次换气）"
    root_cause: "名单传递未分层——骨架（可公开给候选人）与全表（仅成员）未区分"
    fix: "0x10 骨架无 r；0x12 全表仅递成员（P3 slice-4；invariants seat_offer-carries-no-r）"
    test: "拒入者持骨架+全部截获圈帧 → 无法推导 K"
  P_10_core_pattern_pipe:
    attack: "core_pattern 为管道（apport/systemd-coredump）时内核不检查 RLIMIT_CORE → T1 故事在多数现代发行版上有洞"
    root_cause: "RLIMIT_CORE 只约束文件落盘路径——检测式补丁挡不住结构性收集器"
    fix: "prctl(PR_SET_DUMPABLE, 0)——结构解：core 从一开始就不会被产生（P7；ssh-agent 同款手段）"
    test: "文档化验证：管道 core_pattern 下进程不可 dump（/proc/self/auxv 或 prctl 回读）"
  P_11_length_inference:
    attack: "0..N 均匀 pad 对『文本长度→帧长度』的混淆取决于 pad 熵对文本熵之比；中文 UTF-8 文本长度方差极大，小 pad 形同虚设"
    root_cause: "填充分布未按结构规约"
    fix: "分桶填充（指数桶/2^n 对齐）——填充长度是内容的结构函数，不是对观察的响应（P7；论文二测量对象）"
    test: "长度推断测量实验（对论文二复用）"
  P_12_r_collision_silent_eviction:
    attack: "若 r 由引荐人代选，可故意选与现有成员碰撞的 r → 凑话按 r 去重 → 新人贡献被当重复静默丢弃 = 无声驱逐"
    root_cause: "A1——他人代选身份材料"
    fix: "r 永远自选 + 收端唯一性响亮检查（P3 slice-4）"
    test: "构造 r 碰撞的 ADD → 响亮拒绝，不静默"

# ───────────────────────────────────────────────────────────────
# KNOWN LIMITATIONS — honest edges; do NOT paper over in docs
# ───────────────────────────────────────────────────────────────

known_limitations:
  irreducible:                               # 四条不可消除边界——物理的价，哲学的价，写进论文 limitations 是诚实加分
    fork_can_be_triggered: "分叉可被引发（A2：泛洪无全序的价）——但必被听见（T1）、必可修复（点名）"
    single_link_framing: "单链成员可被栽赃/锁出（A2：在场的边界=你的直连）——与 rm_false_witness_honesty 同根"
    carrier_metadata: "载体可见元数据（circle_id/流量/拓扑）——桌子的物理，茶馆继承"
    no_sas_no_theorem: "无 SAS 仪式则机密性定理无法陈述（T5）——对词是给愿意谨慎的人的工具，不对账是用户的自由"
  sleeping_member_false_rm: "沉睡成员可被伪证 RM 临时驱逐；回来发言即 mumble → 点名回归；入档不修（群钥固有+物理等价已论证）"
  transitional_window: "RM/ADD 应用不一致期间滞后发言可被刚拿到名单者解密——走向桌子时听见话尾，物理等价，入档"
  no_liveness_probes: "established conversations have no keepalive — a vanished peer hangs the line until /quit; under v3 a zombie member's contribution stays in the key until witness topup / roll call"
  seized_device_reads_on: "a seized in-circle device reads until its lines die + topup lands; old plaintext remains readable to old key holders (history, not a leak); the coercion answer is inherent: close the program"
  pcs_gap: "PCS（后妥协安全）缺口：被缴设备读过的不追索（A3：物理世界不存在『让他忘掉已听见的』）；MLS 有 PCS 我们没有——无持久化+无身份的价格，正面声明"
  identity_bounded_by_direct_links: "contribution name-binding under flooding is first-seen-tentative + witness-adjudicated — secrecy is mathematical, member-identity certainty is bounded by direct connections"
  zram_writeback_residue: "android mmd zram writeback lands idle pages on /data — keys are wipe-on-drop so the residue window ends at process exit; the 32MB file buffer stays an accepted, documented residue"
  cross_version_chat_one_way: "peers predating a frame kind silently drop it; v3 kinds 0x0F-0x12 inherit the same one-way property"
  nested_no_prior_art: "serverless nested circles have no known prior art — experimental label stays until hardened"

# ───────────────────────────────────────────────────────────────
# PENDING WORK — settled board, ordered; P5 parallelizable anytime
# ───────────────────────────────────────────────────────────────

pending_work:
  P3_v3_axiom_rebuild:                       # 核心：理论落地。slice 顺序=依赖顺序
    status: not-started
    depends: []
    slices:
      - "slice-1 名单即圈：roster 行结构（name‖r‖pk‖contrib_sig）+ 0x0F 创世（署名邀请名单+seats）+ 名字到齐完成 + 信念路由（ Kills P-02/P-03/P-05 主体）"
      - "slice-2 告别与补话：署名 0x0C + 见证人 RM topup（无 epoch/无选举/无级联）+ 在场辩护（近期验签发言 vs RM 冲突→扣下+响亮）（Kills P-01/P-06 主体）"
      - "slice-3 点名：/circle 重敲=新 circle_id 重聚（与创世同构复用）；mumble 响亮呈现；无自动建议（Kills P-06 修复路径）"
      - "slice-4 入席流：0x10 看座（骨架无 r）/ 0x11 落座署名 / 0x0B ADD（自署行+引荐收据）/ 0x12 座次表（全表仅递成员）+ r 唯一性响亮检查 + 同名见证裁决（Kills P-04/P-05/P-07/P-09/P-12）"
      - "slice-5 对词：/word 1:1 会话短词 + 群短词（K 派生）；即时显示零落盘（Kills P-08 的可检测性）"
      - "slice-6 判例测试矩阵：attack_precedents 每条的 test 字段全部落为集成测试 + 交换代数断言（不相交变更序无关）+ 50 人规模 + 点名恢复全场景"
  P4_removal_face_3_0_0:
    status: not-started
    depends: [P3_v3_axiom_rebuild]
    scope: "删除 shout/hidden/lan listener（v1 家族）+ 同步机全部残骸（epoch/initiator/validity/cascade/计数完成/无签名 0x0C/全对话泛洪）；ffi create_hidden → deprecated alias；README.md/README_ZH.md 按 v3 世界重写；版本 3.0.0"
  P5_android_abi:
    status: not-started
    scope: "cargo-ndk + NDK r28+ (16KB page alignment — verify LOAD p_align via llvm-readelf); targets aarch64/armv7/x86_64/i686-linux-android; api 24; verify 12 e2ee_* symbols per .so; GitHub Actions job + release artifacts; pure-rust deps only"
    parallelizable: true                     # no dependency on P3/P4
  P6_nested_circles:
    status: not-started
    depends: [P3_v3_axiom_rebuild]
    scope: "hall+table slots in Inner; per-circle circle_id + belief routing; line reuse; /gsay target argument; compartmentalization tests; carrier_policy per group_v3.nested_circles"
  P7_hardening:
    status: not-started
    depends: []
    parallelizable: true
    scope: "PR_SET_DUMPABLE(0)（P-10，结构解）；punch tag 128-bit 熵（结构性不可猜）；padded_chat 分桶填充（P-11）；counter 去重=最高水位+有界乱序窗口（规约定死）"

  research_track:                            # 非代码轨——论文工作，不阻塞任何 P 项；细节另档
    note: "T1-T5 Tamarin 形式化（P3 落地后建模）；取证测量论文（Signal/Telegram/Briar/本系统 本地残留对比）；trilemma 以猜想形态入论文一。README 不承载研究细节。"

# ───────────────────────────────────────────────────────────────
# GLOSSARY — zh metaphor → mechanism
# ───────────────────────────────────────────────────────────────

glossary:
  耳朵:      "inbound tcp accept loop (person::born acceptor; one ear task per conversation)"
  出生:      "Person::born — bind + spawn acceptor, returns (Person, Events)"
  嘴:        "outbound tcp connect (dial)"
  见面:      "noise nn handshake"
  密谈:      "noise transport phase"
  挂断:      "session teardown"
  道别:      "empty-plaintext goodbye frame"
  转身即忘:  "zero persistence"
  自报家门:  "unsigned self-reported name frame (noise-encrypted, kind 0x05)"
  无名氏:    "default name when -n omitted"
  找人:      "/dial outbound connect"
  注意力:    "focus — which conversation receives typed lines"
  收摊:      "conversation teardown by the ear task"
  介绍人:    "teahouse keeper's side job — udp address exchange, same port as tcp"
  递拳头:    "punch — repeated PUNCH frames to the peer's public endpoint until one comes back"
  打洞:      "udp hole punching; UdpPunch (one frame per datagram, echoes swallowed)"
  名片:      "/card — name + per-interface dial addresses (survives all removals)"
  茶馆:      "--courier two-person-room relay; keeper doubles as introducer on udp"
  暗号:      "user-chosen pairing secret, SPAKE2-verified, chains at farewell"
  对词:      "SAS — 派生短词，两边各看各的屏幕，想核对的人口头对一遍；即时显示零落盘"
  凑话:      "contribution — each member's fresh 32B random; v3: 自署行 (name‖r‖pk‖contrib_sig)"
  圈语:      "CircleMumble — undecipherable group frame surfaced as a notice (never silence)；v3 升格为核心感知（听见失败）"
  补话:      "topup (0x0B) — 见证人署名的一帧成员变更；不相交变更序无关（交换代数），冲突必响亮"
  点名:      "roll call — /circle 重敲 = 新 circle_id 重新聚首；与创世同构；唯一的真理修复路径"
  见证人:    "witness — 死线持有者；离席的唯一合法证明人（A2）"
  名单:      "member table — 圈的唯一状态；每行自署（身体的印）；同时是路由表和世界观（信念）"
  信念:      "belief — 每个成员的名单是她的局部感知；没有权威副本；分歧是常态"
  开台:      "genesis (0x0F) — 署名邀请名单 + 席位标志；名单=发起者感知快照（事实陈述，非权力凭证）"
  看座:      "seat offer (0x10) — 引荐人把席位骨架（无 r！）递给候选人"
  落座署名:  "seat accept (0x11) — 候选人自选 r、自签整行递回"
  座次表:    "seat roster (0x12) — 全表（含 r）仅递已成成员"
  引荐收据:  "introducer signature on ADD — 问责线：出问题时『谁带来的人』有答案；rekey 后遗忘"
  封桌:      "closed seats — 多人圈默认；ADD 本地拒收（响亮）；换血=点名"
  舞台:      "open stage — 一人圈默认开放；『想要观众就开个台』；引荐即背书"
  堂:        "hall — the big shared circle slot (P6)"
  桌:        "table — the small private circle slot; content never reaches hall or other tables (P6)"
  署名钥:    "one-time ed25519 voice key — public half rides the roster row, private half dies with the process"
  泛洪圈:    "flood circle — connected-is-enough distribution; belief-routed (v3: 只走名单上的线)"
  一人成圈:  "solo circle — a channel of one, open stage by default; joiners arrive via seat flow"
  话筒:      "the cancelled loudspeaker — kept here ONLY so future agents recognize it as rejected"
  具身:      "A1 — 人=进程实例；嗓音与生俱来随进程死亡；一切有后果的行为必须署名"
  局部性:    "A2 — 感知只来自直连；无全局名册；路由表=世界观；分歧是常态"
  不可撤回:  "A3 — 说出的收不回；换气只封存未来；旧钥明文是历史"
  可重启:    "A4 — 重新介绍、重新对暗语永远可用；点名与创世同构"

# ───────────────────────────────────────────────────────────────
# VERIFY — how to check your work
# ───────────────────────────────────────────────────────────────

verify:
  build: "cargo build --release"
  test: "cargo test --release — 31 tests at HEAD (25 integration + 5 unit + 1 pq), ~seconds"
  ffi_build: "cargo build --release -p e2ee-ffi — target/release/libe2ee.so; verify: nm -D target/release/libe2ee.so | grep ' T e2ee_' (expect 12)"
  ffi_contract_test: "gcc -O2 -Wall -I include ffi/ffi_test.c -o /tmp/ffi_test -L target/release -l:libe2ee.so -Wl,-rpath,$PWD/target/release && /tmp/ffi_test — expect 'FFI OK'; then python3 ffi/ctypes_test.py"
  static_build: "cargo build --release --target x86_64-unknown-linux-musl — static-pie ~1.3MB"
  wire_confidentiality_check: "tcpdump during a two-instance conversation; grep -a for names/chat words in the pcap — MUST find nothing"
  yaml_selfcheck: "python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))'"
  v3_test_matrix:                            # write these tests WITH the v3 implementation (pending_work.P3 slice-6)
    - "star and chain topologies form a working circle (non-full-mesh, member-subgraph connected)"
    - "P-01: forged bye (wrong/no sig) rejected loud; own bye triggers rekey everywhere"
    - "P-02: member's out-of-circle 1:1 line receives ZERO circle frames (belief routing)"
    - "P-03: off-roster contribution rejected loud; on-roster all-names-arrived completes"
    - "P-04/P-12: introducer-forged row fails contrib_sig; colliding r rejected loud"
    - "P-06: disjoint topups applied in any order converge to the same roster+key (exchange algebra)"
    - "P-06: conflicting topups → mumble on both sides, NEVER silent split; roll call recovers"
    - "P-07: full seat flow — newcomer decrypts + verifies after 0x10→0x11→ADD→0x12"
    - "P-08: MITM-spliced gathering yields DIFFERENT sas words on the two halves (deterministic)"
    - "P-09: declining candidate (holds skeleton + all sniffed frames) cannot derive K"
    - "presence-defense: RM against a member with recent verified speech → topup withheld + loud conflict"
    - "solo circle forms (open stage); joiner arrives via seat flow; closed circle rejects ADD loud"
    - "50-member flood circle gathers, speaks, and roll-calls within patience budget"
  agent_preflight: "read theory (axioms + decision_procedure) FULLY, then group_v3, then settled_decisions + attack_precedents BEFORE proposing any design change — most ideas you will have were already settled or rejected here; if an idea survives step_1-5, bring it to the owner with its axiom derivation"

licensing:
  license: AGPL-3.0-only                    # no commercial exceptions, no dual licensing, ever
  non_commercial_stance: true               # full statement lives in the human READMEs
  commercial_exceptions: never              # do NOT offer, imply, or negotiate dual/commercial licensing on the author's behalf
  sponsorship_channels: none
  readme_license_note: "human READMEs state: AGPL-3.0 only — use must comply in full; resale ignoring AGPL obligations = copyright infringement. When editing READMEs, NEVER re-add 'commercial use requires written permission'-style clauses that conflict with AGPL grants"
  disclaimer_keys:
    - research-and-education-use-only
    - telephone-model-no-identity-auth      # active-MITM risk is MITIGATED (not eliminated) by SAS; accepted by design
    - unaudited-experimental-code           # not for sensitive communications
    - no-warranty-liability-capped-by-law
