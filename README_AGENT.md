# ═══════════════════════════════════════════════════════════════
# README_AGENT — machine-facing project brief
#
# FILE CLASS : machine-facing brief. Audience: LLM agents / coding copilots.
# HUMANS     : stop here. Use README.md / README_ZH.md instead.
# FORMAT     : yaml-dialect. indent=2; keys are stable IDs; `#` comments are
#              NORMATIVE (constraints), not decoration. File body is pure YAML.
#              STRUCTURAL INVARIANT: the whole file MUST parse as ONE valid
#              YAML document.
# SCOPE      : as-built facts describe HEAD (v3 slice-1..6 landed 2026-10-15/16,
#              P15 closed 2026-10-17 — P3 complete; P4 removal face CODE-COMPLETE
#              2026-10-17: v1 discovery family REMOVED + ffi alias + human READMEs
#              rewritten + flood_frame_all migrated & deleted (removals board all
#              cleared) — release chores (tag 3.0.0) remain, owner leaning cut;
#              P5 android + P6 nesting + P7 hardening + P16 line-proof pending).
#              group_v3.nested_circles and pending_work carry OWNER-SETTLED DESIGNS
#              not yet written — every such block is status-tagged. NEVER document
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
  rule: "README.md / README_ZH.md 已随 P4 按 v3 世界全量重写（2026-10-17，doc_index.rule 约定兑现）；此后三份文档同步维护——human READMEs 描述用户面，本文件承载机器面与裁决库。"

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
    2026_10_16_topup_signature_payload: "topup 签名的载荷 = 变更 + 发起者自己的新行（RM：witness_sig 签 circle_id‖RM‖离场者‖新行；ADD：introducer_sig 签 circle_id‖ADD‖新人行）。行的当前有效签名 = 最初的落座署名，或最近一次旋转她的 topup 签名。没有任何机制旋转别人的 r——RM 转见证人自己的，ADD 转引荐人自己的，增删和旋转永远同签名者（A1）。"
    2026_10_16_presence_wallclock: "在场辩护的『近期』= 本地挂钟 30 秒窗口（帧无时间戳，counter 单调但无速率语义——水位不随沉默衰减，无法表达『最近还在说』）。到达时刻本地记账，不落盘、不进协议。判定输入 = 本地证据 ∪ 任何线到达的该成员近期已验签发言——泛洪天然把在场证明送上门，见证人可以不看她，挡不住别人的线把她的声音转过来（A2：证据的到达延迟由泛洪补偿）。已见其署名告别 → 视为一致照常应用。"
    2026_10_16_sas_format: "SAS 短词 = 12 位数字分 4 组（123 456 789 012，约 40bit）。人类工程学：电话念 12 位分组数字是数十年验证过的仪式；安全性：与 ZRTP 20-30bit 相比更保守，且每次聚首新鲜、不累积不持久。1:1 从 Noise transcript 哈希派生，群从 HKDF(K, \"circle-sas\") 派生。"
    2026_10_17_left_voices_tombstone: "left_voices（圈内嗓音墓碑：名字→公钥；apply_rm 除名时写入；随圈生灭，点名重置，进程死亡）——A2 直接推导：我的信念记得她走过，墓碑是信念的一部分，与名单同族非新物种。用途：迟到直接告别帧的验签（RM 先到、名单已除名时，直接接收者经 members ∪ left_voices 取公钥验签后照常发见证 RM——幂等应用正是 rm_multiplicity 语义；转发接收者的提前跳过保留，旧闻无害）。四条精化随裁决钉死：① 名字复用优先级——members 优先于墓碑，ADD 应用时清除该名字的墓碑条目；② bye 去重键 = 签名而非裸名字（Ed25519 确定性（RFC 8032）⇒ sig 即事件 ID：同事件重放同 sig 被去重，重入后新钥新 sig 放行——裸名字键在『离场→重入→再离场』下吞掉第二次告别；flood_key 注释的『告别用名字』预留被本裁决取代）；去重在验签之后（集合只收有效签名，垃圾进不了集合）；③ 墓碑只存公钥非秘密——不 Zeroizing，RAM-only 零持久化不受影响；有界性：对抗 ADD/RM 循环可增长但每轮受 seat-flow 成本约束，接受为已知边界；④ 衍生检查项（已回答 2026-10-17，升级立案 P-15）：别名映射本身结构性无碰撞（仅 seat flow 写入、按对话编号键控，无自报名合并路径）；但基础路由第一判定 their_name（0x05 密文内自报、无验证）构成 P-02 重生面——自报名即入路由 → 圈帧送入攻击者 Noise 会话 → 收齐 r 得 K → 静默旁听。已立案 P-15 判例（方向 a 线绑定），规格裁决见 theory.rulings.2026_10_17_p15_line_binding_spec。crash 路径 witness_ctx 同步改 members ∪ left_voices。"
    2026_10_17_p15_line_binding_spec: "P-15 方向 a 规格裁决（owner 复核编码模型立案，追认五条精化 + 两处 spec 钉死 + 实现授权）。五条精化全部追认：① 三种线绑定行为（form_circle 发起者快照=创世名单裁决复用——名单本就扫自她自己的线，自洽感知宣誓；0x0F 送达绑定；seat flow 双侧绑定=封桌裁决『新人命运必然经引荐人』的必然后果——0x10/0x12 是 1:1 直发帧，到达绑定无转发歧义）② 两个负结果入档（0x09 凑话到达不绑线——泛洪帧署名者≠线的对端、转发与直达不可区分，绑它=用别人的嗓音给自己的线授权，且捕获合法 0x09 重放即得圈线资格；『验签发言绑线』死码拒绝——信念路由决定发言只从已绑线发出，机制永不可达=投机机器）③ genesis direct-lock 退役（creator==their_name 本身就是一处 their_name 检查=P-15 攻击面在创世处的实例，退役是同一修复的完成而非新裁决；幻影圈等价 1:1、/word 群词可检，走 P-08/T5 既有模型）④ P-05/见证判定连带迁移（必要非清理：crash 路径见证判定读 their_name + 攻击者自报成员名 → 其线被当成员线 → 杀线触发伪证 RM 逐出真成员，全程无需伪造任何签名；顶替场景『直连声明取代』退为『响亮冲突+点名』——两个身体同名是两个身体（A1）、自报不是感知（A2）、裁决权在人类（A4），是修正不是退化）⑤ 已知代价诚实入档（发起者中继瓶颈、/circle 全量重绑；瓶颈死亡场景退化为 mumble→点名，A4 完整保留）。方向 a 凭证枚举勘误：owner 原枚举含『凑话到达』，编码模型剪枝正确，追认。两处 spec 钉死：(a) 0x0F 送达绑定的语义 = 圈线资格（该线获得此圈的路由资格），非 creator 名字绑定——链式拓扑下 0x0F 经成员转发到达，线的对端不是 creator；creator 身份来自帧内签名而非线，绑定若写成名字绑定则链式成员误绑转发者为 creator，结构性错误。(b) 0x12 重放面显式断言：前成员重放历史自签 0x12（行级 contrib_sig 用旧公钥永远可验——A1 一次性嗓音但公钥不撤销）→ 若到达绑定授予线资格 → 安全性依赖『每次离席必旋转一行（见证人 fresh r）』不变量：离席者必缺 ≥1 枚现行 r，无 K；其名单外发言被 P-03 响亮拒，最坏流量黑洞、点名恢复。此安全性不得靠 incidental 成立——不变量钉为测试断言，任何未来改动若允许无旋转离席，0x12 重放面即静默失效。簿记三项：P-14 补录（slice-4 落地的行内 sig_pk 与名单一致性检查追溯占号）；P-15 立项带『slice-6 前必修』标注（测试矩阵 P-02 条目在恶意自报变体下不成立，无依赖≠次序自由）；判例 8 条断言与矩阵条目映射核对（防并入遗漏）。实现授权：P15_line_binding_routing 开工——绑定结构、四个改写点（genesis 快照 / 0x0F 送达 / seat flow 双侧 / P-05+见证判定迁移）、测试矩阵按本裁决修正后执行。"
    2026_10_17_p15_landing_audit: "P-15 落地复审（owner，2026-10-17）：主体全部接受（绑定结构、四个改写点、spec-a/spec-b 测试构造）；三个附带修复逐项追认（0x0F 同圈重放闸=incidental 守卫显式化正是 spec-b 纪律；ADD 契约类拒收照转/伪造类终止=星型下拒收者不得当全局守门人；早到凑话去重键时序定罪）。裁量（members ∪ left_voices）接受但同构论证修正：bye 验签的 union 是验签的必要条件，crash 判定的 union 不产生新证据、只是幂等冗余见证——witnessed_departs 守卫兜住，无害。三项裁决：阻断项 B（via 键撤销在中继拓扑下是 A3 回归——泄漏论证只在 via==线对端时成立，spec-a 合法化的转发送达使 via≠对端成为合法状态；pre-P15 的 their_name 查名单结构性封住此面，P-15 引入的是回归不是可声明边界）→ 修复见 rulings.2026_10_17_p15_peer_keyed_revocation；收尾项 A（中继反方向路由机制未入档——M→B 方向在三种绑定行为下无一路取得资格，要求第四行为/路由规则入档 + A2 推导 + 与负结果①②对账）→ 见 group_v3.routing.relay_asymmetry；收尾项 C（名字复用 × crash 见证缺测试——真风险轴是 ADD 同名清除作用域：v1 线死时 crash 见证读旧绑定指向名、在 members 命中 v2 → 无据 RM 逐出新任期，members-only 与 union 两变体都经 members 分支暴露）→ 作用域钉死（清该名全部绑定=Name+Qualified{peer}，先于引荐人写新绑定）+ 集成测试落地。反向错位（creator 离席过度撤销 via==creator 线含通往在席转发者的线）随 peer 键修复消失——过度撤销面不再存在，点名自愈（A4 创世同构重绑线）钉为测试。关闭条件全部达成：B 修复落地（回归测试自证：修复前红/修复后绿）+ A 入档对账 + C 作用域+测试。slice-6 次序不变。连带定罪：canary 契约曾被违约（迟到告别测试未门控 rogue API → 无 feature 时集成测试编译失败——『恰一条响亮失败』被编译期符号缺失取代），随本轮修复。"
    2026_10_17_p15_peer_keyed_revocation: "阻断项 B 修复裁决（对端键撤销）：Qualified 绑定升为双键 Qualified{via, peer}——via=帧内可验的授予者（genesis creator/seat flow 引荐人 hint），纯溯源不进任何判定；peer=授予时刻送达线对端的 0x05 自报名捕获快照（每线终身不变——握手后首帧定名），成为撤销键/路由键/见证键三判定的唯一读点。公理推导：A3（RM 封存未来）的作用对象是离席者=线对端，不是授予者——撤销键错位在 via 上，转发送达线（via=creator、对端=转发者）就在转发者离席时续命，RM 帧自身携带的见证人新 r 经此线泄漏新 K（先应用后泛洪的时序只保住发起者自己的出线，转发链上每跳的出线由各跳的撤销决定）；A2 认可 peer 为唯一线对端感知源（她在这条线上亲口报的名）——P-15 的功绩是把 their_name 从授予判定退役（自报名不得给予资格），本裁决把它放回熄灭判定（资格的存续以对端在信念为条件）：授予=协议行为（帧内可验），熄灭=信念检查（对端出信念即熄）。四种判定迁移：路由 Qualified{peer}∈belief（原无条件放行=回归根源）；apply_rm 撤 Qualified{peer:离场者}（原 via 键）；apply_add 同名清账扩到 Qualified{peer:重入名}（C 项作用域）；points_to/witness_ctx 见证读 peer（原 via——顺带修正：转发线的对端是转发者，转发者的告别由此线持有者见证，创作者经别线转达的告别不越权见证，感知必须是一跳的）。正向红利：creator 离席不再过度撤销通往在席转发者的中继线（over-revocation 面随键修正消失）。残余面诚实入档（pre-P15 同等暴露、非回归，pk 锚定前不修）：对端 0x05 谎报成员名的资格线在被谎报名离席前持续路由（名字≠身份的既定边界）；谎报名 ∉ 信念的送达线永不路由（fail-closed，转发送达的入座半途而废——pre-P15 同构）。结构性修复方向（方向性批准——owner 关闭裁决 2026_10_17_p15_closure_granted，立 pending_work.P16_voice_key_line_proof，排 P4 之后、不阻塞 3.0.0）：线有密码学验证的对端身份锚定——注意本套件 Noise_NN 无 remote static（NN 模式不传静态钥），owner 前期提示的『Noise remote static』系勘误（NN 下不存在，closure 裁决正式入档）；可行同构是嗓音钥线证明（成员以圈嗓音钥签 circle_id‖线 transcript 哈希，收端按名单行 pk 验证并锚定绑定到 pk——撤销/见证按 pk 而非名字，名字复用与谎报面同时消失），属新协议元素（新帧或送达包装），设计走 decision_procedure 全流程过堂后开工。"
    2026_10_17_p15_closure_granted: "P-15 关闭裁决（owner 复审 B/A/C 回报，三项全闭环，P-15 关闭，P3 全板完成）。(B) peer 键双键修复接受：A3 封存的作用对象是离席者=线对端而非授予者——撤销键追踪对端是公理必然；授予=协议行为（帧内可验，P-15 战果保留），熄灭=信念检查（peer ∈ belief）。攻击链封闭验证：RM 两路径先应用后泛洪 + 全部 peer==离席者线在泛洪前熄灭 → RM 帧自身的见证人新 r 无路送达离席者。自证红/绿（临时还原修复前行为让 B 回归测试变红）入档为修复落地标配：今后每个安全修复的回归测试都须证明修复前真的红。(A) 机制识别接受——owner 矩阵条目前提被取证证伪：M→B 方向结构性不存在，半开边（relay_asymmetry）+ 强连通约束入档，负结果①②对账成立。(C) 作用域接受：清账=该名一切绑定（Name ∪ Qualified{peer}），apply_add 内先于 complete_bring 写新绑定，是 crash 判定之前的门。owner 勘误入档：『Noise remote static』提示错误（本套件 Noise_NN 不传静态钥），编码模型否决正确；其替代方案获裁（见下）。owner 非阻塞验证项（引荐人自身绑定存活）确认已被 C 测试『B 的 Carol 绑定恰一条』覆盖。"
    2026_10_17_flood_frame_all_migration: "flood_frame_all 迁移裁决（owner 复审 P4 盘点表，批准迁移并删除）。编码模型论证『P-15 快照语义下与 belief_flood 目标集构造性等价』在稳态成立，但等价性只是偶然为真：快照绑定与 flood_frame_all 的 convos 读取是两次独立锁获取，漂移窗内新线使发送集偏离誓言集。A2：邀请名单是发起者签过名的感知誓言（创世名单=快照），发送必须结构性地追随誓言——改为向持有本 circle_id 新鲜 Name 绑定的线扇出（=快照集），flood_frame_all 整体删除（唯一调用点 form_circle 0x0F 扇出，点名同路径复用，同裁决覆盖）。漂移窗价格：快照后到达的线得零帧而非一帧孤儿——她的名字本不在名单（P-03 拒凑话），结构上更诚实；恢复路径 = 重开桌（A4）或 seat flow。无机密性差异（0x0F 无 r），此迁移封的是一致性破洞（发送集≠誓言集），非机密性破洞。纪律同族：witnessed_departs 守卫、0x0F 重放闸——偶然安全必须显式化为结构安全。"
    2026_10_17_p16_spec_approved: "P16 spec 过堂裁决（owner，2026-10-17）。Spec 过堂通过，四裁决点已裁：① 严格档照准（成键后名字锚线路由熄灭）；② 到达未绑定线的 0x13 验签通过 ⇒ 授予 pk 锚定（授予家族第三员；relay_asymmetry 半开边随之消解）；③ 三发照准（凑话后 + 成键后 + 点名后，幂等）；④ 撤销（moot）——owner 已裁决先全部完成代码再发版，P16 落入 3.0.0 本体，版本号 3.0.0 非 3.1.0，3.0.0↔3.x 断代不存在。窗口信息流围栏裁决（owner 补充）：名字锚窗口内谎报名线可收创世 + 0x09 行（行携带 r，集齐即得首代 K）——遏制 = 严格档使该线成键即熄，零密封帧到达，K-无密文 = 零信息增益；主动伪造被路由封死（线熄 = 无投递路径）；此遏制为结构性论证，不得依赖『恰好收不齐行』的流量时序侥幸。实现范围（spec §7 授权）：LineBinding 增 proven_pk: Option<VerifyingKey>（None = 名字锚窗口态）；0x13 LINE_PROOF 收发处理器（载荷 circle_id(8B) ‖ sig(64B)，签名载荷 circle_id ‖ 本线 transcript(32B)）；四类判定迁移（belief_flood / witness_ctx / circle_bye / apply_rm 按 pk 读，判定函数签名收 &VerifyingKey 不收 &str——结构性不可达）；成键切换点（key.is_some() 时名字锚线路由熄灭）；proof_payload 定界函数 + 单元；ffi 零变更零新事件。落地要求：their_name 重对账（判定路径残迹数必须 = 0，含 circle_bye targets 迁移后）；测试矩阵 spec §6 十条 + 单元面，镜子测试红绿自证标配，测试 1 的零帧断言显式覆盖密封帧类（窗口围栏的落地形态）；p15_fake_name_peer_residual 与 P15_C 矩阵条目加『P16 镜子测试』注记；越权即停（如要求名单外对端回证明才继续 1:1——那是越权，砍掉不自行发明）。"
    2026_10_17_p16_closure_granted: "P16 关闭裁决（owner，2026-10-17）。落地审计通过：their_name 判定残迹 0（全仓重对账 34 处逐一分类）、镜子红绿自证（镜子2 红实证：按名字查 → 无据 RM 误逐 v2；镜子1 复合还原红实证：密封帧到达出局冒名者）、rogue 全套 79 = 17 单元 + 61 集成 + 1 pq ≥ 72 基线、10 轮压力 60×10 全过 0 flaky、ffi 零变更零新事件、canary 契约双态复验。镜子 1 单还原（只关严格档）全绿的分层防御读法追认为红绿纪律判据——『纵深各层分别定罪』：单层还原测试仍绿时，须给出该层未上场、他层封面的机制读法（此例：冒名者为自证所发 0x13 使其线升级 pk 锚，出局由 pk 判定〔行删即熄〕封面，严格档为纵深第二层），再复合还原逼出本层的红（关严格档+关锚授予 → 冒名者停留窗口态 → 红实证）；今后每个纵深防御机制的镜子测试都按此判据执行。三处簿记随本 slice 捎带：verify 计数 72→79（61 集成 + 17 单元 + 1 pq）、modules.tests 头部 55→61、their_name_residuals 补【P16 已修】注记（照 p15_fake_name_peer_residual 格式）。P16 关闭；P6_nested_circles 开工——spec 先行，过堂后实现。"
    2026_10_17_p6_spec_approved: "P6 spec 过堂裁决（owner，2026-10-17）。第一裁决批准并追认为 P6 宪章级起点：协议状态不存在圈与圈的关系——『嵌套』是人类语义，协议眼里只有 N 个平行圈共享一些线；一切派生裁决由此生成。核心七裁照准：无子集强制（『⊆外圈』是关于他人信念的全称命题，无读点无法陈述、执行即跨圈判定=全局协调种子；子集=快照自然结果非规则）；钥匙独立（per-seat r 素材、输入集不相交；A3 封存作用域=被旋转素材所属的圈——外换气不封内、内不封外；『外逐则内逐』=政策传播机器，砍；外逐内留是语义非洞，诚实入档）；共享线多锚（线=物理对话，绑定/锚按圈独立，0x13 载 circle_id 既有结构直接支撑）；两套墓碑+无跨圈见证（一跳感知按圈取用；外除名内在席=平行事实各自收敛）；零新帧+选择性开圈（誓言对象是这份名单而非全部对话——A2 正译：请谁是主人自由；点名仍全量按本圈名单，A4 修复语义不动；语义升级：创世誓言集=指定线子集，空参默认=全部快照向后兼容，未入誓言的既有线与快照后新线同权零帧——P-02 按圈不变量，flood_frame_all 迁移裁决按新口径延续）；内圈第一天 pk 锚+严格档 per 圈（&VerifyingKey 签名形状即嵌套就绪证明）；深度=结构允许、语义同构、零优化（『外圈批准才可开内圈』=同步机复活，砍；RAM 每圈线性入档）。五呈报点裁决：① 圈类事件加 circle_id 批准——条件：字段必须到达 ffi 事件载荷（『C ABI 签名零变更』成立仅当序列化载荷含 circle_id，GUI 路由依赖；ffi round-trip 断言钉死），1:1 事件面零污染；② subset form = /circle 名1 名2 …（同一动词可选参，空=全部快照；不立新动词——新动词会把协议拒绝的区分实体化；名字显示层解析须唯一精确匹配，歧义响亮报错）；③ 内圈席位默认封桌照准（与外圈分歧需推导，无推导即无规则）；④ Gathering 容器留实现期（约束：迭代序确定性+进程内状态非跨节点注册表）；⑤ ffi 口径确认：零新事件类型、圈类事件载荷增 circle_id、命令面零新增、C ABI 签名零变更——实现中若需动 C 头，停下报裁决。测试矩阵增补三条：内圈 SAS 相异性（每圈独立群对词，内圈 /word 异于外圈——P-08 人墙在嵌套下仍工作）、flood_seen 跨圈不互斥（同内容帧两圈皆达，去重键含 circle_id 入断言）、共线多锚圈间独立（一圈行删熄灭不波及他圈绑定）。实现授权：按 spec 复用清单动手（判定面原样复用、调用点 per-Gathering 化、新增仅 Gathering 容器+subset 参数+事件 circle_id），红绿按『纵深各层分别定罪』纪律，79 基线不回退。"
    2026_10_17_commit_message_discipline: "commit 消息纪律（owner，2026-10-17）。成因澄清：d561f43 消息复用上条模板系 owner 侧失误（消息文本由 owner 提供），非编码 AI 责任，前例 aac8d7d『薄记』/5171ec7 计数偏差同族不再归罪。规则（中性，适用于任何人撰写的 commit 消息）：commit 消息须逐项对照 diff 据实撰写，禁止整体复用上一条模板；计数与范围失实等同内容失实。git 历史是本项目判例库的原材料，证词失真污染裁决体系。执行：d561f43 由 owner amend 重写（顶端 commit 无下游依赖，force-with-lease 安全）；此后失实消息一律 amend 重推。"
    2026_10_17_p7_spec_lite_approved: "P7 spec-lite 过堂裁决（owner，2026-10-17）。六项照准：密文填充三层尺寸框架（pad→norm→max，8→16 随机取值破尺寸指纹；帧类语义公共长度集（0x0F/0x10/0x11/0x12/0x0C/0x13 归一）；运输层 TCP/MSS 吸收变长；P-16 同判例族——观测面=攻击面）；courier tag 三层规则（会合包装 k=1 房号前缀、配对会合词模式 a+b 连接词+nonce+role、打洞 tag=词对+nonce+role——P-07 判例族）；PR_SET_DUMPABLE=0 结构性禁 ptrace（P-10 判例族正译）；counter 水位窗口（新决策项：水位=响亮的换气不可用性还原——窗口内早帧定位攻击等噪音，丢弃保持安静，与『噪音无传播权』一致；窗口外正常应用）；T0 边界重申。一处勘误：『重放闸现为接收侧帧计数上限』表述不准——flood_seen 是 (circle_id, 发送者 pk, 序列号) 三元组去重判定，非计数上限，实现时以代码实际语义为准。实施顺序建议：padding → tag 规则 → dumpable → 水位。"
    2026_10_17_p7_control_frame_padding_deferred: "P7 裁决①缓办项归档（owner，2026-10-17）。spec-lite 已批的『帧类语义公共长度集（0x0F/0x10/0x11/0x12/0x0C/0x13 归一）』实现落场为仅 0x0A 密封帧分桶填充——落差裁决：控制帧长度归一化缓办。理由：控制帧长度泄露的是圈子结构（成员数），而成员数已被稳态流量分析结构性泄露（每发言的扇出模式对线路观察者恒可见）——归一化封的是已开信道，无边际保密收益；有实质意义的指纹面（聊天内容长度推断）已由 0x0A 分桶填充封死。与不对称表『载体可见元数据=诚实入档，不修』同族：此为诚实声明的边界，非遗漏。若未来出现仅经控制帧长度可感知、流量分析不可达的信息差，此裁决重开。"
    2026_10_17_p5_completion_scope: "P5 完成面裁决（owner，2026-10-17）。aarch64 直测通过（owner 侧独立构建复现：12 e2ee_* T 符号逐项一致、四条 LOAD Align 全 0x4000、host 契约 FFI/hidden/ctypes 复跑全绿；owner 构建 sha256 edce9230…与交付物 67b10bd2…不同——无 reproducible-build 旗标下的正常非确定性，发布物身份=随 release 交付的二进制及其 sha，非字节复现性）。剩余三目标（armv7/x86_64/i686）+ GitHub Actions + release artifacts 全部完成于 tag 前——owner 裁决：先全部完成再发版涵盖打包面。注意：armv7 为 ELF32，其 p_align 表现以 llvm-readelf 实测为准（若 NDK r28 对 32 位默认非 16KB，如实入档报裁决，不强行加旗标）。"
    2026_10_17_ci_debug_budget_mismatch: "CI 测试 job 红迹归档（owner，2026-10-17，附复现矩阵）。现象：ff148bb 的 CI（rust.yml——0bc55b8 v1 时代添加，先于全部 v3 工作存在）test-linux/test-windows 红：五十人（linux）/P16_镜子1（双平台）/在场辩护（windows），签名一律为事件 helper 15s 预算 Elapsed。owner 侧单变量实验定罪：同套件唯一变量换 debug——五十人即刻红（release 恒绿）；P16_镜子1 debug 单跑 3.26s 绿（争抢受害者：与五十人 60s+ 长跑共享 2 核被饿死）；taskset 2 核复现同形。根因=rust.yml 沿用 v1 时代 cargo test（debug）+ 15s 固定事件预算 + 2 核共享 runner——v3 验证纪律自 P3 起全部按 release 执行，CI 配置从未跟进；红迹自预算重型测试落地即存在（Actions 历史可核），ff148bb 非回归源（P17 行为中性，三环境 92 基线绿）。修复双管：① 测试步骤改 --release（对齐项目实际验证形态）；② 事件预算环境化 E2EE_TEST_EVENT_BUDGET_MS（默认 15000 不变，本地语义零变化——预算是环境校准非断言语义，不触测试语义弱化红线；CI 置 45000）。此即 fifty_star_scale_budget 定罪时悬置的『规模档预算分层』之裁——环境速度分层经预算旋钮实现。流程教训：CI 红迹长期无人察觉（含 owner 侧误报——『无 workflow』检查曾被 zsh glob 失败短路）；结构性建议：owner 开 branch protection 必需检查，红 CI 阻断合流。"

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
  birth_modes: "born | --no-chain (never offers/accepts a next secret — the open-desk shape) | --guard <machine-secret> (EVERY arrival must pass PAKE; arming NEVER consumed; armed open-desk = --guard KEY --no-chain)。3.0.0 已执行：出生只竖 TCP 耳——喊话发现家族整体退役（removals_3_0_0.v1_discovery_family LANDED 2026-10-17），--hidden/born_hidden 随之消亡（无喊话则无隐身语义），ffi e2ee_person_create_hidden 保留为 deprecated 别名（ABI 冻结）"
  other_job: "--courier — the same binary as a teahouse: room numbers are GUEST-CHOSEN; wires same-room arrivals, relays opaque bytes; no identity, no storage, no plaintext"
  commands:                                  # in-session, parsed by CLI handle_command (src/main.rs)
    dial:  { syntax: "/dial ADDR:PORT [secret]", behavior: "outbound connect + hybrid NN handshake as initiator; optional secret → PAKE first; auto-uses chained secret for that addr (burned only when actually verified); 5s connect timeout" }
    meet:  { syntax: "/meet COURIER ROOM [secret]", behavior: "enter a teahouse room; roles assigned by courier (later arrival initiates); optional secret → PAKE inside the room; teahouse lines never chain" }
    punch: { syntax: "/punch INTRODUCER TAG [secret]", behavior: "register the same tag with the introducer; he exchanges public endpoints; both sides hole-punch; noise handshake runs over the punched udp line; best-effort — on failure the user re-routes via /meet MANUALLY" }
    await:  { syntax: "/await SECRET", behavior: "arm the password challenge for the NEXT inbound arrival (one-shot; spent only when PAKE masks are actually exchanged)" }
    talk:  { syntax: "/talk id|name-prefix", behavior: "switch input focus; ambiguous prefix lists candidates" }
    list:  { syntax: "/list", behavior: "enumerate live conversations, mark focused" }
    card:  { syntax: "/card", behavior: "print one 'name  ip:port' line per interface — touches no network；discover.rs 在 3.0.0 删除面后仅存 card/local_ips" }
    circle: { syntax: "/circle [封桌|开台]", behavior: "开台/围坐/点名：扫描发起者当前会话→署名邀请名单→0x0F 只走一跳。席位默认多人封桌/一人开台，可显式覆盖（开台是刻意的宣言）。重跑 = 点名 roll call（新 circle_id 重新聚首，A4）。库面 form_circle() / form_circle_seats(Option<u8>)" }
    gsay: { syntax: "/gsay TEXT", behavior: "speak in the circle (signed speech 0x0A); v3 gains a circle-target argument for nested circles" }
    bring: { syntax: "/bring [拟名]", behavior: "领人入座（slice-4 已落地）：向聚焦对端（候选人）发 0x10 看座。带拟名 → 期望门（0x11 自署名不匹配 → CircleConflict 响亮暂停 + 回执丢弃 + 待座保留；放行=重跑 /bring）；不带 → 无门直接自动。封桌时 CLI 警告但照发（拒绝是全员契约行为）。库面 bring(Option<&str>) -> seats，外层据此警告" }
    sit: { syntax: "/sit [名字]", behavior: "落座（slice-4 已落地）：候选人收到看座事件后触发——0x11 落座署名必须是候选人本人的手势（A1）。默认取本人 0x05 自报名，可覆盖（「这桌叫我阿花」）；多份邀约按 /talk 模式列候选（聚焦线优先→唯一→歧义响亮）。库面 sit(Option<&str>)" }
    word: { syntax: "/word", behavior: "对词（slice-5 已落地）：聚焦 1:1 时显示会话短词（Noise transcript 哈希 HKDF(p2p-sas) 派生，两端相同）；在圈时显示群短词（HKDF(K, circle-sas) 派生，全员相同）——两者兼有时都报。12 位数字分 4 组（~40bit）。即时计算零落盘。库面 sas_word() -> SasReport（NoSasChannel）。见 group_v3.sas" }
    file:  { syntax: "/file PATH", behavior: "hand a file to the focused conversation: 0x06 head + 0x07 chunks; lossless; 32MB cap both ways; received files auto-save to ./received/" }
    bye:   { syntax: "/bye", behavior: "offer next secret (chainable lines only), then circle_bye (0x0C) if in a circle, then empty-plaintext goodbye to focused conversation" }
    quit:  { syntax: "/quit | Ctrl-D", behavior: "circle_bye to all circles (chainable lines offer secrets), goodbye to all conversations, exit 0" }
  planned_commands_v3: []                     # word/bring/sit all LANDED (slice-4/5) — as-built entries live in commands above
  sessions: multiple-concurrent-1to1
  focus_model: "typed lines go to the focused conversation; focus set by /dial or /talk; incoming conversations never steal focus"
  symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node — extends to circles (ownerless)
  packaging: workspace-three-gates            # root pkg = rust lib `e2ee` + bin; ffi/ member = cdylib gate — see ffi block
  api_style: "library NEVER prints nor reads stdin nor touches the fs; actions in = metaphor-verb methods; experiences out = Event stream (tokio mpsc); Event/LeaveReason/PersonError are non_exhaustive; errors carry semantics, CLI renders prose"
  modules:
    src/lib.rs:     "crate root: pub mod circle + courier + discover + person + punch + wire; re-exports Person/Events/Event/LeaveReason/PersonError/RosterEntry/SasReport; cfg(rogue) re-exports person::rogue. 3.0.0: Discovered re-export 随喊话发现退役移除"
    src/person.rs:  "core person model; handshake<S> GENERIC over AsyncRead+AsyncWrite (tcp and UdpPunch alike); ear() dispatches on frame_kind; ZERO printing, ZERO stdin, ZERO fs. v3 adds (slice-1..4): Gathering (voice keypair + roster rows + seats + line-aliases), 0x09 self-certified rows, 0x0A dual-gate, 0x0B RM/ADD (witness/unified introducer, sig_pk-vs-roster consistency), 0x0C signed bye, 0x0F genesis/roll-call, seat flow 0x10/0x11/0x12 (bring/sit + expectation gate + closed-table fork), belief-routed flood with aliases, presence defense (30s wallclock); slice-5: per-convo Noise transcript capture (noise_stage) + sas_word() -> SasReport (NoSasChannel); slice-6: #[cfg(feature=\"rogue\")] pub mod rogue — rogue_frame/rogue_flood/circle_id/roster_snapshot/forge_row, injection via REAL send paths; 2026-10-17 fix: Gathering.left_voices 嗓音墓碑 + witnessed_departs 义务幂等 + bye sig 键去重（验签后）+ 迟到直接告退见证复活（members ∪ left_voices）+ crash 路径 witness_ctx union；P-15: enum LineBinding{Name,Qualified{via,peer}} + Gathering.line_bindings（取代 aliases）——belief_flood 只认绑定（授予=协议行为，熄灭=对端键 peer 过信念检查）/ form_circle 快照 / 0x0F 送达（direct-lock 退役+同圈重放闸）/ seat flow 双侧 / P-05+bye+crash 判定迁移（读 points_to=peer）/ apply_rm 撤 Qualified{peer:leaver} + apply_add 同名清账 Name+Qualified{peer} / ADD 契约类拒收照转；P4：flood_frame_all 已删——0x0F 扇出走 belief_flood（扇出追随誓言，rulings.2026_10_17_flood_frame_all_migration）、Person::shout/born_hidden 已随喊话家族退役"
    src/punch.rs:   "introducer client + punch transport: UdpPunch (frame-per-datagram, echo swallowing); INTRO2 source-checked"
    src/circle.rs:  "group key schedule: derive_group_key (HKDF over byte-sorted contributions, takes &[&[u8]] borrows), derive_sender_subkey, seal/open (ChaCha20Poly1305, nonce = counter ‖ sender-tag); slice-5: sas_word_from_transcript (HKDF(p2p-sas)) / sas_word_from_group_key (HKDF(circle-sas)), 12-digit 4-group format; cfg(rogue): RosterRow::seat_with_r (P-12 injection material); unit tests for order-independence, subkey isolation, SAS derivation, P-13 rotation-seals-history"
    src/courier.rs: "teahouse: serve() wires same-room arrivals, splices raw bytes; serve_introducer on the same udp port; JOIN frame bounded 10s"
    src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; clean EOF vs mid-frame break"
    src/discover.rs: "3.0.0 删除面已执行（2026-10-17）：喊话家族（SHOUT_PORT/PROBE/REPLY/Discovered/listen_for_shouts/shout）整体退役；仅存 local_ips + card（自己的网卡清单，不碰网络）"
    src/main.rs:    "thin CLI renderer; safe() sanitizes ALL remote-controlled strings; harden_process() = RLIMIT_CORE=0 + swap warning (P7 adds PR_SET_DUMPABLE)"
    ffi/src/lib.rs: "cdylib gate: 12 exported e2ee_* symbols; per-handle tokio runtime"
    include/e2ee.h: "C header — MUST change together with ffi/src/lib.rs (frozen ABI)"
    tests/conversation.rs: "70 integration tests, real persons on ephemeral ports (slice-4 adds 7: full seat flow, expectation-gate retry, seat-name override + line alias, closed-table fork + roll-call re-accept, closed-table repeated-bring DoS surface, same-introducer double-ADD ordered convergence, declining-candidate zero frames; slice-5/6 add 13: sas same-word + roll-call freshness, MITM-splice different words (P-08), 4-chain relay + seat completion, solo-stage joiner, forwarded-bye never issues RM, presence-defense withheld path (sacrificial-thread crash sim), P-01/P-03/P-04/P-05/P-12 rogue-injection rejections, 50-member circle, rogue canary; 2026-10-17 fix adds 2: late-bye witness revival (delaying-carrier gate: RM₁ preempts direct bye → tombstone verification → RM₂ → both rotations converge) and depart→rejoin→re-depart (sig-keyed dedup + ADD triple-clear + second RM fires); P-15 lands 3 new + rewrites 4 for star-routing semantics: P15_圈外自报名_零圈帧 (impersonating line receives zero circle frames), P15_spec_a_转发创世 (forwarded genesis grants circle-line QUALIFICATION via frame-creator, never name-binding; replayed direct genesis binds nothing), P15_spec_b_前成员重放 (mortal-proxy line kill: zombie table bring → phantom circle, every-departure-rotates invariant asserted, ex-member lacks rotated r → no K, P-03 loud reject); P-15 audit lands 2: P15_B_转发资格线_对端离席熄灭 (peer-keyed revocation regression, self-certified red-pre/green-post) + P15_C_同名重入_crash见证不误逐新任期; P4 删除面: 隐身 test dies with the feature, 出生无喊话耳_删除面守卫 takes its slot (bind 127.0.0.1:37777 succeeds ⇒ no 0.0.0.0:37777 listener — structural removal guard, canary family); flood_frame_all 迁移: 快照后新线_零圈帧_从第一帧起 (0x0F fanout follows the oath; unbound line receives zero circle frames from the first frame — event-level invariant, wire-level drift-window orphan undetectable via Event stream as it is silently roster-dropped); expect() deadline 15s (parallel-run scheduler pressure); P16 lands 6 (rulings.2026_10_17_p16_spec_approved): P16_镜子1_谎报名线_成键后熄灭 (mandate test 1, window-fence sealed-frame zero-arrival, red-green self-certified), P16_镜子2_同名复用_pk锚不混淆 (mandate test 2, red-green self-certified: name-based witness_ctx → false RM evicts v2 → red), P16_撤销统一_行删即熄_幂等重放无害, P16_circle_bye按pk_冒名线零圈告别帧 (their_name_residuals death), P16_半开边点亮_证明授予 (ruling ② unbound-line grant), P16_乱序容错_点名后三发重锚_双向恢复 (ruling ③); P15_B race-tolerant rewrite 2026-10-17: two-RM arrival race no longer order-pinned, both worlds asserted (M-first kept verbatim; A-first asserts peer-keyed revocation directly — forwarded line extinguishes, zero frames to the evicted)"
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
    topup: "0x0B — P2 as-built: circle_id ‖ epoch(u32) ‖ change-spec ‖ fresh_r(32B) ‖ initiator_sig(64B). v3 RESTRUCTURES (see group_v3.rekey.topup): epoch DIES, initiator election DIES; 变更字节 'R'=RM（离场者名 ‖ 见证人新行 ‖ witness_sig）/ 'A'=ADD（新人行 ‖ 引荐人新行 ‖ introducer_sig，签 circle_id‖'A'‖新人行）——布局镜像对称，行结构三载体同构。收端一致性检查（slice-4）：行内 sig_pk 必须与名单一致（A1：行名可伪、嗓音不可顶替）；ADD 非回声路径强制引荐人 r 已旋转（P-13）；去重键 = 帧签名者的新 r（RM 见证人/ADD 引荐人，镜像对称）"
    circle_bye: "0x0C + circle_id(8B) ‖ name — P2 as-built is UNSIGNED (forgeable, P-01). v3 ADDS bye_sig(64B) over (circle_id‖name)"
    # ── v3 frame kinds — slice-1..4 LANDED as-built ──
    genesis: "0x0F + circle_id(8B) ‖ seats(u8: 0=closed 封桌 / 1=open 开台) ‖ 发起人(名长u8‖名字) ‖ count(u8) ‖ invited names ‖ creator_sig(64B) — 开台公告：署名的邀请名单=发起者感知快照（事实陈述，非权力凭证）。凑话完成条件=名单上的名字全部自署到场（计数死，P-03 fix）。点名=新 circle_id 的重新 0x0F，社会连续、密码学新鲜"
    seat_offer: "0x10 + circle_id(8B) ‖ seats(u8) ‖ count(u8) ‖ (名长u8 ‖ 名字 ‖ 嗓音公钥32B)× — 看座（slice-4 已落地）：1:1 帧（非圈帧），引荐人把席位骨架递给候选人。红线：绝不携带任何 r——拒入者拿到 r 即可推导群钥（P-09）。同线新邀约覆盖旧的"
    seat_accept: "0x11 + 行（名长u8 ‖ 名字 ‖ r(32B) ‖ sig_pk(32B) ‖ contrib_sig(64B)，与 0x09 行同构）— 落座署名（slice-4 已落地）：候选人自选 r、生成嗓音钥、自签整行，经 1:1 递回引荐人。r 自选（128bit+ 碰撞概率可忽略）+ 收端唯一性响亮检查（P-12 fix）"
    seat_roster: "0x12 + circle_id(8B) ‖ seats(u8) ‖ count(u8) ‖ 全部名单行（每行同 0x09 布局，含 r）— 座次表（slice-4 已落地）：1:1 帧，ADD 应用后引荐人把全表递给新人（新人由此持全部 r 计算群钥，按本人嗓音公钥找到自己的座位）。只递已成成员。收端验每行自署 + 名字/r 唯一性"
    # 0x0D/0x0E 曾预留给见证/分叉通告——死于理论重建（同步机整体退役）；不得复活
  unknown_kinds: "silently skipped — forward compatibility (this is what makes the v3 additions rollout-safe for old peers)"
  hangup: "empty plaintext frame | tcp close"
  teahouse_control:                          # plaintext frames on the courier line, before the splice
    join: "'JOIN <room>' — first frame from each arrival; rooms guest-chosen; same-number collisions = cross-wired pairs"
    paired: "'PAIRED R' to the first arrival, 'PAIRED I' to the second — roles for the noise handshake"
  introducer_and_punch:
    introducer: "courier::serve_introducer — udp side of the teahouse port; INTRO1 tag / INTRO2 R|I peer; one-shot tags, ttl 90s, MAX_WAITING_TAGS 1024. P7 tag 三层规则：/punch 省略 TAG → 自动生成 128bit 随机标签并上屏供带外转告（建议 Crockford base32 分组，去混淆字符）；显式给出短标签 → 响亮警告但照用（降级自由，与暗号长短同理）；警告文案必须写明补偿关系（『短标签可被猜测劫持会合；若本次配对带暗号，则由暗号兜底』——tag 猜测只需介绍人在线、无需受害者在场，与 PAKE 的在场爆破模型不同构，不能靠在场感知兜底）。tag 防的是『无网络位置的廉价会合劫持』；带暗号时后果被 PAKE 层补偿，不带暗号时攻击者通过网络位置 MITM 本可达同样效果"
    punch: "register w/ retry (3s), connect to peer's public endpoint, exchange PUNCH frames (~150ms apart, 8s window); INTRO2 accepted ONLY from the introducer's own address"
    transport: "UdpPunch: one frame per datagram, punch-echo discarded, max frame 65507B; no liveness detection — a vanished peer hangs the line until /quit"
    fallback: "on punch failure the USER re-routes via /meet manually — no automatic fallback"
  lan_discovery:                             # v1 家族——3.0.0 删除面已执行（2026-10-17，removals_3_0_0.v1_discovery_family）
    status: removed-2026-10-17
    was: "udp 37777; E2EEPROBE1 broadcast probe, E2EEREPLY1 name+port reply; hidden mode = listener never spawned"
    removal_proof: "出生无喊话耳_删除面守卫（bind 127.0.0.1:37777 成功 ⇔ 无 0.0.0.0:37777 监听者）——结构守卫防爬回；cargo build 默认构建零 SHOUT/PROBE/Discovered 符号"

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
# GROUP v3 — 公理重建（P3 全部 slice 已落地 2026-10-15/16；P4 删除面/P6 嵌套见 pending_work）
# 这是 theory 块的实例化。一切机制可回溯公理；一切裁决见 theory.rulings。
# ───────────────────────────────────────────────────────────────

group_v3:
  status: implemented-P3-complete            # slice-1/2/3 LANDED 2026-10-15 (roster/0x0F genesis/信念路由/署名 0x0C/见证人 RM/在场辩护/点名)；slice-4 LANDED 2026-10-16 (入席流 0x10-0x12 + 期望门 + 封桌 fork 语义 + P-13 旋转强制)；slice-5/6 LANDED 2026-10-16 (/word 双层对词 + 判例注入矩阵 + rogue feature)；迟到告别竞态修复 LANDED 2026-10-17 (left_voices 墓碑 + sig 键去重 + 见证复活)；P-15 线绑定路由 LANDED 2026-10-17（方向 a：LineBinding 两形态 + their_name 降纯显示，attack_precedents.P_15）；P4 removal face next; P6 nesting after
  one_line: >-
    圈不是对象，是 N 个局部信念的对齐。协议的职责不是维持一致（那是同步机，
    违反 A2），而是制造四种感知：听见（AEAD）、嗓音（签名）、在场（直连）、
    见证（死线报告）。分歧不可预防但必被听见（T1），修复永远可得（点名，A4）。

  the_circle_object:                         # 名单（member table）是圈的唯一状态
    roster_row: "name ‖ r(32B) ‖ sig_pk(32B) ‖ contrib_sig(64B over circle_id‖name‖r‖sig_pk) — 每行自签（A1：身体的印），任何人伪造不了任何一行"
    row_rotation_semantics: "r 旋转后原 contrib_sig 签的是旧值，行『失签』。行的当前有效签名 = 最初的落座署名（0x11），或最近一次旋转她的 topup 签名（RM 见证人行 / ADD 引荐人行——帧携带重签的整行）。行结构三载体同构：座次表 0x12、ADD 新人行、RM/ADD 发起者新行 = 同一解析器同一验证器"
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

  routing:                                   # 信念路由——转发按名单，不按会话集（P-15 已落地：只认线绑定）
    rule: "圈帧只走圈协议建立的线绑定 − 来源线（P-15 方向 a LANDED 2026-10-17 + 复审 peer 键修复 rulings.2026_10_17_p15_peer_keyed_revocation）：两种绑定同一语义『这条线的对端在我信念里在座』——Name 绑定（form_circle 快照/complete_bring）对行名过信念检查；Qualified 资格线（0x0F/0x10/0x12 送达）对线对端键 peer 过信念检查（peer=授予时刻送达线对端 0x05 自报名捕获快照——A3 封存的作用对象是离席者=线对端，撤销键在授予者 via 上是回归：转发线在转发者离席时续命，RM 帧自身的见证人新 r 经此线泄漏新 K）。授予=协议行为（帧内可验），熄灭=信念检查（对端出信念即熄）；apply_rm 急切撤 Qualified{peer:离场者}（发送路径先应用后泛洪，RM 帧不漏给除名者）；apply_add 同名清账 Name+Qualified{peer}（旧身体的线不得因新身体入座复明）。their_name（0x05 自报名）不进授予判定；peer 捕获快照只用于熄灭/撤销/见证"
    kills: "P-02（圈外旁听：成员的圈外 1:1 线再也收不到任何圈帧，连 circle_id 都看不见）；P-04 补话断递（除名者连 topup 都收不到，fresh_r 无从获取——含转发送达线在转发者离席时熄灭）；P-15（自报名路由：冒名线零圈帧，收不齐 r 无 K）"
    topology_constraint: "连通性收紧为『成员导出子图连通』且**强连通**（每方向独立取得资格，见 relay_asymmetry——半开边只承载单向）；创世时发起者的星形天然满足；星心离场=断连=点名重建——现状同样脆，现在写成显式约束"
    relay_asymmetry:                         # 中继反方向路由机制（复审收尾项 A 入档，2026-10-17）
      mechanism: "M→B 方向（转发者→经转发送达的入座者）在三种绑定行为下**无一路取得资格**——结构性不存在：M 的线到 B 只能由 M 自己的协议行为绑定（快照=她是 creator、seat flow=她是引荐人、送达=B 递给她 0x0F/0x10/0x12），B 经 M 转发的 genesis 入座只绑定 **B 侧**的 B-M 线（出向），不绑定 M 侧（入向）。转发送达的入座因此是**半开边**：B 的凑话/发言出得去（B→M 资格线），A 的凑话/圈话进不来（M→B 无绑定）——B 的成钥必须经她自己的**另一条**被对端绑定的线（spec-a 测试：A-B 直达线放行后 B 补收凑话成钥；『凑话照收』的是 B 的行照收进 A/M 的名单〔出向工作〕，不是 M→B 承载了任何帧）。B 侧的 A-B 线接收不依赖 B 的绑定（路由是发送方判定）——接收面完整、发送面半开"
      a2_derivation: "A2：感知只来自直连。M 对『B 在圈里』的知识来自 B 的凑话到达（A2 合法）；但 M 向 B 发圈帧的资格只能来自 M 自己被授予的绑定——**别人的入座不构成我对她线的资格**（B 经 M 的线入座是 B 的感知宣誓，不是 M 的）。半开边是 A2 的诚实推论，不是缺陷：M 想点亮 M→B 只有两条正路——B 对 M 发起 seat flow（complete_bring 绑 M 侧），或点名（B 的快照全量重绑自己的线 + 新 0x0F 送达绑 M 侧，A4 创世同构自愈）"
      negative_results_reconciliation:      # 与 spec 裁决两个负结果的对账
        unbound_line_carries: "未获资格的线（发送方无绑定）**零圈帧出向**——belief_flood 在发送侧拒绝，含 r 的帧（0x09 凑话/0x12 座次表）只走发送方已绑定的线。接收方处理到达帧不需要自己的绑定（路由是发送方判定）——到达 ≠ 资格，到达不授予任何东西"
        contrib_replay: "负结果①对账：捕获合法 0x09 重放给受害者 → 行照收（若名在信念）但转发**排除来源线**（攻击者线一帧也拿不回）且 0x09 到达不绑线（无资格面）——①要防的『重放即得圈线资格』不成立。r 帧流向圈外线的唯一路径=持有 0x0F/0x12 送达帧的伪造递送（成员泄帧/载体 MITM（P-08，SAS 可检）/共谋（成员本有 K）——皆文档化边界，非①回归）；peer 键修复后此面进一步收紧：伪造递送线在谎报名（或真名）出信念时熄灭"
        speech_binding: "负结果②对账：『验签发言绑线』仍为死码——发言只从发送方已绑线发出，未绑线永远收不到发言，绑它=永不可达的投机机器（spec 裁决原文，peer 键不改变此结论）"
      their_name_residuals: "发送侧 their_name 残留面清单（P-15 范围外、无 r 泄漏）——【P16 已修】判定路径残迹 = 0（2026-10-17 全仓重对账：34 处命中逐一分类为注释/显示面/peer 捕获/测试对照，circle_bye targets 已迁移 pk 锚；P16 镜子测试 P16_circle_bye按pk_冒名线零圈告别帧 钉死），本条保留为迁移前档案。原记录（P-15 范围外、无 r 泄漏）：circle_bye_frames 的告别 targets 筛选读 belief.contains(their_name)——自报成员名的圈外线可收到 0x0C 告别帧（泄露面=圈号+离场者名+签名，元数据级；0x0C 无 r 无密文，不在 P-02 红线内）。owner 裁决（rulings.2026_10_17_p15_closure_granted）：与 P16_voice_key_line_proof 同批处理——同族代码一次触碰，排 P4 之后、不阻塞 3.0.0"
    carrier_visibility: "茶馆转发的是密文，但 circle_id 是明文（路由需要）——载体知道圈的存在/流量/拓扑。这是桌子的物理（不对称表第 4 行），诚实入档，不修"

  rekey:                                     # 换气——点名为真理，补话为捷径
    roll_call:                               # 点名（A4：唯一的真理路径）
      mechanism: "任何成员任何时候发起 = 新 circle_id 的重新 0x0F（与创世完全同构，代码路径复用）。同时是离开修复、分裂修复、怀疑修复、纯换钥——一个机制覆盖全部"
      social_continuity: "密码学上是新圈；社会上是同一张桌重新点名"
      trigger: "仅手动（再敲 /circle）。协议绝不自动建议点名——『建议』本身就是一个检测器，违反『协议不监视，人类做验证』（A2）。机器对已听见的失败（mumble）的响应仅限响亮呈现"
    topup:                                   # 补话（快速路径——人类也有捷径：有人离席，剩下的人不会重新自我介绍）
      rm_layout: "0x0B + circle_id ‖ 'RM' ‖ leaver_name ‖ 见证人新行(自签: name‖r‖pk‖contrib_sig, ~100B) ‖ witness_sig(64B, 签 circle_id‖RM‖离场者‖新行)。见证人换 r 后原 contrib_sig 失签——『每行自签』是公理级不变量（座次表 0x12 递给新人时每行必须可验），故帧携带重签整行"
      rm_witness: "见证人=离开的直接感知者（A2：离席只对直接信道可观察）——两条路径统一：crash 路径=死线持有者（直接感知线死）；自愿告退路径=署名 0x0C 的直接接收者（线的对端名字==告退者名，一跳，非转发；转发收到者只应用不发起——感知必须是一跳的，与 direct-lock 同构）。不再是 smallest-r 选举人——她自己供应 fresh_r 并署名。最小 r 占位攻击（P-06 前身）随选举机制整体死亡。自洽性：信念路由保证 0x0C 只走名单线 ⇒ 直接接收者必然是成员，不存在无人可发 RM 的死局；帧丢失 → 僵尸成员留在旧钥（no_liveness_probes 既有语义），直到谁的线死走 crash 路径"
      rm_multiplicity: "多直连对端各自发 RM 是不相交变更（删同一人 + 各旋转各行，T2 可交换，先₁后₂=先₂后₁）。幂等应用语义（多见证人收敛的必要规约）：RM 应用 = 条件删除（告退者在则删，不在则跳过——不能因已删而整帧丢弃，否则后到见证人的行旋转丢失、K 收敛失败）+ 无条件行替换（见证人签名验证通过则替换她的行）。中间态（只收到一个 RM 的人）短暂 mumble——响亮、自愈"
      rm_voluntary_vs_presence: "告别前发言与 presence_defense 的交互由既有例外覆盖：『已见其署名告别 → 视为一致照常应用』——Alice 说完『晚安』立刻 /bye，其 30 秒窗口内发言不阻止 RM 应用（该例外条款为两条路径共用）"
      late_bye_witness_revival: "迟到告退的见证复活（2026-10-17 定罪竞态的修复）：Alice 的见证 RM 抢在 Carol 直接告别帧之前到达 Bob 时，Bob 的 bye 处理器不得因『已不在名单』提前返回——直接接收者经 members ∪ left_voices（嗓音墓碑，A2 推导，见 theory.rulings.2026_10_17）验签后照常发出自己的见证 RM（幂等：条件删除空转 + 自己行照换，正是 rm_multiplicity 语义）；提前跳过仅对非见证（转发）接收者保留。bye 去重键 = 签名（sig 即事件 ID）。crash 路径 witness_ctx 同款 union"
      rm_presence_defense: "在场辩护：RM 点名者在 30 秒挂钟窗口内有已验签发言（判定输入 = 本地证据 ∪ 任何线到达的该成员近期已验签发言——泛洪天然补偿证据到达延迟）且与 RM 矛盾 → 扣下 topup + 响亮冲突 + 人类裁决。已见其署名告别 → 视为一致照常应用。『还在说话即不可被判离开』。窗口起点=帧到达时刻本地记账（不落盘、不进协议）；counter 水位语义废弃（不随沉默衰减，无法表达『最近还在说』）"
      rm_false_witness_honesty: "沉睡成员可被伪证 RM（见证人撒谎『我的线死了』）→ 临时驱逐，成员回来发言即 mumble → 点名回归。物理等价：你离席接电话时有人散布你走了——桌子重排，你回来发现桌变了。响亮、可恢复、入 known_limitations"
      add_layout: "0x0B + circle_id ‖ 'ADD' ‖ 新人行(自签: name‖r‖pk‖contrib_sig) ‖ 引荐人新行(自签, 含旋转后 r) ‖ introducer_sig(64B, 签 circle_id‖ADD‖新人行)。布局与 RM 镜像对称"
      add_fresh_r_p13: "引荐人旋转自己的 r（P-13 修复）：恶意载体存档密文 + 共谋候选人入座 → 若 ADD 不旋转任何 r，新人拿到 0x12 全表现行 r、排除自己即算出 K_old → 解密入座前历史。旋转后 K_old 缺引荐人的旧 r，无从推导。密钥边界=成员边界：RM 封未来于离开者，ADD 封过去于进入者（A3 两半合璧）。注：无共谋时旧窃听者论证仍成立（她拿不到任何现行 r）"
      add_semantics_ruling: "递话不代笔（settled 2026-10-15）：引荐人递的是新人自署的整行，伪造不了任何字段；她的签名只是引荐收据（问责：出问题时『谁带来的人』有答案）"
      exchange_algebra: "不相交变更序无关（T2：集合加减排代换天然可交换）；一切不可交换情形（同人二次变更、见证冲突、部分投递）必然退化为 mumble——响亮，交给点名。级联定时器、发起人选举、validity 规则：整体删除，不是换更好的，是不需要"
      pure_rekey: "无纯换钥 topup——密钥保鲜=点名（谁、何时、为什么换，是全桌的事，不是一个人的事）"
    transitional_window: "RM/ADD 应用不一致期间，滞后成员仍在旧钥下发言；刚拿到名单者可解滞后发言——物理等价：你走向桌子时有人正把话说完，你听见了尾巴。入档为已知边界，不修"

  admission:                                 # 入席（seat flow：0x10 → 0x11 → 0x0B ADD → 0x12）
    commands: "引荐人 /bring [拟名]（向聚焦对端发 0x10——聚焦对端即候选人）；候选人 /sit [名字]（落座，A1：落座署名是身体的行为，必须本人触发）。动词有向，各签各的：/bring 的签名落在 ADD 引荐收据上，/sit 的签名落在 0x11 自署行上。候选人主动想入座（拉模式）不需要新命令——1:1 里说『带我去那个圈』是社会信道手势，协议手势仍是引荐人的 /bring"
    sit_name: "/sit 默认取候选人在这条线上已自报的 0x05 本人名（A1：你的名字是你已经说过的话）；可覆盖（『这桌叫我阿花』是她自己的嗓音）。多个待接看座邀约 → 按 /talk 歧义处理模式列候选"
    expectation_gate: "/bring 带拟名时：0x11 回执的自署名与拟名匹配 → 自动完成（引荐人的手势已用 /bring 做完，剩下是机械收尾：泛洪 ADD + 递 0x12）；不匹配 → CircleConflict 响亮事件 + 回执丢弃（不缓存不悬挂——每次手势的协议效果完整）+ 候选人待座状态保留。引荐人想放行就重跑 /bring（改拟名或不带拟名），候选人再 /sit 应新 0x10——不新增命令、不新增状态、动词有向不破。物理等价：你说带 Alice 来，坐下的人自称 Bob——你收回名片，重新说一次『我带 Bob』。/bring 不带拟名 = 无门直接自动（她聚焦了这条线，她的直连感知就是她的验证，A2 允许）"
    closed_circle_bring: "封桌 /bring 语义（三条例子独立强制，非选择）：① P-13 → 0x12 必须携带旋转后的行（否则新人解入座前历史）→ 旋转是真实密钥状态变更，不存在『提议性旋转』（contrib_sig 签的是新 r，签不持有的 r = 签署假话）；② A1 → 签名即承诺（引荐收据+新行签名 = 你此刻的状态，要么别签，签了就生效）；③ A2 → topup 发起者的应用是发送原子的（与一切 topup 同构：见证人发 RM 也边发边应用，从不等确认——等批准 = 表决 = 同步机复活）。故：引荐人本地照常应用（旋转 r、加新人、换钥）并递出含旋转行的 0x12；其余成员各自响亮拒收（席位契约）；结果 = 引荐人+新人与圈 fork（mumble），点名重聚裁决。封桌是契约不是法律：协议不阻止 fork（阻止 = 共识 = A2 违反；物理世界也拦不住 Alice 坚持带人另开一桌），协议的职责是让违约响亮（全员本地拒收事件 + 双向 mumble = 圈子看见了这次尝试），裁决权在人类（点名：重新接纳 → 留线点名；甩掉她 → 先 bye 线）。新人的命运必然经引荐人——她对圈的唯一连接是引荐人的线，任何他人点名的会话快照里没有她（A2 一贯到底）。客户端警告但照发（拒绝是全员契约行为，不是发起人自我审查）；不引入表决"
    step_1_offer: "引荐人向候选人（1:1）发 0x10 看座：席位骨架（名字+pk，绝无 r——拒入者拿 r 即可算 K，P-09）"
    step_2_accept: "候选人自选 r（128bit+，碰撞可忽略）、生成嗓音钥、自签整行（0x11 落座署名）递回"
    step_3_add: "引荐人泛洪 0x0B ADD（新人自署行 + 引荐人旋转后新行 + 引荐收据——布局见 rekey.topup.add_layout，P-13 旋转）。开放台：任何成员的 ADD 有效（引荐即背书）；封闭桌：引荐人自己照常应用（发送原子，fork 语义见 closed_circle_bring），其余成员本地拒收 + 响亮（席位契约，不是表决）"
    step_4_roster: "ADD 应用后，引荐人向新人（1:1）递 0x12 座次表（全表含 r，每行可验——行旋转语义见 the_circle_object.row_rotation_semantics）——新人由此计算群钥。座次表只递成员"
    r_collision: "收端唯一性检查：r 与现有任何成员碰撞 → 响亮拒绝（P-12：r 碰撞若静默去重=无声驱逐；自选 r 下碰撞概率≈0，恶意碰撞被响亮拦截）"
    name_conflict: "同名到达 → 不再 first-seen-wins（那是抢注，A1+A2 违反）：先到暂定、直接对端的见证帧裁决、冲突响亮。见 theory.rulings"
    knows_vs_receipt: "知道了就是同意——签名是这张同意的收据。引荐发生在人类信道（你怎么知道这个圈的，是你的事）；入座发生在协议信道（谁签你进来的，是全桌的事）。系统遗忘、人类记忆——这次的记忆是问责线索，rekey 后连收据都忘掉"

  sas:                                       # 对词（settled 2026-10-15：群 SAS + 1:1 SAS；格式裁决 2026-10-16）
    what: "/word——1:1 聚焦时显示会话短词（Noise transcript hash 派生，双方各看各的屏幕）；在圈内时显示群短词（HKDF(K, \"circle-sas\") 派生）"
    format: "12 位数字分 4 组（123 456 789 012，约 40bit）——电话念 12 位分组数字是数十年验证过的仪式（人类工程学）；与 ZRTP 20-30bit 相比更保守（安全性），且每次聚首新鲜、不累积不持久"
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
# P4 盘点已执行（2026-10-17）：逐项状态标注在每条尾部——
# 【已死无码】= v3 slices 早已替换、src 无残骸；【已删】= 本轮删除落地；
# 【仍在运行】= 活残骸，走 decision_procedure 报裁决后才能动（owner 指令）。
# ───────────────────────────────────────────────────────────────

removals_3_0_0:
  version_window: "HEAD is post-P2 unreleased; the next release IS 3.0.0 (breaking: 0x09/0x0B/0x0C layout changes) — no 2.0.0 release is cut"
  v1_discovery_family:                       # 原 removals_2_0_0——本轮（2026-10-17）执行删除，全部【已删】
    shout: "udp lan discovery (probe/reply + listener + /shout) — was the ONLY unsolicited exposure source; circles absorb audience-finding。【已删 2026-10-17】discover.rs 仅存 local_ips+card；Person::shout/born 内 listener spawn 已除；/shout 指令已除；出生无喊话耳守卫钉死"
    hidden_mode: "born_hidden / --hidden / e2ee_person_create_hidden — existed only to dodge shout probes; ffi symbol stays as deprecated alias (ABI frozen add-only)。【已删 2026-10-17】born_hidden/--hidden 消亡（无喊话则无隐身语义）；ffi e2ee_person_create_hidden 保留符号、行为= create（deprecated alias 落地）"
    lan_broadcast: "the loudspeaker /say design — cancelled before birth (settled_decisions.rejected)。【已死无码】从未成码"
    listen_toggle: "the deaf/undeaf switch — evaporates with the loudspeaker。【已死无码】从未成码"
  sync_machinery:                            # v3 死刑——理论裁决（A2），不是实现取舍
    epoch_field: "epoch(u32) in 0x0B — 目录的残留；防重放由署名 counter 承担；或降级为纯显示代际标签。【已死无码】v3 slice-2 的 rm_payload/add 布局（circle_id‖'R'/'A'‖…）不含 epoch 字段，src 全文无 epoch 残迹——owner 担心的『活在 wire 布局里』不存在，无需裁决"
    initiator_election: "smallest-r initiator + 3s cascade + 'initiator named in frame' — 选举机制整体退役；见证人取代。【已死无码】smallest-r/级联定时器在 src 无残迹（v3 slice-2 起彻底替换）"
    validity_rules: "first-arriver-wins / same-leaver-no-op / serialize-by-arrival — 全部退役；T2 交换代数 + T1 响亮分歧取代。【已死无码】收端语义=幂等应用（条件删除+无条件行替换），无任何 serialize-by-arrival 残迹"
    fork_announcement_frame: "0x0E 分叉通告（曾计划的 prev-hash 链）——死于重建：分叉不预防、不通告，分叉必被听见（mumble），修复必可得（点名）。【已死无码】frame_kind 无 0x0D/0x0E 常量（死于理论，从未成码）"
    counting_admission: "expected = convos.len()+1 — 计数死；名单上的名字到齐才是完成（P-03）。【已死无码】form_circle 走署名邀请名单，src 无计数式准入残迹"
    unsigned_bye: "0x0C 无签名形态死——告别是身体的行为（A1），必须署名（P-01）。【已死无码】bye 处理器按长度门槛+验签解析，无签名形态结构性不可解析（v3 slice-2 替换彻底）"
    all_convos_flooding: "flood to all convos minus source — 信念路由取代（P-02/P-04）。【已删 2026-10-17】owner 裁决 rulings.2026_10_17_flood_frame_all_migration：等价性只是偶然为真（快照与发送两次独立锁获取间的漂移窗），0x0F 扇出迁移至 belief_flood（=快照绑定集，扇出追随誓言），flood_frame_all 整体删除——删除面清零。结构断言测试：快照后新线_零圈帧_从第一帧起（事件流可见不变量；漂移窗孤儿帧是 wire 层差异，收端被名单检查静默丢弃，红绿自证不适用——一致性修复非安全修复）"
  ffi_compatibility: "e2ee_person_create_hidden stays as deprecated alias; circle v3 events extend the enum add-only; 0x09/0x0B/0x0C layout changes are wire-breaking = exactly why this is 3.0.0。【已执行 2026-10-17】alias 落地（12 符号不变，契约测试照常绿）"

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
    add_rotates_introducer_r: "2026-10-16（P-13 修正）：ADD 携带引荐人 fresh_r 旋转她自己的行——原『ADD 不需要 fresh_r』只论证了圈外窃听者，共谋入座者可回读历史；修正后密钥边界=成员边界（RM 封未来于离开者，ADD 封过去于进入者）。经 attack_precedents 机制裁决，非重提案"
    topup_carries_resigned_row: "2026-10-16：RM/ADD 帧携带发起者重签的整行（~100B）——r 旋转后行失签，『每行自签』是公理级不变量（0x12 每行必须可验）；行结构三载体同构（座次表/ADD 新人行/topup 发起者行）"
    seat_commands_directed_verbs: "2026-10-16：命令面 = /bring [拟名]（引荐人，期望门：0x11 自署名不匹配则响亮暂停）+ /sit [名字]（候选人，A1：落座署名必须本人触发；默认本人 0x05 名，可覆盖）。动词有向，各签各的——同一动词给两个角色用会把问责线搅浑"
    presence_defense_wallclock: "2026-10-16：在场辩护『近期』= 本地挂钟 30 秒窗口（counter 水位语义废弃——不随沉默衰减，无法表达『最近还在说』）；判定输入 = 本地证据 ∪ 任何线到达的该成员近期已验签发言（泛洪补偿证据到达延迟）；已见署名告别 → 照常应用"
    sas_word_format: "2026-10-16：SAS = 12 位数字分 4 组（~40bit）；1:1 transcript 派生、群 HKDF(K, circle-sas) 派生"
    rm_witness_unified: "2026-10-16：见证人=离开的直接感知者，两条路径统一——crash=死线持有者，自愿告退=署名 0x0C 的直接接收者（一跳，非转发；转发者只应用不发起）；多直连对端各自发 RM（T2 不相交，幂等应用=条件删除+无条件行替换）；其余成员只应用不发起"
    expectation_gate_discard: "2026-10-16：0x11 回执与拟名不匹配 → CircleConflict + 回执丢弃 + 待座状态保留；放行=重跑 /bring + 再 /sit——不新增命令、不缓存不悬挂（每次手势协议效果完整）"
    punch_tag_tiered: "2026-10-16：tag 三层——省略→自动 128bit 上屏转告（Crockford base32）；显式短标签→响亮警告照用（降级自由）；警告写明 PAKE 补偿关系。与暗号自由的同构有一处修正：tag 猜测无需受害者在场（向介绍人枚举注册即可），与 PAKE 在场爆破模型不同构——故默认值必须强，降级必须知情"
    line_binding_direction_a: "2026-10-17：P-15 修复走方向 A——路由/裁决/见证只认圈协议建立的线绑定（三种写入行为：发起者快照/0x0F 送达/seat flow 双侧），their_name 降为纯显示；方向 B（自报名撞名响亮冲突）被否——检测式补丁、不覆盖 P-05/见证连带面。规格全文见 attack_precedents.P_15.fix（含两个负结果：0x09 不绑线、发言绑线惰性已弃）"
    closed_table_fork_semantics: "2026-10-16：封桌 /bring = 引荐人本地照常应用（发送原子，与一切 topup 同构）+ 其余响亮拒收 + fork + 点名裁决。三条例子独立强制：P-13（旋转必须真实）/A1（签名即承诺）/A2（发送即应用，等批准=表决）。封桌是契约不是法律——协议让违约响亮，不使违约不可能；新人命运必然经引荐人（他人点名的快照里没有她）。连带边界入档：座次表是复印件不是活视图（newcomer_roster_is_copy）；封桌 DoS 面极小（closed_table_dos_surface）"
    rogue_injection_feature_gated: "2026-10-16：测试注入面 = #[cfg(feature=\"rogue\")] pub mod rogue——默认构建编译期零注入符号（结构解）。doc(hidden) 被否，按本项目判例法：文档层遮掩+约定不调用 = 检测式编码（P-10 判例同族：PR_SET_DUMPABLE 优于 core_pattern 告警）；符号残留产物违反洁净交付纪律（Release 零日志同族）；『生产不调用』须为编译期事实而非君子协定。原语：rogue_frame（1:1）/rogue_flood（圈帧伪造走名单线泛洪=恶意成员真实路径）/circle_id()+roster_snapshot()（circle_id 非机密、随帧可见，正当 API）。注入经真实发送路径（每线 Noise 加密）——注入即恶意对端能力，T0 界外无攻击面增量。canary 测试：feature 缺失时恰一条测试响亮失败提示 --features rogue——静默跳过=罪。注意 #[cfg(test)] 不可用于此（integration tests 编译库时 cfg(test) 关闭）"
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
    fix: "递话不代笔：ADD 携带新人自署行 + 引荐人旋转后新行（布局见 group_v3.rekey.topup.add_layout；P3 slice-4）"
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
  P_13_newcomer_reads_history:
    attack: "恶意茶馆存档全部密文（转发路径上天然可见）→ 共谋候选人受邀入座 → 0x12 座次表递给新人全体成员现行 r → 她排除自己的 r 即算出 K_old → 解密入座前全部历史。原『ADD 不需要 fresh_r』裁决只论证了圈外窃听者，漏了入座者本人"
    root_cause: "密钥边界未对齐成员边界——RM 侧旋转了（见证人 fresh_r），ADD 侧不旋转 → 新旧 K 共享除新人外全部输入"
    fix: "ADD 携带引荐人 fresh_r（旋转她自己的行）：新人拿到的表里引荐人 r 已旋转，K_old 缺一块无从推导。密钥边界=成员边界：RM 封未来于离开者，ADD 封过去于进入者（A3 两半合璧）——发现于 seat flow 命令面过堂（2026-10-16），走 attack_precedents 机制，非 settled 重提案"
    test: "共谋场景：载体存档 + 新人入座 → 新人持 0x12 全表 + 存档密文无法解密入座前帧；同引荐人并发双 ADD → 后入座者对新话 mumble → 点名恢复（T2 相交变更落入 T1）"
  P_14_intra_row_sig_pk_consistency:
    attack: "伪造的 RM/ADD 帧携带『名字行换钥』——行内 sig_pk 与收端名单中该名字的现行 sig_pk 不一致（行自署可验，但签的是另一把钥）：冒名者借帧内的『见证人/引荐人』行顶替真成员的嗓音公钥，后续即可冒签发言/告别"
    root_cause: "A1——行名可伪、嗓音不可顶替；slice-4 实现时随 RM/ADD 收端检查落地（『行内 sig_pk 必须与名单一致』），判例占号追溯补录（owner 簿记 2026-10-17）"
    fix: "已随 slice-4 落地：RM 见证人行 / ADD 引荐人行的 sig_pk 与本地名单逐一比对，不一致=响亮拒收（外人冒见证人）"
    test: "slice-4 集成测试覆盖（RM/ADD sig_pk 一致性检查路径）；此条为占号补录，无新增面"
  P_15_self_reported_name_routing:
    attack: "圈外攻击者与任一在座成员建 1:1、在 0x05 自报该成员的名字（自报名密文内、无验证——telephone model 本义允许）→ belief_flood 线资格第一判定 belief.contains(their_name) 直接放行这条线 → 全部圈帧路由进攻击者自己的 Noise 会话：0x09 携带每个成员的 r，收齐即 K=HKDF(sorted r) → 静默旁听整圈（读权限全拿）直到下次换气。P-02 的重生面——P-02 修复的威胁模型隐含假设『圈外对端诚实自报名』。连带同根面（同一 their_name 依赖）：① P-05 直连声明裁决（line_peer==row.name）——自报成员名 + 自署新行即构成『直连声明』顶替暂定行，现行裁决机制可被操纵；② bye/crash 见证人判定（their_name==leaver / their_name∈members）——冒名线可冒领见证、对在座者发伪证 RM（无特权踢人复活）。发现于 2026-10-17 flaky 修复的④号检查项过堂：aliases 取证证明别名机制本身结构无碰撞（complete_bring 是唯一写点），真洞在基础路由第一判定条件"
    root_cause: "A2 违反（映射变体）：线→名字的资格映射从未经圈协议建立——their_name（0x05 自报名）直接充当路由、裁决、见证三类依据。物理世界不存在『自称某人即获得该人的桌位感知与作证资格』：感知只能来自亲手见证（她署名的公告、她被引荐的手续），不能来自自称"
    fix: "方向 A（owner 裁决 2026-10-17，LANDED 同日）：路由/裁决/见证只认圈协议建立的线绑定（line→名），their_name 降为纯显示。绑定只有三种写入行为，全部是协议行为、无一以自报名为资格：① form_circle 发起者快照——邀请名单本就扫描自发起者自己的线（自洽的感知宣誓，与署名 genesis 同责），快照随成圈写全她的线；② 0x0F 送达——把署名公告递到这条线的人绑为 creator（转发天然良性：受邀者经第二路径入座、同圈号同名单同材料同 K，同圈重放保护既有；伪造 genesis = 幻影圈——join 替换旧桌故无真圈泄漏、二人圈机密性等价 1:1、与真圈 /word 群词必不同，telephone model 边界内）；③ seat flow 双侧——complete_bring 绑引荐人侧候选人线（既有），0x10/0x12 到达绑候选人侧引荐人线（新人的出话路径）。负结果入档（防再提案）：0x09 凑话到达**不**绑线（转发与本人直达在收端不可区分，绑它即开抢注面——新鲜自署行+自报名即可顶替）；发言到达也**不**需要绑线（发言只从发送方已绑的线发出，永远不会落到未绑线上——推演中提出的『验签发言绑线』是惰性机制，已弃）。连带迁移（同根修复，随 slice 落地）：genesis direct-lock（creator==their_name）退役——递署名公告是协议行为，自称不是，0x0F 送达绑定取代之；P-05 直连声明判定与 bye/crash 见证人判定从 their_name 改读线绑定（未绑线 = 非直连声明/非见证——比现行更诚实：自报名从来不该是裁决依据；代价：顶替场景从『直连声明取代』退为『响亮冲突 + 点名』，本来的裁决就可被攻击者操纵，这是修正不是退化）。已知代价（入档 known_limitations.mesh_direct_pair_relay / relay_half_open_edge）：发起者之外的 mesh 直连对（双方都不是 creator、也不是 seat-flow 对端）在下次点名前只经发起者中继（双方线未绑，发言只走各自已绑线）；任一成员 /circle 即全量重绑（快照绑一方 + genesis 送达绑另一方）——A4 完整保留，mesh 冗余的恢复路径就是点名。方向 B（自报名撞名响亮冲突）被否：只堵路由面、检测式补丁家族，且不覆盖 P-05/见证连带面。落地形状（实现细节，rulings 2026_10_17_p15_line_binding_spec 两处钉死全遵照）：LineBinding 两形态——Name(String)（快照/bring 写入；路由过信念检查，RM 后随信念熄灭但保留供迟到告别见证判定 points_to；apply_add 同名重入清除）/ Qualified{via, peer}（0x0F/0x10/0x12 送达授予；spec a：名字不进路由判定；**peer 键修复（复审阻断项 B，rulings.2026_10_17_p15_peer_keyed_revocation）**：via=帧内授予者纯溯源，peer=授予时刻线对端 0x05 捕获快照——路由 Qualified{peer}∈belief、apply_rm 撤 Qualified{peer:离场者}、apply_add 同名清账含 Qualified{peer}、见证 points_to 读 peer；撤销键曾错位在 via 上=转发送达线在转发者离席时续命泄漏 RM 帧新 r，A3 回归，回归测试自证修复前红/修复后绿）；ADD 处理器的契约类拒收（名字冲突/r 碰撞/封桌——签名已验过的合法帧）照转（星型路由下静默丢弃会让下游成员看不见违约尝试，违反『让违约响亮』；伪造类拒收仍到此为止）；同圈创世重放检查显式补上（direct-lock 退役后的唯一重放闸，先于绑定——重放不得授绑）"
    test: "圈外线自报成员名 → 零圈帧到达（路由不看 their_name）；已除名者重放凑话/发言 → 线不获绑（P-04 断递保持）；星型成圈说话点名全通（快照+送达绑定全覆盖）；链式经逐跳已绑线中继；mesh 直连对：点名前经发起者中继互通、任一方点名后直达恢复；伪造 genesis → 加入者与伪造者成二人圈且与真圈 /word 群词不同（确定性断言）；冒名+自署行顶替（P-05 迁移面）→ 未绑线非直连声明 → 响亮冲突不静默顶替；冒名线断开 → 不冒领见证（无伪证 RM）。复审追加（2026-10-17 landing audit）：阻断项 B 回归——转发送达线（via=creator、peer=转发者）在转发者离席时熄灭 + 除名者零圈帧（含 RM 帧自身）+ 幸存者同钥对话 + 点名自愈重绑出话路径（P15_B_转发资格线_对端离席熄灭_除名者零圈帧，修复前红/修复后绿自证）；收尾项 C——v1 离场后 v2 同名入席，v1 线死时 crash 见证读已清绑定 → 无据 RM 不发、v2 无恙、v2 新绑定在清除后存活（P15_C_同名重入_crash见证不误逐新任期）；单元——对端键熄灭/授予键留溯源/过度撤销消失/同名清账（线绑定_生死周期_对端键熄灭_授予键留溯源）"

# ───────────────────────────────────────────────────────────────
# KNOWN LIMITATIONS — honest edges; do NOT paper over in docs
# ───────────────────────────────────────────────────────────────

known_limitations:
  irreducible:                               # 四条不可消除边界——物理的价，哲学的价，写进论文 limitations 是诚实加分
    fork_can_be_triggered: "分叉可被引发（A2：泛洪无全序的价）——但必被听见（T1）、必可修复（点名）"
    single_link_framing: "单链成员可被栽赃/锁出（A2：在场的边界=你的直连）——与 rm_false_witness_honesty 同根"
    carrier_metadata: "载体可见元数据（circle_id/流量/拓扑）——桌子的物理，茶馆继承"
    no_sas_no_theorem: "无 SAS 仪式则机密性定理无法陈述（T5）——对词是给愿意谨慎的人的工具，不对账是用户的自由"
  control_frame_length_shape: "控制帧长度形状未归一（P7 裁决①缓办归档，rulings.2026_10_17_p7_control_frame_padding_deferred）：0x0F/0x10/0x11/0x12/0x0C/0x13 帧长随名单规模变化，泄露面=圈子结构（成员数）——该面已被稳态流量分析结构性泄露（每发言的扇出模式对线路观察者恒可见），归一化封的是已开信道、无边际保密收益；有实质意义的指纹面（聊天内容长度推断）已由 0x0A 分桶填充封死（P-11/P7）。与 carrier_metadata 同族（不对称表第 4 行：桌子的物理，诚实入档不修）。重开条件：未来出现仅经控制帧长度可感知、流量分析不可达的信息差"
  left_voices_growth: "嗓音墓碑随离席累积（对抗 ADD/RM 循环可增长）——每轮受 seat-flow 成本约束，随圈生灭（点名重置、进程死亡），RAM-only 零持久化；接受为已知边界（rulings 2026_10_17_left_voices_tombstone ③）"
  mesh_direct_pair_relay: "P-15 线绑定代价：发起者之外的 mesh 直连对（双方都不是 creator、也不是 seat-flow 对端）在下次点名前只经发起者中继（双方线未绑，发言只走各自已绑线）；创作者星心离场=分叉静默（两幸存者的 RM 无路互达，各自单旋转不同钥）——mumble/T1 语义，点名即全量重绑恢复。A4 完整保留：mesh 冗余的恢复路径就是点名（rulings 2026_10_17_p15_line_binding_spec ⑤）"
  relay_half_open_edge: "转发送达的入座是半开边（复审收尾项 A 入档，见 group_v3.routing.relay_asymmetry）：B 经 M 转发的 genesis 入座只取得 B→M 出向资格，M→B 入向结构性无资格（M 侧三种绑定行为无一覆盖）——B 的成钥依赖她自己另一条被对端绑定的线；中继者离席后 B 的出向亦熄灭（peer 键）→ 发送面归零=听见不了=响亮（A4 点名重绑自愈，测试钉死）。同族：中继者若中途诚实点亮反向（对 B 发起 seat flow）则边转全通——机制已存在，无需新协议元素"
  p15_fake_name_peer_residual: "peer 键修复的残余面（pre-P15 同等暴露、非回归）——【P16 已修】pk 锚定落地（rulings.2026_10_17_p16_spec_approved）：谎报名线成键后严格档熄灭 + 零密封帧（P16 镜子测试：P16_镜子1_谎报名线_成键后熄灭，窗口围栏落地形态）；本条保留为窗口态的既有暴露记录（成键前窗口内谎报名线可收创世 + 0x09 行——owner 窗口围栏裁决已接受的 K-无密文价格）。原记录：peer 键修复的残余面（pre-P15 同等暴露、非回归，pk 锚定前不修——owner 确认 rulings.2026_10_17_p15_closure_granted）：资格线对端 0x05 谎报成员名 → 该线在被谎报名离席前持续路由（名字≠身份既定边界——谎报者须先持 0x0F/0x12 送达帧，成员泄帧/MITM(P-08)/共谋皆文档化边界）；谎报名 ∉ 信念的送达线永不路由（fail-closed，入座半途而废——pre-P15 同构）。结构性修复=嗓音钥线证明锚定 pk（P16_voice_key_line_proof，方向性批准 rulings.2026_10_17_p15_closure_granted，排 P4 之后、不阻塞 3.0.0）"
  p15_seat_flow_via_hint: "P-15 残余：候选人侧 0x12 建圈的 Qualified 取 0x10 到达线的自报名为 hint（0x12 无整帧签名）——peer 键修复后 hint 同时落 via（溯源）与 peer（撤销/路由/见证键）：引荐人自报名=她的行名（常态）时三判定全对；自报名≠行名（引荐人本人覆盖落座名的罕见合成）时该线 fail-closed（peer ∉ 信念 → 永不路由）——pre-P15 同构的 liveness 边角，点名自愈，无泄漏面。可随未来 0x12 整帧签名升级消除，暂入档不修"
  sleeping_member_false_rm: "沉睡成员可被伪证 RM 临时驱逐；回来发言即 mumble → 点名回归；入档不修（群钥固有+物理等价已论证）"
  newcomer_roster_is_copy: "封桌被拒的新人（或任何刚拿到 0x12 的人）手里的座次表是引荐人信念的复印件，不是活视图——她看到五个名字却只听见引荐人说话时，可能不知道自己被拒了，除非引荐人告诉她。物理对应：朋友拿到一张座次表，但那只是引荐人的说法。不修——修法是『拒绝通告帧』，那是同步机 creeping back；A2 推到极致：连你手里的座次表都只是别人的直连感知"
  closed_table_dos_surface: "恶意成员在封桌圈反复 /bring 的 DoS 面极小：第一次 fork 后，后续 /bring 都作用于她自己已分叉的信念（她的信念里已有 N₁，第二次加 N₂……），其余成员的对话完全不受影响，代价只是响亮事件和与她的 mumble。成员间对 open/closed 信念不一致时同理：各自应用/不应用——同一部机器（T1 fork → mumble → 点名），无需新机制"
  transitional_window: "RM/ADD 应用不一致期间滞后发言可被刚拿到名单者解密——走向桌子时听见话尾，物理等价，入档。注意边界：这是『应用期间』的秒级窗口；入座前的历史对新人不可读（P-13 旋转修复后成立——ADD 后引荐人 r 已旋转，K_old 无从推导），两者不可混淆"
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
    status: complete-2026-10-17             # slice-1/2/3 LANDED 2026-10-15；slice-4 LANDED 2026-10-16；slice-5/6 LANDED 2026-10-16；迟到告别竞态修复 + P-15 LANDED 2026-10-17；关闭裁决 rulings.2026_10_17_p15_closure_granted：P3 全板完成。71 测试：16 单元 + 54 集成 + 1 pq（跑法 --features rogue）
    depends: []
    engineering_debt:                       # slice-4 期间发现的既有 flaky（上游 slice-2 遗留，非 slice-4 引入）
      teardown_lock_fix: "已修（随 slice-4）：ear 收摊曾在持有 Inner 锁期间 await（writer 锁+TCP shutdown）——阻塞窗口内一切等 Inner 锁的任务（RM 处理等）卡死，见证人 RM 帧到达却无人处理 → 双方都只应用自己的 RM → rekeyed 缺一份 → 测试超时。修复：锁内纯同步收尾，shutdown/witness 移到锁外"
      pending_contrib_dedup_fix: "已修（随 slice-6）：早到凑话（0x09 先于 0x0F 到达）曾在暂存时就把去重键记入 flood_seen——join_genesis 补放时重入 handle_contribution 撞上去重被静默丢弃，乱序容忍机制名存实亡（名单永不齐、永不铸钥）。单运行时对局里 genesis 几乎总先到，时运掩盖了缺陷；slice-6 的牺牲线程对局（跨运行时调度）让 0x09 稳定先到，暴露此缺陷。修复：去重键只在确实处理（圈在且非暂存）时记录；补放走完整路径（含去重+转发），重复暂存无害（补放时第二份撞去重）"
      echo_order_test_fix: "已修（随 slice-6）：围坐一圈测试的裸 expect(heard) 在自回声竞态下确定性失败（Linux 沙箱调度下 Alice 自己的话经第三者线绕回，稳定先于对端直达帧——两帧在不同线上赛跑，直收序本就不保证；owner 机器时运相反故全绿）。改为按说话人 filter——slice-4 测试已确立的防自回声模式，断言语义不变"
      residual_flaky: "已定罪（2026-10-17，8 轮压力复现 + 插桩 witness=true 与失败 100% 逐次对应）：协议级竞态，非调度玄学——Carol 署名告退时，Alice 的见证 RM 抢在 Carol 直接告别帧之前到达 Bob → Bob 的 bye 处理器撞『告退者已不在名单（RM 先到）→ 静默跳过』提前返回 → Bob 的见证 RM 永不发出 → 第二份 rekeyed 事件缺失 → 15s 超时。单跑难复现（单运行时直接告别帧通常先到）/加打印不复现（时序扰动）/封桌点名类重拓扑才触发（仅 bye→双 RM 级联有此窗口）均由此解释。安全复核（owner 独立重推）：自愿路径因果结构保证至少一位见证者的 RM 必然发出——RM 的存在以 bye 已被处理为前提，两见证全灭需因果环路，不可构造——非安全破洞，是 rm_multiplicity 已定稿语义（『不能因已删而整帧丢弃』）的发端实现偏差。修复裁决见 theory.rulings.2026_10_17_left_voices_tombstone；crash 路径 witness_ctx 同款偏差同步修。已修（2026-10-17 同日落地）：left_voices 墓碑 + sig 键去重（验签后）+ 迟到直接告退的见证复活 + issue_witness_rm 见证义务幂等守卫（witnessed_departs，bye/crash/重放三路对同一离场只发一次 RM——裁决未明说的工程面，守卫是『照常发 RM（幂等）』的发端化）；迟到告别（延迟载体确定性构造：C→B 字节闸门，RM₁ 先到、直接告退后放行）与离场重入再离场（sig 键 + ADD 三清账）双集成测试落地"
      p15_b_race_tolerant_fix: "已修（2026-10-17，无 feature 编译验证会话）：P15_B 测试把两份诚实见证 RM（A 的 RM(Mallory) 走 A-B 载体锚线、M 的 RM(Alice) 走 M-B Proven 线）的竞速钉死为 M 先到（『Proven 出线竞速获胜』）——两帧在不同线上赛跑，直收序本就不保证（echo_order_test_fix 同族），本沙箱调度下 A 稳定先到，测试确定性红。修复：按 B 第一份 rekeyed 的 change 串分世界分支（判别可靠：apply_rm 的 change 只含离场者名『{leaver} 离席』，无见证人名）。M 先到世界断言原样保留；A 先到世界为镜像分叉（B 世界观 {A',B}）：拒收 M 的 RM（见证人 Mallory 已出信念）、B 的转发线随对端键撤销熄灭（**阻断项 B 修复的本体断言在此世界直接现形**——修复前 via 键续命泄漏 RM 帧新 r 的路径，此世界下泛洪义务结构性消失，M 零圈帧）、B-A 同钥互通；点名抚平两世界共用。判例语义不变：两侧各自正确、响亮、可修（T1）"
      fifty_star_scale_budget: "已定罪（2026-10-17，插桩取证：超时点 roster_snapshot 对比）：五十人星型测试在本沙箱 4/5 红（owner 机器全绿），根因是**规模×沙箱速度**，非协议缺陷——hub 名单满 50（中继无丢帧），leaf00 在 15s 预算耗尽时仅 19 行且持续增长：星型中继凑话 = 49×48 ≈ 2400 帧中继 + 三发 0x13 风暴全部串行过 hub 的单把 Inner 锁（ear 均 tokio::spawn 在测试 current_thread 运行时），本沙箱每帧操作慢约两个数量级 ⇒ 单叶凑齐需 30-60s，越过 expect 的 15s 单事件预算（预算与 N=50 均为已裁规格，未动）。TCP 环回同线保序 + 去重/暂存/锚定路径逐一核对，无丢失路径。处置：留档不改——若需沙箱全绿，需 owner 裁决规模档预算分层或中继批量化（越权即停，不自行发明）"
    slices:
      - "[x] slice-1 名单即圈：roster 行结构（name‖r‖pk‖contrib_sig）+ 0x0F 创世（署名邀请名单+seats）+ 名字到齐完成 + 信念路由（ Kills P-02/P-03/P-05 主体）"
      - "[x] slice-2 告别与补话：署名 0x0C + 见证人 RM topup（无 epoch/无选举/无级联）+ 在场辩护（近期验签发言 vs RM 冲突→扣下+响亮）（Kills P-01/P-06 主体）"
      - "[x] slice-3 点名：/circle 重敲=新 circle_id 重聚（与创世同构复用）；mumble 响亮呈现；无自动建议（Kills P-06 修复路径）"
      - "[x] slice-4 入席流（LANDED 2026-10-16）：0x10 看座（骨架无 r）/ 0x11 落座署名 / 0x0B ADD（自署行+引荐收据+P-13 旋转强制）/ 0x12 座次表（全表仅递成员）+ r 唯一性响亮检查 + 期望门（拟名不符→CircleConflict+回执丢弃+待座保留）+ 封桌 fork 语义（引荐人发送原子应用+其余响亮拒收）+ /circle 席位覆盖（form_circle_seats）+ 线别名（覆盖落座名的信念路由）+ RM/ADD 行内 sig_pk 与名单一致性检查（堵『名字行换钥』冒名洞——A1 推导，随 slice-4 落地）（Kills P-04/P-07/P-09/P-12）"
      - "[x] slice-5 对词（LANDED 2026-10-16）：/word 双层短词——1:1 从 Noise transcript 哈希 HKDF(\"p2p-sas\") 派生（per-convo transcript 捕获于 noise_stage）、群从 HKDF(K, \"circle-sas\") 派生（circle.rs sas_word_from_*）；12 位数字分 4 组；即时显示零落盘（Kills P-08 的可检测性）"
      - "[x] slice-6 判例测试矩阵（LANDED 2026-10-16）：注入面 #[cfg(feature=\"rogue\")] pub mod rogue（rogue_frame/rogue_flood/circle_id/roster_snapshot/forge_row，经真实发送路径）+ 判例集成测试（P-01/P-03/P-04/P-05/P-08/P-12 拒收或检测）+ P-13 确定性单元断言（旋转封旧史）+ 交换代数断言（不相交序无关收敛 + 同旋转者相交必分叉→T1）+ 四点链式中继 + 转发告别只应用不发起 + 在场辩护扣下路径（牺牲线程 crash 模拟）+ 一人开台收客 + 50 人规模 + canary（缺 feature 恰一条响亮失败；CI 已带 --features rogue）"
  P4_removal_face_3_0_0:
    status: code-complete-2026-10-17           # 删除面清零：v1 发现家族【已删】+ 同步机残骸盘点（六项已死无码）+ flood_frame_all【已删】（rulings.2026_10_17_flood_frame_all_migration）+ ffi alias + human READMEs 重写。剩发布杂务（tag 3.0.0 / release notes / CHANGELOG——owner 建议切出：破坏性变更全部在位，P5/P6/P7/P16 皆增量）
    depends: [P3_v3_axiom_rebuild]
    scope: "删除 shout/hidden/lan listener（v1 家族）+ 同步机全部残骸（epoch/initiator/validity/cascade/计数完成/无签名 0x0C/全对话泛洪）；ffi create_hidden → deprecated alias；README.md/README_ZH.md 按 v3 世界重写；版本 3.0.0"
    landed_2026_10_17: "盘点（removals_3_0_0 逐项状态标注）：同步机六项全部【已死无码】（v3 slices 替换彻底，epoch 不在 wire 布局——owner 担心不成立）；v1 家族【已删】：discover.rs 仅存 local_ips+card、Person::shout/born_hidden/born listener spawn 删除、main.rs --hidden + /shout 删除、lib.rs Discovered 导出删除；ffi e2ee_person_create_hidden = deprecated alias（12 符号不变，契约+ctypes 绿）；测试：隐身 test 随功能退役，出生无喊话耳_删除面守卫补位；README.md/README_ZH.md 按 v3 世界全量重写（doc_index.rule 时点已到）。第二轮（同日）：flood_frame_all 迁移落地（0x0F 扇出走 belief_flood=誓言集、函数删除、快照后新线_零圈帧_从第一帧起结构断言）——removals_3_0_0 全部条目清零"
  P5_android_abi:
    status: closed-2026-10-17                # aarch64 owner 直测通过（独立构建复现 12 符号/16KB 对齐/契约三绿——rulings.2026_10_17_p5_completion_scope）+ 完成面同日落地：三目标构建/CI 断言/CHANGELOG/README 命令表同步——四 ABI 全交付，P5 关闭；release artifacts 上传随 v3.0.0 tag（发布杂务）
    scope: "cargo-ndk + NDK r28+ (16KB page alignment — verify LOAD p_align via llvm-readelf); targets aarch64/armv7/x86_64/i686-linux-android; api 24; verify 12 e2ee_* symbols per .so; GitHub Actions job + release artifacts; pure-rust deps only"
    landed: "aarch64（第一目标，owner 会话指令 2026-10-17）：构建链 = NDK r28b（28.1.13356709）+ cargo-ndk 4.1.2 + rustup target aarch64-linux-android，`ANDROID_NDK_HOME=<NDK r28+> cargo ndk -t arm64-v8a -P 24 -o <dir> build --release -p e2ee-ffi`；src/ffi/include 零 diff——打包面纪律自证：无任何代码层妥协，链接选项零添加（r28b 默认 LOAD max-page-size=16KB，无需 -Wl,-z,max-page-size 旗标）；唯一 C 面=pqclean Kyber（snow fork）经 NDK clang 编译通过，其余依赖纯 Rust 构建自证。自测面（本侧沙箱，2026-10-17）：① .so 形态 = ELF64 / AArch64 / DYN，1,031,688 B；② nm -D 计数 e2ee_* T 符号 = 12（含 create_hidden 别名）✓；③ llvm-readelf -lW：四条 LOAD 段 Align 全 = 0x4000（16384，16KB 页对齐）✓；④ host 契约复跑（同源构建面）：cargo test -p e2ee-ffi round-trip ✓ + gcc ffi_test.c 'FFI OK'（两人全程 C ABI 对话、错暗号响亮、暗号链、hidden 别名）+ python3 ctypes_test.py 'ctypes OK' ✓。构建物交付（workspace 产物目录，git 不收二进制——release artifacts 随 tag 上 GitHub Releases）：arm64-v8a/libe2ee.so（sha256 67b10bd241810da9bcaaed9877bf0a25740041ba7213fb08ff462d3fd6ddaca6）+ include/e2ee.h 副本。收尾 pending：owner 直测两项（llvm-readelf p_align / nm 12 符号，命令见 verify.android_build）复核后 P5 关闭；其余三目标（armv7/x86_64/i686）+ GitHub Actions job + release artifacts 未开工——本次会话指令范围=aarch64，排期 owner 裁决。完成面（同日 owner 会话指令，rulings.2026_10_17_p5_completion_scope）：aarch64 直测通过（owner 独立构建复现 12 符号逐项一致/四条 LOAD 全 0x4000/契约三绿；sha256 非字节复现性已裁决——发布物身份=随 release 交付的二进制及其 sha）；三目标构建落地（同构建链，35-45s/目标）：armeabi-v7a = ELF32/ARM、812,664 B、12 符号名单一致、四条 LOAD Align 实测全 0x1000（4KB——NDK r28 对 ELF32 默认，无 16KB 页的 32 位设备形态，按裁决注记如实入档、不强行加旗标）；x86_64 = ELF64/AMD64、1,285,784 B、12 符号一致、四条 LOAD 全 0x4000 ✓；x86 = ELF32/80386、1,275,616 B、12 符号一致、四条 LOAD 实测全 0x1000（同 armv7 注记）。四产物 sha256：arm64-v8a 67b10bd2…6aca6 / armeabi-v7a 0f37804f…a3981 / x86_64 4d63a8bc…e121 / x86 d29c960b…b320（清单 SHA256SUMS.txt 随产物目录交付）。CI：.github/workflows/android.yml——四目标矩阵（fail-fast=false），NDK r28b 发布物直装（与本地验证同一 zip，不依赖第三方 action 版本漂移）、cargo install cargo-ndk --locked、cargo ndk 构建、两项直测脚本化常驻断言（符号名单 diff + LOAD p_align 阈值：64 位 16384/32 位 4096）、artifact 上传；owner push 后看首跑绿灯。CHANGELOG.md 新建（3.0.0 条目按本文件 landed 链撰写，破坏性变更置顶）；README.md/README_ZH.md 命令表同步（审计缺口：/circle 选择性开圈可选参 + /punch 标签可省略）。P5 关闭；release artifacts 上传随 v3.0.0 tag（发布杂务）"
    parallelizable: true                     # no dependency on P3/P4
  P6_nested_circles:
    status: closed-2026-10-17                # LANDED + 矩阵十条全交付（共线中继连通性自愈收尾）+ owner 侧独立复验（87 全绿 + 10 轮压力 0 flaky）；落地记录见 landed
    depends: [P3_v3_axiom_rebuild, P16_voice_key_line_proof]  # P16 基线：内圈判定从第一天 pk 锚（无名字窗口捷径）
    scope: "选择性开圈（subset form = /circle 名1 名2 …，空参=全部快照向后兼容；唯一精确匹配，歧义响亮报错）+ Gathering 按圈容器化（迭代序确定性，进程内状态非跨节点注册表）+ 共享线多圈绑定（线=物理对话，绑定/锚按圈独立）；零新帧（0x0F/0x09/0x10/0x11/0x12/0x0B/0x0A/0x13 全复用）；钥匙独立双推导（A3）；协议状态里不存在圈与圈的关系（嵌套=人类语义）；判定函数签名零改动（&VerifyingKey 嵌套就绪）；测试矩阵十条+三增补；carrier 元数据诚实入档"
    landed: "实现（复用清单逐项）：gatherings: BTreeMap<u64, Gathering>（裁决④迭代序确定）+ active_circle 当前座位指针（最近开桌/入席的圈——无参动词 /gsay·/word·/bring 落座这里）；45 处访问点逐圈上下文化（ear 各处理器局部 circle_id 直取；belief_flood 按载荷圈号查容器——函数体零改动；send_line_proofs(inner, cid)；complete_bring/issue_witness_rm 按挂账圈号；g_check_r_collision 加圈参）；witness_ctx 改按圈收集——共享线死=对每个把这条线绑进自己圈的圈各自见证（A2 一跳感知按圈取用，义务幂等守卫兜底）；判定面原样复用（routes/witness 推导/apply_rm/apply_add/anchor_line_proof/proof_payload 签名未动——P16 嵌套就绪实证）；动词面 circle_speak/sas_word/bring 无参=当前座位（单圈行为不变）+ circle_speak_in/sas_word_in 显式变体；subset form=form_circle_subset(targets, seats)（唯一精确匹配，NoExactMatch/AmbiguousMatch 响亮；空参 /circle=全部快照+放下旧桌〔A4〕，subset 开圈旧桌保留；CLI /circle [封桌|开台] [名1名2…]）；圈类事件加 circle_id（端到端：Rust 枚举→构造→CLI 渲染→test helpers→ffi 载荷 E2eeEvent.id 值复用〔C 头/布局/签名零变更〕+ round-trip 断言钉死；1:1 事件面零污染）；ffi 零新事件类型零新 C 函数。测试矩阵（8 集成新增 + 映射）：P6_K独立_内外互不泄漏_圈外零帧（矩阵1+4：K 独立+P-02 内圈版全类零帧）、P6_外换钥_内圈照常（2）、P6_内圈入席流_solo_subset_全走位（5：空 targets=solo 舞台→bring/sit→Rekeyed(3)）、P6_内圈独立点名_外圈无感（6）、P6_纯内圈成员_离席_外圈无感（7：平行事实可行面；外逐内留完整构造受同名多线 subset 解析歧义边界限制，入档）、P6_深度2_圈中圈_SAS相异_flood跨圈（8+11+12：三圈圈号互异/SAS 两两相异/同内容跨圈不去重）、P6_多圈RM_共享线告退_两圈各自换钥（13+审计封口：恰两 Rekeyed 圈号互异各点名 Bob）、P6_pk锚按圈_圈间独立（9：双圈绑定全锚、分属两圈账本，rogue）；P6_共线中继_两圈经一继_连通性自愈（10：C 经代理中继独线入两圈——两圈帧经中继互达、A—B 段断开仅外圈 RM 内圈无感〔多锚独立撤销面的镜像〕、空参点名重聚恢复）；单元=ffi round-trip（①条件）。红绿自证（分层定罪）：动词歧义层红实证——容器化初版无座位指针，7 条既有判例确定性红（AmbiguousCircle{2}：点名=插入非替换，受邀者持旧圈致动词歧义）→ active-seat 修复全绿；分发层红证由既有判例网承担（P-02/P-15/P16 全绿=圈头分发正确性；查无此圈静默语义未被本 slice 还原测试——其红绿由 P-02 内圈版零帧断言正向钉死）。已知边界（诚实入档）：点名受邀者的旧桌滞留为 RAM 僵尸（零持久化，进程死亡即清——跨圈清除需圈间关系=宪章禁止，不修）；外逐内留完整构造受同名多线 subset 解析歧义限制（唯一精确匹配的代价，点名自愈）；外圈-only 成员可经共线者观测内圈流量存在（carrier_visibility 嵌套延伸，密文不可读）。验证（owner 侧独立复验 2026-10-17）：87 = 17 单元 + 69 集成 + 1 pq 全绿、canary 恰一响亮、ffi round-trip ✓、**10 轮压力 69×10 全过 0 flaky**（36.4-36.6s/轮）；共线中继测试收尾后全集 88 = 70 集成 + 17 单元 + 1 pq（本侧复核 无 feature 49+canary / rogue 68+1 新增 ✓）——79 基线不回退；P6 关闭"
  P16_voice_key_line_proof:                  # 嗓音钥线证明（P-15 的 A1 完成态：名字→身体）——方向性批准 rulings.2026_10_17_p15_closure_granted
    status: closed-2026-10-17                # LANDED + 落地审计通过 + 关闭裁决 rulings.2026_10_17_p16_closure_granted（分层定罪红绿判据追认；簿记三处随 slice 捎带）；落地记录见 landed
    depends: [P4_removal_face_3_0_0]        # 排 P4 之后、不阻塞 3.0.0（发布后跟进）
    scope: "成员以圈嗓音钥签 circle_id‖线 transcript 哈希，收端按名单行 pk 验证并锚定绑定到 pk——撤销/路由/见证按 pk 而非名字（谎报名〔p15_fake_name_peer_residual〕与名字复用面同时消失）；their_name_residuals（circle_bye targets 读 their_name）同批处理——同族代码一次触碰（owner 裁决）；属新协议元素（新帧或送达包装），设计走 decision_procedure 全流程过堂后开工"
    landed: "实现（spec §7 范围逐项）：LineBinding 三变体均带 proven_pk: Option<VerifyingKey>（Name/Qualified 窗口态 None，Proven 新变体=未绑定线授予——裁决点②）+ anchor_pk() 统一读点；0x13 LINE_PROOF 收发（载荷 circle_id(8B)‖sig(64B)，签名载荷 crate::circle::proof_payload = circle_id‖本线 transcript(32B) 定长定界——跨线重放结构性失败 INV-P16-1；收端按 members∪my_row 逐行试钥恰一行通过 ⇒ 升级 proven_pk/授予 Proven，全败安静=无后果行为；暂存 pending_proofs CAP 64 + join_genesis 补放〔凑话补放后跑，名单就绪〕）；三发幂等（凑话后 form_circle/join_genesis/complete_bring + 成键后 try_complete 尾 + 点名后重开桌同路径）；四类判定迁移按 pk 读——belief_flood 走 routes(belief_names, belief_pks: &HashSet<[u8;32]>, keyed: bool)（签名收 pk 集），witness_ctx 锚 pk→行/墓碑→名字，circle_bye targets 同构，apply_rm 急切撤 anchor_pk==删行 pk + Qualified{peer:离场者}（行删⇒线熄单一机制）；严格档切换 routes() keyed=true ⇒ 名字锚线 return false（窗口围栏：创世+0x09 可达、密封载荷成键后不可达）；ffi/include 零变更零新事件（git diff 空）。their_name 重对账（复审会话全仓 rg 重跑，2026-10-17）：src 命中 34 处逐一分类=注释 10/显示面 14（talk_to·RosterEntry·SAS 报告·事件载荷）/捕获面（Qualified.peer=0x05 快照——P15 裁决设计本体）/测试对照（line_bindings_debug）——**判定路径残迹 = 0（含 circle_bye targets）**。测试矩阵（6 集成 + 单元，映射授权编号）：镜子1=P16_镜子1_谎报名线_成键后熄灭（授权测试 1，零帧断言显式覆盖密封帧类 Heard/CircleMumble+占位一致性守卫）、镜子2=P16_镜子2_同名复用_pk锚不混淆（授权测试 2）、P16_撤销统一_行删即熄_幂等重放无害、P16_circle_bye按pk_冒名线零圈告别帧（their_name_residuals 之死）、P16_半开边点亮_证明授予（裁决点②）、P16_乱序容错_点名后三发重锚_双向恢复（裁决点③）；单元=proof_payload 定界三断言（错 transcript/错圈号必败）+ 严格档窗口熄灭两断言（裁决点①）+ 线绑定生命周期（proven_pk 撤销）。红绿自证（复审会话执行入档）：镜子2 红——witness_ctx 改回按名字查 + apply_rm 锚撤禁用（P15 语义双还原）⇒ CircleRekeyed{2,\"Carol 离席\"} 无据 RM 误逐 v2（assert_quiet 捕获，0.9s），还原后绿（4.4s）；镜子1 红——单关严格档 4 跑全绿（冒名者为自证所发 0x13 使其线升级 pk 锚，出局由 pk 判定〔行删即熄〕兜底封面——严格档=纵深第二层）⇒ 复合还原（关严格档+关锚授予，冒名者停留窗口态）7 跑第 3 跑红（密封帧到达出局冒名者，76.1s），集成级红被 fake 结构性占位序压制〔她无旧圈状态，contrib 稳定先到——时运依赖，机制级红由严格档单元断言钉死〕，还原后双绿（7.7s）。验证（2026-10-17 复审会话）：rogue 全套 79 = 17 单元 + 61 集成 + 1 pq ≥ 72 基线不回退；canary 契约双态复验（缺 feature：42+1 canary 响亮失败恰一条；带 rogue 全绿）；10 轮压力循环（并行默认负载，skip 五十人）**60×10 全过 0 flaky**（38.2-38.5s/轮，方差 <0.6%）；五十人_成圈说话点名 环境规模预算已定罪留档（engineering_debt.fifty_star_scale_budget：O(N²) 星型中继×沙箱慢速，非协议缺陷，owner 机器全绿）"
  P7_hardening:
    status: closed-2026-10-17                # f5b6bba 四项落地 + owner 审计通过（复验全绿：20 单元/rogue 70 集成/无 feature 50+canary 设计性失败/双裁决逐字/YAML）+ 裁决①缓办归档 rulings.2026_10_17_p7_control_frame_padding_deferred（边界入档 known_limitations.control_frame_length_shape）——P7 关闭
    depends: []
    parallelizable: true
    landed: "四项逐项（顺序=裁决建议 padding→tag→dumpable→水位）：① 密文填充三层框架（P-11 分桶填充）——circle::padded_len 档位 [256,512,1024,2048,4096,8192,16384,32768]，seal 载荷=真长 4B‖明文‖桶填充（外层只露档位），open 剥前缀还原；档内 8..=16 随机抖动破『同档同长』二次指纹；顶档之上=max 层原长+抖动；作用域=圈密封帧（0x0A 泛洪观测面——1:1 Noise 信道无中继观测者不入此面）。② courier tag 三层规则（CLI 面）——/punch 标签可省略：省略→自动 128bit Crockford base32（26 字符 5-5-5-5-5-1 分组上屏，经介绍人转告）；显式标签照用+<16 字符响亮警告（PAKE 补偿关系写明——tag 猜测无需受害者在场）；词对+nonce+role 的帧内组合由 tag 字符串承载（打洞协议帧内重排未动——改动=打洞协议变更，如需另行报裁决）。③ PR_SET_DUMPABLE=0（P-10 结构解正译）——harden_process 增 prctl(PR_SET_DUMPABLE,0)：RLIMIT_CORE 拦不住 core_pattern='|' 管道收集器，DUMPABLE=0 让 core 从一开始不产生+ptrace 附着被禁（ssh-agent 同款）；RLIMIT_CORE 留作腰带背带；测试=TracerPid=0 恒验+Dumpable 条件断言（本沙箱内核 status 无 Dumpable 行——已探针 prctl ret=0 证实生效，条件式断言入档）。④ counter 水位窗口——Gathering.counter_watermark: 每发送者 tag→已应用最大 counter；水位下早帧（重放/定位噪音）静默丢弃（噪音无传播权），水位上新帧照常；换气清账（try_complete/apply_rm/apply_add——新钥新计数空间）；纯函数 circle::counter_admits 单元三断言。测试：单元 3 新增（pad 同档同长+跨档升位+档位定值、pad roundtrip 真长还原七长度、水位 admits 四断言）+ dumpable 集成 1；红绿自证（padding 镜子，分层定罪）——填充禁用单层还原 ⇒ 『同档同长』断言红实证（外层长度泄露明文长度，left≠right）→ 还原绿；水位为纵深第二层（第一层=flood_seen 去重既有判例），单层还原场景与 P16 镜子1 同构读法已入判据。验证：无 feature 20 单元+1 dumpable+50 集成+canary 恰一响亮+pq ✓；rogue 20 单元+1 dumpable+69 集成+pq ✓（五十人=已定罪环境项）；ffi round-trip ✓；88 基线不回退；10 轮压力随 owner 侧复验执行。关闭（2026-10-17 同日，owner 会话指令）：f5b6bba 审计通过——owner 侧独立复验全绿（20 单元 / rogue 70 集成 / 无 feature 50+canary 设计性失败 / 双裁决逐字 / YAML）；commit 消息逐项据实（纪律遵循确认）。裁决①缓办项归档：『帧类语义公共长度集（0x0F/0x10/0x11/0x12/0x0C/0x13 归一）』实现落场为仅 0x0A 密封帧分桶填充——落差裁决控制帧长度归一化缓办（rulings.2026_10_17_p7_control_frame_padding_deferred：控制帧长度泄露面=圈子结构〔成员数〕，已被稳态流量扇出结构性泄露，归一无边际保密收益；与不对称表『载体可见元数据=诚实入档，不修』同族），边界入档 known_limitations.control_frame_length_shape；verify 计数同步 92 = 70 集成 + 20 单元 + 1 pq + 1 dumpable（bin）"
    scope: "PR_SET_DUMPABLE(0)（P-10，结构解）；punch tag 三层规则（省略 TAG→自动 128bit 上屏转告（Crockford base32 分组）；显式短标签→响亮警告照用，文案写明 PAKE 补偿关系）；padded_chat 分桶填充（P-11）；counter 去重=最高水位+有界乱序窗口（规约定死）"
  P17_release_hygiene:                       # 发布前清场（owner 立项 2026-10-17，tag v3.0.0 阻塞项）
    status: landed-2026-10-17               # 六笔静态残迹处置（①keep / ②-⑥remove）+ 扩展扫描清零（含 1 项修复）——待 owner 审计，审计通过即 tag
    depends: []                             # 与 P5 同族的打包面收尾：零协议变更、零 wire 变更、测试面零改动
    scope: "owner 预扫描六项逐项取证复核（初判可能错——①复核后改判 keep）+ 扩展扫描（allow 面 / cfg(test) / features 默认面 / dev-deps 泄漏 / 线程与 env 探针 / CLI 动词双语对账）；红线清单逐一保留：unreachable!x2、swaps 提醒、PR_SET_DUMPABLE、短标签响亮警告、canary 契约、RLIMIT_CORE 腰带背带、全部裁决引用注释"
    landed: >-
      处置表（①-⑥ = owner 表序）：
      ① Qualified.via——KEEP（复核后改判处置方向）：零判定读确认（一切判定路径
      matches!(Qualified{peer,..}) 只读 peer，P16 后判定全按 pk 锚）+ wire 零字节
      （line_bindings 纯内存、从不序列化）⇒ 删除在技术上合格；但 via 是 P-15 spec-a
      「creator 身份来自帧内签名而非线」（rulings.2026_10_17_p15_line_binding_spec
      spec-a）的唯一结构性证人——转发送达时 peer 合法地就是转发者，区分「正确记录
      帧内 creator」与「误绑转发者」的可观察记录只有 via；删除须改写已关闭 slice 的
      断言面（P15_spec_a 断言 qualified:via=Alice peer=Mallory；P15_B 2705-2711 同
      族）且改写后无法验证同一不变量 = 「测试语义不得弱化」红线 ⇒ 留。警告消除走
      结构性表述：cfg_attr(not(feature="rogue"), allow(dead_code))——唯一读者是
      rogue 断言面 line_bindings_debug，豁免精确限定在读者缺席的构建形态（rogue
      构建下字段活、不豁免）——非君子协定；字段注释承载完整裁决链。测试面零改动
      （spec-a/B 断言原样通过，全量复跑实证）。
      ② frame_kind::CIRCLE(0x04)——REMOVE 常量+注释：零引用 v1 残骸（CONTRIB 0x03
      有 ear 读点故留）；「帧编号不复用」档案归 wire_protocol.circle_chat（0x0D/
      0x0E 先例：历史帧号留档文档不留代码）；rogue 判例族洁净交付纪律。
      ③ person.rs:1245 多余 mut——REMOVE（circle_bye_frames 对 g 只读）。
      ④ person.rs:1925 未用局部 circle_id——REMOVE（块内全用 bring.circle_id 直
      取；不走 _circle_id 改名——变量无存在必要）。
      ⑤ seats/table——拆三处置：Gathering.seats 的 #[allow] 过时（读点=看座 0x10/
      座次表 0x12 随帧/ADD 封桌契约 3184）⇒ 删 allow+更正注释、字段留；
      SeatOffer.seats 与 SeatOffer.table 零读取（读点 grep 证：仅 circle_id/
      introducer/my_seat）⇒ 删字段+写点；0x10 的 seats 验形（>SEATS_OPEN 拒收）与
      桌名入 SeatOffered 事件的局部 table 保留——wire 解析零变更。
      ⑥ courier.rs ROOM_LINGER——REMOVE 常量+占位块（撤房路径即时、无宽限挂载）；
      「房随线生，线断房灭」文档语义纯注释化落在等位簿声明处（owner 处置方向第
      二支）。
      扩展扫描（owner 未覆盖面，全部清零）：allow 面终态=仅存 person.rs 相遇落座
      的 clippy::too_many_arguments（API 形态决策+注释依据，留）与 ①的 cfg_attr
      （精确条件豁免）；#[cfg(test)] 全部合法（测试模块 + Gathering::headcount 单
      元助手）；[features] rogue=[] 无 default 键（不入用户构建，确认）；
      dev-dependencies 全仓零条目（root+ffi；vendor/snow 的 dev-deps 是上游测试
      脚手架——依赖的 dev-deps 永不入发布编译）；std::thread::sleep / env::var /
      dbg! / todo! 零命中；unreachable!x2 = 红线保留（「Proven 恒有锚」响亮不变
      量）。CLI 动词对账：handle_command 15 动词与 README.md/README_ZH.md 命令表
      逐行一致 ✓；扩展发现——未知指令回退提示只列 5/15（v1 时代残句）⇒ 修为全
      列（响亮且准确）。
      验证：三 target 零警告（root lib+bin / ffi cdylib / rogue 构建，真实重编
      22.8s/13.5s/23.1s，grep warning=0）；92 基线不回退：rogue 全量 20 单元+1
      dumpable+1 pq+70 集成（69 绿+五十人环境红=已定罪项 fifty_star_scale_
      budget），无 feature 50 绿+canary 恰一红。四 ABI 重建（清场后发布物身份
      变更——旧 sha 作废、P5 landed 的 sha 表随本记录 superseded；发布物身份
      原则 rulings.2026_10_17_p5_completion_scope 适用）：四 .so 仍各恰 12 个
      e2ee_* T 符号、64 位 LOAD 全 0x4000 / 32 位实测 0x1000 不变；新 sha256——
      arm64-v8a 7abb6611…6526（1,031,480 B）/ armeabi-v7a ad3c67a5…fcd3
      （812,408 B）/ x86_64 94c1469c…d37f（1,285,288 B）/ x86 ff05f637…92ee
      （1,274,880 B）；SHA256SUMS.txt 重出（5fab87b9…0d4f）随产物目录交付。
  P15_line_binding_routing:                  # P-15 方向 A（owner 裁决 2026-10-17；落地复审 2026-10-17 闭环；关闭裁决 2026-10-17）
    status: closed-2026-10-17               # LANDED + 复审三项闭环（rulings.2026_10_17_p15_landing_audit）+ 关闭裁决（rulings.2026_10_17_p15_closure_granted：B/A/C 全接受，P-15 关闭，P3 全板完成；自证红/绿入档为安全修复落地标配；『Noise remote static』勘误；pk 锚定方向性批准 → P16）。71 测试：16 单元 + 54 集成 + 1 pq
    depends: []
    landed: "person.rs：enum LineBinding{Name,Qualified{via,peer}}（points_to 归一判定——Qualified 读 peer）+ Gathering.line_bindings（取代 aliases）；belief_flood 只认绑定（Name 过行名信念检查；Qualified 过对端键 peer 信念检查——授予=协议行为，熄灭=信念检查）；form_circle 快照全量绑定；GENESIS 处理器 direct-lock 退役 + 同圈重放闸（先于一切）+ join_genesis(src_line) 送达绑定 Qualified{via:帧内 creator, peer:送达线 0x05 捕获}；handle_contribution 签名改 src_line（暂存保线号——P-05 直连声明判定与转发排除读它，非逆向路径学习）+ P-05 直连声明读 Name 绑定；complete_bring 绑 Name(0x11 行名)；handle_seat_roster 绑 Qualified{via:引荐人 hint, peer:hint}；bye/crash 见证判定读 points_to（peer）；apply_rm 急切撤 Qualified{peer:leaver} + apply_add 同名清账 Name+Qualified{peer}（先于引荐人写新绑定）；ADD 契约类拒收照转。测试：单元（线绑定_生死周期_对端键熄灭_授予键留溯源——含过度撤销消失断言）+ P15_圈外自报名_零圈帧 + P15_spec_a_转发创世（确定性载体构造：直发被扣→转发送达→放行重放不授绑）+ P15_spec_b_前成员重放（mortal proxy 杀线不杀进程：僵尸桌 bring 幻影圈 + 旋转不变量断言 + 旧材料无钥 + P-03 拒收）+ P15_B_转发资格线_对端离席熄灭_除名者零圈帧（复审阻断项 B 回归：转发者离席熄灭+RM 帧不泄漏+幸存者同钥+点名自愈）+ P15_C_同名重入_crash见证不误逐新任期（复审收尾项 C：v1 线死无据 RM、v2 新绑定存活）+ 既有测试按新拓扑语义重写（署名告别→创作者离场双见证/点名点亮 B-C；在场辩护→唯一绑定线见证；点名→单 RM 收敛；迟到告别→双代理线确定性构造）"
    landing_audit_2026_10_17: "owner 复审：主体接受、三附带修复追认、union 同构论证修正（crash 的 union=幂等冗余见证，义务守卫兜住）；阻断项 B（via 键撤销=A3 回归）+ 收尾项 A（中继反向未入档）+ 收尾项 C（名字复用×crash 见证缺测试）三项裁决，关闭条件全达成（B 修复落地自证 + A 入档对账 routing.relay_asymmetry + C 作用域+测试）；残余面（谎报 peer 名、pk 锚定提案）入档 known_limitations.p15_fake_name_peer_residual 与 rulings 尾段——后续裁决已落地：关闭见 rulings.2026_10_17_p15_closure_granted，pk 锚定方向性批准立 P16"

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
  领座:      "/bring [拟名] — 引荐人的手势；带拟名则开期望门（0x11 自署名不匹配 → 响亮暂停）"
  落座:      "/sit [名字] — 候选人的手势；A1：落座署名必须本人触发；默认本人 0x05 名，可覆盖"
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
  ci: "7 job 全绿形态：test-linux / test-windows（cargo test --release --verbose --features rogue + E2EE_TEST_EVENT_BUDGET_MS=45000——双管修复裁决 rulings.2026_10_17_ci_debug_budget_mismatch；rust.yml 变更=两测试 job 的测试步骤加 --release、job 级 env 置预算，musl/android job 不动）/ musl-static-build / android×4（符号+p_align 常驻断言）。红迹起点（Actions 历史，2026-10-17 查证）：最早红 run = b535a8f（P3 slice-1/2/3 当日，2026-10-15T14:22Z，test-windows 红、test-linux 绿）；五十人/在场辩护随 093d85a（slice-5/6+P-15）入仓，此后多平台红迹持续（65 run 中 20 红）。结构性建议已裁决：owner 开 branch protection 必需检查，红 CI 阻断合流。"
  test: "cargo test --release --features rogue — slice-6 起判例注入测试需要 rogue feature（canary：缺 feature 时恰一条测试响亮失败，不静默跳过——曾因 rogue API 漏门控违约为编译失败，2026-10-17 复审连带修复）；92 tests at HEAD (70 integration + 20 unit + 1 pq + 1 dumpable[bin]), ~seconds"
  ffi_build: "cargo build --release -p e2ee-ffi — target/release/libe2ee.so; verify: nm -D target/release/libe2ee.so | grep ' T e2ee_' (expect 12)"
  ffi_contract_test: "gcc -O2 -Wall -I include ffi/ffi_test.c -o /tmp/ffi_test -L target/release -l:libe2ee.so -Wl,-rpath,$PWD/target/release && /tmp/ffi_test — expect 'FFI OK'; then python3 ffi/ctypes_test.py"
  static_build: "cargo build --release --target x86_64-unknown-linux-musl — static-pie ~1.3MB"
  android_build: "rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android；cargo install cargo-ndk（4.x）；ANDROID_NDK_HOME=<NDK r28+> cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 -P 24 -o <dir> build --release -p e2ee-ffi——四产物 <dir>/<abi>/libe2ee.so。两项直测（CI android.yml 已常驻断言，本地同款）：nm -D … | grep ' T e2ee_' 恰 12 且名单一致；llvm-readelf -lW——64 位目标（arm64-v8a/x86_64）四条 LOAD Align 全 0x4000（16KB，NDK r28 起默认）；32 位目标（armeabi-v7a/x86）ELF32 实测全 0x1000（4KB，NDK r28 对 32 位默认——无 16KB 页的 32 位设备形态，裁决 2026_10_17_p5_completion_scope 注记：如实入档，不强行加旗标）"
  wire_confidentiality_check: "tcpdump during a two-instance conversation; grep -a for names/chat words in the pcap — MUST find nothing"
  yaml_selfcheck: "python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))'"
  v3_test_matrix:                            # LANDED with slice-6（rogue 注入面覆盖 P-01/P-03/P-04/P-05/P-12；P-08 见对词 MITM 拼接测试；P-13 见单元断言）；P-15 判例 8 条断言 ↔ 矩阵条目映射已核对（owner 簿记 2026-10-17）：①零圈帧→P15_圈外自报名 ②重放不绑线→P15_spec_a 重放断言+P15_spec_b ③星型→名单圈_星型拓扑（新语义下重跑）④链式→链式拓扑_四点中继（新语义下重跑）⑤mesh 中继→署名告别_创作者离场（点名点亮 B-C）⑥伪造 genesis→矩阵条目（幻影圈断言由 P15_spec_b 的幻影桌面 partially 覆盖；/word 确定性断言属对词_格式与派生单元域——无遗漏 ⑦冒名顶替→P05_同名对峙（重写版：未绑线非直连声明）⑧冒名断线→在场辩护（重写版：未绑线不见证）——全部落地，无并入遗漏
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
    - "P-13: colluding carrier + newcomer seat → newcomer cannot decrypt pre-arrival frames despite holding full roster + archived ciphertext"
    - "voluntary-bye multi-witness: leaver with TWO direct links → both direct receivers issue RM, both applied in either order converge to same roster+key (idempotent semantics: conditional delete + unconditional row swap). P-15 注记：双『直接』线的含义收紧为双『绑定』线——普通成员的 bye 只走她唯一的绑定线（单见证），双见证=创作者离场（星形送达线 Qualified{via:她}）——见『署名告别_创作者离场』与『迟到告别_墓碑验签』；序无关收敛断言由单元交换代数 + rm_multiplicity 语义保持"
    - "voluntary-bye: forwarded-0x0C receivers NEVER issue RM (application only); direct receiver does"
    - "late-bye witness revival: RM₁ preempts leaver's direct bye at second witness → tombstone (members ∪ left_voices) verification still issues RM₂; both row rotations converge (rm_multiplicity semantics, the adjudicated flaky fix)"
    - "depart → rejoin (seat flow) → re-depart: second bye NOT swallowed (sig-keyed dedup); ADD clears the name's tombstone; witness RM fires for the second departure"
    - "P-15: out-of-circle line self-reporting a member's name receives ZERO circle frames (routing reads line bindings, never their_name)"
    - "P-15: replayed contribs/speech by a removed member bind no line (P-04 断递 intact under line bindings)"
    - "P-15: mesh direct pair (neither endpoint = creator) relays via creator pre-roll-call, direct path restored after either runs /circle (snapshot binds one side + genesis delivery binds the other)"
    - "P-15: fabricated genesis → joiner + fabricator form a two-person phantom circle whose /word group word differs deterministically from any real circle's (telephone-model boundary, no real-circle leak)"
    - "P-15 migration faces: impersonating line + fresh self-signed row → unbound line is NOT a direct claim → loud conflict, no silent replacement (P-05); impersonating line dying → no witness RM issued (no false-witness kick)"
    - "expectation-gate: 0x11 self-name mismatches /bring 拟名 → CircleConflict event + receipt discarded + pending-seat preserved; retry flow (/bring again + /sit again) completes"
    - "punch tag: omitted TAG → 128-bit auto-generated + displayed; explicit short tag → warning shown, punch proceeds"
    - "closed-table /bring full path: introducer+newcomer share new key and talk; rest stay on old key; BOTH directions mumble; roll call with introducer's line kept → re-accepts; roll call after bye-ing introducer's line → both excluded"
    - "closed-table repeated /bring DoS surface: subsequent /bring acts on introducer's OWN forked belief (N₁, N₂, …); rest of circle's conversations unaffected"
    - "presence-defense: RM against a member with verified speech within 30s wallclock (incl. evidence arriving via OTHER lines' flood) → topup withheld + loud conflict"
    - "presence-defense: RM against a member whose signed bye was seen → applied without conflict"
    - "concurrent ADDs by same introducer (two newcomers) → later-arriving one mumbles on new speech → roll call recovers (T2 intersecting change falls into T1)"
    - "solo circle forms (open stage); joiner arrives via seat flow; closed circle rejects ADD loud"
    - "50-member flood circle gathers, speaks, and roll-calls within patience budget"
    - "P-15 spec (a): chain topology — 0x0F arriving via member forward grants the line circle-line QUALIFICATION (routing eligibility), never a creator-name binding; forwarding member's line not misbound as creator; creator identity comes from the frame's signature"
    - "P-15 spec (b): ex-member replays her historical self-signed 0x12 → even if arrival binding grants line qualification, she lacks the departed-row rotation r → cannot derive K; her off-roster speech rejected loud (P-03); the every-departure-rotates-a-row invariant itself is asserted (guards against future rotation-free departure silently reopening this surface)"
    - "P-15 audit B (peer-keyed revocation, LANDED 2026-10-17): forwarded-delivery line (via=creator, peer=forwarder) extinguishes when the FORWARDER departs (RM peer-keyed) → departing forwarder receives ZERO circle frames incl. the RM frame itself (witness's fresh r never leaks; A3 seal restored); creator departure no longer over-revokes relay lines to still-seated forwarders; survivors converge on same key; roll call re-binds the half-open edge (B regains an outbound path) — P15_B_转发资格线_对端离席熄灭_除名者零圈帧 (self-certified: red pre-fix / green post-fix)"
    - "P-15 audit C (name-reuse × crash witness, LANDED 2026-10-17): v1 departs (Name binding retained per late-bye design) → v2 same-name re-enters via seat flow (apply_add clears Name AND Qualified{peer} of the reused name, before the introducer writes v2's new binding) → v1's line dying triggers NO witness RM (binding already cleared — the gate before both members-only and union variants); v2 not evicted, speaks and is heard; v2's own new binding survives the clearing — P15_C_同名重入_crash见证不误逐新任期【P16 镜子测试：P16_镜子2_同名复用_pk锚不混淆——pk 锚下 v1 的锚线随行删即熄，同名复用结构性不混淆；红绿自证已证（witness_ctx 改回按名字查 → 无据 RM 误逐 v2 → 红）】"
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
