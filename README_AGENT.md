# ═══════════════════════════════════════════════════════════════
# README_AGENT — machine-facing project brief
#
# FILE CLASS : machine-facing brief. Audience: LLM agents / coding copilots.
# HUMANS     : stop here. Use README.md / README_ZH.md instead.
# FORMAT     : yaml-dialect. indent=2; keys are stable IDs; `#` comments are
#              NORMATIVE (constraints), not decoration. File body is pure YAML.
#              STRUCTURAL INVARIANT: the whole file MUST parse as ONE valid
#              YAML document.
# SCOPE      : as-built facts describe HEAD (v1.0.1). group_v2, memory_hygiene
#              and pending_work carry OWNER-SETTLED DESIGNS not yet written —
#              every such block is status-tagged. NEVER document planned work
#              as built; NEVER re-propose anything listed in settled_decisions.
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
  maturity: early-development                 # v1.0.1 shipped; v2.0.0 design settled (see group_v2 + pending_work)
  ui_language: zh                            # user-facing strings are chinese; keep style
  comment_language: zh-with-person-metaphors # see glossary at bottom

doc_index:                                  # entry alignment across the 3 docs
  zh: README_ZH.md
  en: README.md
  agent: README_AGENT.md                     # this file
  rule: "README.md / README_ZH.md describe the SHIPPED v1 world; they must be
         rewritten together with the v2.0.0 release (see removals_2_0_0 and
         pending_work.P4). Until then they legitimately lag this file."

doc_protocol:                               # how to maintain THIS file
  update_triggers:                          # a change that does any of these MUST update this file in the same commit
    - new wire frame kind or changed frame layout
    - owner ruling in review/conversation → append to settled_decisions with date
    - a pending_work item lands → move its facts into the as-built sections, drop it from the board
    - a new falsified/rejected idea → append to settled_decisions.rejected
  yaml_selfcheck: "python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))' — MUST succeed"

# ───────────────────────────────────────────────────────────────
# ARCHITECTURE (as-built at v1.0.1)
# ───────────────────────────────────────────────────────────────

architecture:
  binary_count: 1                            # INVARIANT: deliverable is exactly one executable
  deps:
    - tokio
    - snow                                   # features: hfs + pqclean_kyber1024
    - thiserror
    - if-addrs
    - spake2                                 # PAKE (pairing secret)
    - getrandom
    - sha2                                   # group key schedule
    - hkdf                                   # group key schedule
    - chacha20poly1305                       # group AEAD
  planned_deps_v2:                           # settled, not yet in Cargo.toml
    - ed25519-dalek                          # one-time circle signing keys (group_v2.one_time_signing)
    - zeroize                                # memory hygiene (memory_hygiene)
  lifecycle: single-startup-person           # INVARIANT: no role subcommands; one process is born with both ears and mouth
  birth_modes: "born (v1: visible, answers /shout — REMOVED in 2.0.0) | --no-chain (set_chaining(false): never offers a next secret, never accepts one — the open-desk shape) | --guard <machine-secret> (expect_secret_stable: EVERY arrival must pass PAKE; arming NEVER consumed; the armed open-desk shape is `--guard KEY --no-chain`) | --hidden (born_hidden — DEPRECATED alias in 2.0.0, see removals_2_0_0)"
  other_job: "--courier — the same binary as a teahouse: room numbers are GUEST-CHOSEN (agreed out-of-band, same channel as secrets); it wires same-room arrivals, relays opaque bytes; no identity, no storage, no plaintext"
  commands:                                  # in-session, parsed by CLI handle_command (src/main.rs)
    dial:  { syntax: "/dial ADDR:PORT [secret]", behavior: "outbound connect + hybrid NN handshake as initiator; optional secret → PAKE first; auto-uses chained secret for that addr (burned only when actually verified); 5s connect timeout" }
    meet:  { syntax: "/meet COURIER ROOM [secret]", behavior: "enter a teahouse room; roles assigned by courier (later arrival initiates); optional secret → PAKE inside the room; teahouse lines never chain" }
    punch: { syntax: "/punch INTRODUCER TAG [secret]", behavior: "register the same tag with the introducer; he exchanges public endpoints; both sides hole-punch; noise handshake then runs over the punched udp line; best-effort — on failure the user re-routes via /meet MANUALLY" }
    await:  { syntax: "/await SECRET", behavior: "arm the password challenge for the NEXT inbound arrival (one-shot; spent only when PAKE masks are actually exchanged)" }
    talk:  { syntax: "/talk id|name-prefix", behavior: "switch input focus; ambiguous prefix lists candidates" }
    list:  { syntax: "/list", behavior: "enumerate live conversations, mark focused" }
    card:  { syntax: "/card", behavior: "print one 'name  ip:port' line per interface — SURVIVES the 2.0.0 discovery removal (card only lists own addresses, touches no network)" }
    shout: { syntax: "/shout", behavior: "v1: udp-broadcast who-is-there on the lan — REMOVED in 2.0.0 (removals_2_0_0)" }
    circle: { syntax: "/circle", behavior: "v1: form a gathering over ALL current conversations (requires full mesh); v2: see group_v2 (flood topology, solo allowed)" }
    gsay:  { syntax: "/gsay TEXT", behavior: "speak in the circle; v2 gains a circle-target argument (group_v2.nested_circles)" }
    file:  { syntax: "/file PATH", behavior: "hand a file to the focused conversation: CLI reads the path, sends as 0x06 head + 0x07 chunks; lossless; 32MB cap both ways; received files auto-save to ./received/" }
    bye:   { syntax: "/bye", behavior: "offer next secret (chainable lines only), then empty-plaintext goodbye to focused conversation" }
    quit:  { syntax: "/quit | Ctrl-D", behavior: "goodbye to all conversations (chainable lines offer secrets), then exit 0" }
  sessions: multiple-concurrent-1to1
  focus_model: "typed lines go to the focused conversation; focus set by /dial or /talk; incoming conversations never steal focus"
  symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node — extends to circles (ownerless, group_v2.ownerless)
  packaging: workspace-three-gates            # root pkg = rust lib `e2ee` + bin; ffi/ member = cdylib gate — see ffi block
  api_style: "library NEVER prints nor reads stdin nor touches the fs; actions in = metaphor-verb methods; experiences out = Event stream (tokio mpsc); Event/LeaveReason/PersonError are non_exhaustive; errors carry semantics, CLI renders prose"
  modules:
    src/lib.rs:     "crate root: pub mod circle + courier + discover + person + punch + wire; re-exports Person/Events/Event/LeaveReason/PersonError/RosterEntry/Discovered"
    src/person.rs:  "core person model; handshake<S> GENERIC over AsyncRead+AsyncWrite (tcp and UdpPunch alike); ear() dispatches on frame_kind; ZERO printing, ZERO stdin, ZERO fs. Security-critical: burned flag semantics (secret consumed iff SPAKE2 masks exchanged); noise_stage pads handshake messages 0..=255B; send_chat emits padded 0x08; send_file randomizes chunk sizes 8–32KB; register_convo encrypts the NAME frame; Convo.chainable gates secret-chain offers; Inner.chaining is the master switch"
    src/punch.rs:   "introducer client + punch transport: UdpPunch (frame-per-datagram, echo swallowing); INTRO2 source-checked"
    src/circle.rs:  "group key schedule: derive_group_key (HKDF over byte-sorted contributions), derive_sender_subkey, seal/open (ChaCha20Poly1305, nonce = counter ‖ sender-tag); unit tests for order-independence and subkey isolation"
    src/courier.rs: "teahouse: serve() wires same-room arrivals, splices raw bytes; serve_introducer on the same udp port; JOIN frame bounded 10s"
    src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; clean EOF vs mid-frame break"
    src/discover.rs: "v1: lan shout + card. 2.0.0: shout listener + shout() REMOVED; card() (own-interface listing) survives"
    src/main.rs:    "thin CLI renderer; safe() sanitizes ALL remote-controlled strings"
    ffi/src/lib.rs: "cdylib gate: 12 exported e2ee_* symbols; per-handle tokio runtime"
    include/e2ee.h: "C header — MUST change together with ffi/src/lib.rs (frozen ABI)"
    tests/conversation.rs: "21 integration tests, real persons on ephemeral ports"
    tests/pq.rs:       "hybrid suite round-trip"

# ───────────────────────────────────────────────────────────────
# CRYPTO (as-built)
# ───────────────────────────────────────────────────────────────

crypto:
  suite_no_secret: "Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s"       # hybrid X25519+Kyber1024
  suite_with_secret: "Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s" # PAKE output as PSK
  pake: "SPAKE2 (spake2 crate, Ed25519Group) — masks exchanged, secret never crosses the wire; wrong secret → loud failure; burn semantics: secret spent iff masks were exchanged; stable guard never consumed"
  static_keys: none                          # INVARIANT: ephemeral per-connection only
  identity_layer: none                       # INVARIANT: no auth / TOFU / trust store / accounts — for how v2 circle signing relates to this, see invariants.no-identity-layer
  auth_model: telephone                      # secrecy by default; PAKE = optional per-meeting human-carried secret
  pfs: implicit                              # fresh keys per connection ⇒ forward secrecy for free
  group_layer: v1-full-mesh                  # see group_chat; v2 design in group_v2
  pq_hybrid: implemented                     # X25519 AND Kyber1024, both must break; NEVER pure-PQ
  mid_session_swap: impossible-by-aead

memory_hygiene:                              # STATUS: settled design, NOT yet implemented (pending_work.P1)
  status: settled-design-2026-10-13
  threat_tiers:                              # what we defend against, honestly layered
    T0_same_machine_software: "root / kernel / ptrace / /proc/pid/mem — OUT OF SCOPE (telephone-model boundary, documented)"
    T1_secondary_persistence: "swap-to-disk, zram writeback, core dumps, hibernate images — OUR battlefield; fully closeable in software"
    T2_opportunistic_physical: "live DMA (thunderbolt/usb4), debug ports — OS/IOMMU domain; app can only shrink the exposure window"
    T3_physical_extraction: "cold boot (dram remanence), chip-off/JTAG reconfig — hardware domain; app-level unsolvable; shrink the window only"
  in_memory_encryption: "REJECTED — NEVER re-propose (settled_decisions.rejected.in_memory_encryption). Circular argument: the key that decrypts must itself live in memory; CPU must see plaintext to compute. Without a hardware anchor this is security theater (a costume, not a lock). TRESOR-style register-resident keys don't fit Noise session state; TEE/TrustZone migration breaks single-binary symmetry and plaintext crosses the normal world anyway."
  goal: exposure-minimization                 # minimize TIME WINDOW and PHYSICAL EXTENT of secrets in RAM, weld T1 shut
  zeroize_scope:                             # everything on this list gets zeroize-on-drop / zeroizing wrappers
    - spake2 ignition key [u8;32]
    - group key + sender subkeys + contributions r_i
    - chained/expecting/stable secrets
    - file-transfer buffers (32MB plaintext currently resident!) — InboundFile.buf, FileArrived.data
    - chat plaintext frame buffers
  snow_gap: "snow 0.10.0 has NO zeroize feature — TransportState session keys stay on the heap after drop. DECISION PENDING (pending_work.P1): minimal vendored fork adding ZeroizeOnDrop to CipherState (preferred, tiny diff, behavior-identical) vs accept-and-document the residue"
  mlock: "lock KEY pages only (KB-scale, within RLIMIT_MEMLOCK); 32MB file buffers NOT locked (accepted, documented) — optional future: segmented zeroizing pipeline for file transfer"
  dump_refusal: "RLIMIT_CORE=0 + MADV_DONTDUMP on key pages; evaluate memfd_secret on linux"
  android_note: "android zram is IN-RAM compressed swap, but Android 17 mmd adds zram WRITEBACK to /data loopback — the 'android has no swap-to-disk' intuition is FALSE on new versions; mlock on key pages closes it"
  swap_check: "desktop CLI startup reads /proc/swaps; unencrypted swap partition → one stderr warning line (zero cost, honest engineering)"
  honest_boundary: "T2/T3 remain out of reach; the promise upgrades from 'zero persistence' to 'zero persistence + active memory hygiene' — threat model docs gain a tier table"

# ───────────────────────────────────────────────────────────────
# WIRE PROTOCOL (as-built + settled v2 additions)
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
    contrib: "0x03 + 32B contribution — v1 circles; SUPERSEDED by 0x09 in v2 (parser may retire at 2.x end)"
    circle_chat: "0x04 + nonce ‖ group-AEAD — v1 circles; SUPERSEDED by 0x0A in v2"
    name: "0x05 + utf8 name"
    file_head: "0x06 + name_len ‖ name ‖ total_bytes(u64) — >32MB declined loudly"
    file_chunk: "0x07 + seq(u32) ‖ data — 8–32KB randomized chunks"
    padded_chat: "0x08 + text_len ‖ text ‖ random pad — what send_chat emits"
    # ── v2 frame kinds (planned; see group_v2) ──
    circle_contrib_v2: "0x09 + circle_id(8B) ‖ name_len(u8) ‖ name ‖ r_i(32B) ‖ sig_pk(32B) — DECLARED contribution (group_v2.contribution_format)"
    circle_signed: "0x0A + circle_id(8B) ‖ sender_tag(4B) ‖ counter(8B) ‖ group-AEAD ‖ sig(64B) — signed circle speech (group_v2.one_time_signing)"
    circle_topup: "0x0B + circle_id(8B) ‖ epoch(u32) ‖ change-spec ‖ fresh_r(32B) ‖ initiator_sig(64B) — membership change / rekey in ONE frame (group_v2.topup)"
    circle_bye: "0x0C + circle_id(8B) — voluntary circle departure announcement (group_v2.voluntary_leave)"
  unknown_kinds: "silently skipped — forward compatibility (this is what makes the v2 additions rollout-safe for old peers)"
  hangup: "empty plaintext frame | tcp close"
  teahouse_control:                          # plaintext frames on the courier line, before the splice
    join: "'JOIN <room>' — first frame from each arrival; rooms guest-chosen; same-number collisions = cross-wired pairs"
    paired: "'PAIRED R' to the first arrival, 'PAIRED I' to the second — roles for the noise handshake"
  introducer_and_punch:
    introducer: "courier::serve_introducer — udp side of the teahouse port; INTRO1 tag / INTRO2 R|I peer; one-shot tags, ttl 90s, MAX_WAITING_TAGS 1024"
    punch: "register w/ retry (3s), connect to peer's public endpoint, exchange PUNCH frames (~150ms apart, 8s window); INTRO2 accepted ONLY from the introducer's own address"
    transport: "UdpPunch: one frame per datagram, punch-echo discarded, max frame 65507B; no liveness detection — a vanished peer hangs the line until /quit"
    fallback: "on punch failure the USER re-routes via /meet manually — no automatic fallback"
  lan_discovery:                             # v1 as-built — ENTIRE SUBTREE REMOVED in 2.0.0 (removals_2_0_0)
    status: removed-in-2.0.0
    v1_behavior: "udp 37777; E2EEPROBE1 broadcast probe, E2EEREPLY1 name+port reply; hidden mode = listener never spawned"

# ───────────────────────────────────────────────────────────────
# FILE TRANSFER (as-built)
# ───────────────────────────────────────────────────────────────

file_transfer:
  status: implemented-v1
  capability: "lossless binary handover on ANY conversation path (direct / teahouse / punched line)"
  lossless: "from_utf8_lossy NEVER applies to file payloads; receiver asserts byte-equality in tests incl. invalid-UTF-8"
  chunking: "randomized 8–32KB; receiver size-agnostic; no negotiation"
  cap: "MAX_FILE_BYTES = 32MB, enforced symmetrically"
  in_flight: "ONE transfer per conversation per direction; stalled half-file never blocks the next"
  robustness: "seq must match next_seq exactly; mismatch voids the transfer silently"
  zero_persistence: "reassembly in RAM only; the LIBRARY never touches the fs; bytes surface as Event::FileArrived"
  memory_note: "the 32MB RAM residency is a T1 exposure — see memory_hygiene.zeroize_scope"
  ffi_status: "FileArrived/FileDeclined map to tag 0 in e2ee_poll; an e2ee file API is the natural add-only extension when a C consumer needs it"

# ───────────────────────────────────────────────────────────────
# GROUP CHAT v1 (as-built) — superseded by group_v2 design
# ───────────────────────────────────────────────────────────────

group_chat:
  status: v1-as-built-superseded             # the v2 design below REPLACES topology, attribution and rekey; the KEY SCHEDULE itself is kept unchanged
  v1_boundaries: "full mesh manual; rekey = re-run /circle after new joins"
  scheme: group-key                          # one negotiated group key + per-sender subkeys — KEPT in v2
  rejected_alternative: >-                   # do NOT reintroduce
    pairwise re-encryption per link — considered and rejected; sessions are TRANSPORT ONLY, not group content crypto.
  topology: full-mesh                         # v1: every member dials every other; N² links; practical cap ~8 — THE constraint v2 removes
  group_key_schedule:                        # UNCHANGED in v2 — this is the load-bearing stability
    contribute: "each member broadcasts a fresh 32B random r_i (v1: 0x03 frames)"
    derive: "K = HKDF(byte-sorted concatenation of all r_i) — order-independent; no member list needed"
    per_sender_subkey: "s_i = HKDF(K, r_i) — r_i doubles as sender label"
    send: "encrypt ONCE with s_i (nonce = counter ‖ sender_tag); identical blob fanned out on every link"
    recv: "decrypt with s_j derived from the RECEIVING link's contribution r_j"
  rekey_policy_v1: "membership-join-only, never on leave — SUPERSEDED in v2 (topup)"
  race_handling: "simultaneous /circle runs → later form_circle replaces the gathering wholesale; AEAD failure ⇒ CircleMumble (a notice, never silence); re-run to converge — the v2 final safety net KEEPS this"
  honest_limitation_v1: "any member can derive others' subkeys (r_i is broadcast) but cannot impersonate — v1 injection prevention is TOPOLOGICAL (a link only its owner has); v2 replaces the topology with signatures (see group_v2)"

# ───────────────────────────────────────────────────────────────
# GROUP v2 — flood circles (settled design 2026-10-13/14, not yet implemented)
# ───────────────────────────────────────────────────────────────

group_v2:
  status: settled-design-not-implemented     # implement per pending_work.P2 (core) + P3 (nesting)
  one_line: "circles drop the full-mesh requirement: connected-is-enough, contributions flood, speech is signed by one-time keys, membership changes rekey in ONE top-up frame, and a circle of one is a channel."

  flood_distribution:
    insight: "the v1 'contribution self-contagion' IS epidemic broadcast — the distribution layer barely changes"
    topology: "connected graph, ≥N-1 edges; direct/teahouse/punched lines all count as edges; outer envelopes stay per-line Noise (e2ee-only-transit intact)"
    forwarding: "on receiving a circle frame, relay to all convos EXCEPT the line it arrived from"
    dedup_keys: "contributions dedup by r_i itself (32B random); speech dedups by (sender_tag, counter) — counter doubles as replay protection"
    practical_cap: "~50 members (CPU + convergence patience), up from ~8 (link-squared wall removed); scaling table: links N(N-1)/2 → ≥N-1; per-member outbound per speech = degree"

  contribution_format:                       # frame 0x09
    payload: "circle_id(8B) ‖ name_len(u8) ‖ name(utf8) ‖ r_i(32B) ‖ sig_pk(32B)"
    why_declared: "under flooding a contribution may arrive via a neighbor's line — the v1 keying by conversation-id would attribute the wrong subkey; contributions must SELF-DECLARE who they belong to"
    provenance_rules:                        # how a declared name gets bound to a person
      direct_lock: "if the arrival line's peer name == declared name → trust (Noise line is authentic — strength EQUALS v1)"
      relayed: "else accept but record provenance (via whom it was relayed); small circles are always direct, so the marker never appears there"
      first_seen_wins: "a second contribution under an already-present name is rejected"
    binding_honesty: "SECRECY is mathematical; IDENTITY CERTAINTY is bounded by who you are directly connected to — circles wanting strong member identity should keep direct dials among members (the protocol no longer forces it, nor confiscates it)"

  one_time_signing:                          # frame 0x0A — the replacement for 'a link only its owner has'
    problem: "under flooding a CIRCLE frame can arrive over any line — topological attribution is structurally gone; NOTE: subkeys alone CANNOT replace it (every member can derive every subkey — r_i values are broadcast)"
    mechanism: "at gathering time each member generates a THROWAWAY ed25519 keypair; the PUBLIC key rides the contribution (0x09); the private key lives in RAM only, scoped to that circle, dies with the process"
    signing: "speech frame carries sig = Ed25519(sk, sender_tag ‖ counter ‖ ct); receivers verify with the pk from the gathering"
    strength: "NOT weaker than v1: v1 proved 'the owner of this line' to direct peers; v2 proves 'whoever published this pk at gathering time' to the WHOLE circle, path-independently"
    not_identity: "session-scoped signing capability ≠ identity layer — no cross-circle continuity, no TOFU, no accounts; see invariants.no-identity-layer"
    new_dep: ed25519-dalek                    # pure rust, small; the ONLY new crypto dep

  topup:                                     # frame 0x0B — membership change / rekey in ONE frame
    replaces: "v1 rekey_policy (join-only, full re-gathering) AND the rejected random-backoff auto-re-circling"
    insight: "a rekey does not need fresh entropy from everyone — ONE fresh value the leaver never saw suffices (the leaver knew K_old and ALL old r_i)"
    change_spec: "RM <name>  (member leaves) | ADD <name> <r(32B)> <pk(32B)>  (member joins; r/pk carried by the introducer-topup)"
    frame: "circle_id ‖ epoch(u32) ‖ change-spec ‖ fresh_r(32B) ‖ initiator_sig(64B)"
    initiator: "the remaining member with byte-smallest r_i — deterministic, zero coordination; cascade: if no topup seen within 3s, next-smallest takes over"
    application: "apply the change to the member set; the initiator REPLACES her own r_i with fresh_r; K_new = derive_group_key(sorted(remaining r)) — the EXISTING function, zero crypto change; subkeys re-derived locally by everyone"
    validity: "a topup is valid iff the named leaver is still in MY current member set — first-arriver wins the epoch; same-leaver latecomers are no-ops; different leavers serialize by arrival"
    cost: "O(edges) flooded frames per membership change — from O(N) full re-gatherings to ONE frame"
    covers: "leave (crash or voluntary), join, and plain key-freshening — all the same primitive"

  voluntary_leave:                           # frame 0x0C
    mechanism: "on /bye or /quit, before closing lines, a member broadcasts circle_bye on every circle she sits in → single clean trigger, everyone rekeys via topup once"
    crash_path: "an observer only knows HER line to the leaver died; conservative rule: any dead line to a member ⇒ treat as leave ⇒ topup; ambiguity resolves via the validity rule above; mumble remains the final safety net"

  solo_circle:                               # the 'broadcast' concept, absorbed
    change: "drop the v1 requirement that convos be non-empty to form a circle"
    semantics: "a circle of one is a CHANNEL: open it and wait; joiners arrive via direct dial + ADD topup; 'want an audience' = open a circle; 'want to listen' = join one; no forced-common-channel exists anywhere in the system"

  nested_circles:                            # 大圈套小圈 — halls and tables
    motivation: "a hall everyone shares (the big circle) + small tables talking privately beside it; table content never reaches the hall or other tables"
    slots: "Inner grows TWO named gathering slots (hall + table) instead of one Option<Gathering>; each gathering has an 8B random circle_id; all v2 circle frames carry it; dedup keys are (circle_id, …)"
    line_reuse: "the SAME noise line between two people can carry frames of both circles (outer envelopes are per-line; kinds disambiguate) — zero extra link cost; table edges mostly OVERLAP hall edges"
    compartmentalization: "one table compromised ⇒ hall key leaks with it (member of both), other tables unaffected"
    both_circles_use: "the SAME v2 mechanisms (flood/signing/topup) — nesting is bookkeeping, not new crypto"
    cli: "/gsay gains a circle-target argument; /circle gains slot selection"
    honesty: "serverless nested circles have NO known prior art (closest: draft-ietf-mls-partial for large-broadcast MLS) — experimental label is EARNED, keep it"

  ownerless:                                 # settled social contract
    rule: "NO circle owner, NO admin role, NO kick command — symmetry forbids it"
    why: "real speech is peer-to-peer too; a one-voice hall is what mainstream social platforms sell (they own the room); here NOBODY owns any room — a one-voice hall is not forbidden, it is UNCONSTRUCTIBLE"
    ejecting_a_jerk: "everyone drops their lines to him + one topup — collective action, no privilege"
    starting_over: "open a new circle, tell wanted members over 1:1 — the address book lives outside the system, as always"

# ───────────────────────────────────────────────────────────────
# DISCOVERY & PAIRING (post-2.0.0 shape)
# ───────────────────────────────────────────────────────────────

discovery_and_pairing:
  status: "teahouse + punch + card survive; shout/hidden/lan-broadcast REMOVED in 2.0.0"
  principle: "the system forgets, humans remember — the address book lives OUTSIDE the system (paper, memory)"
  rungs_cheapest_first:
    - { name: card,     status: kept,        metaphor: business-card, mechanism: "print name + ip:port per interface; hand it over out-of-band (no network activity)" }
    - { name: teahouse, status: kept,        metaphor: courier-who-introduces, mechanism: "--courier on the same binary; JOIN room / PAIRED R-I / raw splice" }
    - { name: punch,    status: kept,        metaphor: introducer, mechanism: "udp hole-punch rendezvous by tag; best-effort" }
    - { name: shout,    status: removed-in-2.0.0, metaphor: same-room-call, note: "was the ONLY unsolicited exposure source; hidden mode existed only to dodge it — both die together (removals_2_0_0)" }
  pairing_secret:                            # unchanged
    pake: "SPAKE2 — secret never crosses the wire in derivable form; one-shot per arming; scanner-proof burn semantics"
    key_schedule: "PAKE output only ignites the noise handshake; all conversation keys are fresh ephemerals"
  secret_chain:                              # unchanged
    mechanism: "at farewell, reserve the NEXT secret inside the authenticated channel — offer + ack"
    no_chain_mode: "set_chaining(false) / --no-chain — open-desk shape; armed variant: --guard KEY --no-chain"
    red_line: "NEVER persist a rolling secret to disk"

removals_2_0_0:                              # the deletion face — settled 2026-10-14, ships in v2.0.0
  version_window: "v1.0.1 hotfix (already staged) contains NONE of this; the removal face is a MAJOR version per semver"
  items:
    shout: "udp lan discovery (probe/reply + listener + /shout command) — functionally redundant once circles absorb audience-finding; was the only unsolicited exposure source"
    hidden_mode: "born_hidden / --hidden / e2ee_person_create_hidden — existed only to dodge shout probes; meaningless without it"
    lan_broadcast: "the loudspeaker /say design discussed 2026-10-14 — CANCELLED BEFORE BIRTH (see settled_decisions): unmanageable multi-broadcaster flooding, and 'one voice only' is unenforceable on udp; solo circles absorb the entire use case"
    listen_toggle: "the deaf/undeaf switch — evaporates with the loudspeaker it was meant to gate"
  ffi_compatibility: "e2ee_person_create_hidden STAYS as a deprecated alias of e2ee_person_create (ABI is frozen add-only; removing an exported symbol is a hard break) — docs mark it deprecated"
  consequence: "two peers on a lan with no prior contact can no longer find each other — exchange addresses out-of-band or meet at an agreed teahouse; this ALIGNS with the existing 'address book lives outside' philosophy and removes an automated name-leak channel"

# ───────────────────────────────────────────────────────────────
# PQ HYBRID (as-built, unchanged)
# ───────────────────────────────────────────────────────────────

pq_hybrid:
  status: implemented
  threat: "HNDL — harvest now, decrypt later"
  scheme: hybrid                             # X25519 AND Kyber1024 combined (SETTLED)
  red_line: "NEVER pure-PQ replacement — the hybrid keeps the 40-year classical lock as a floor"
  costs: "≈ +3.1KB per handshake; ≈ +200-300KB binary; tens of µs per handshake"

# ───────────────────────────────────────────────────────────────
# FFI (as-built)
# ───────────────────────────────────────────────────────────────

ffi:
  status: implemented-v1
  layout: "workspace member ffi/ (package e2ee-ffi, cdylib crate name e2ee) over the root rlib; header include/e2ee.h; contract test ffi/ffi_test.c; python proof ffi/ctypes_test.py"
  surface: "12 exported symbols: e2ee_version / e2ee_person_create / e2ee_person_create_hidden / e2ee_person_destroy / e2ee_local_addr / e2ee_last_error / e2ee_await_secret / e2ee_dial / e2ee_speak / e2ee_bye / e2ee_leave / e2ee_poll"
  freeze_rules:                             # HARD — ABI is a published contract
    - "include/e2ee.h and ffi/src/lib.rs MUST change together (same commit)"
    - "add-only: new functions allowed; changing an existing signature or struct layout NEVER"
    - "e2ee_event layout frozen: fixed 64B name / 512B text buffers, truncating copy, always NUL-terminated"
    - "unknown Event variants map to tag 0 — old consumers survive new events"
  v2_notes: "e2ee_person_create_hidden becomes a deprecated alias (removals_2_0_0); circle v2 events (topup/rekey) extend the enum add-only"
  not_yet_exposed: "teahouse / circle / card / roster / file transfer / stable guard — add on demand under the same freeze rules"
  model: "opaque handle; per-handle private tokio runtime; destroy = farewell + runtime drop"
  threading: "handle lifetime managed serially on one thread; never destroy while another thread is still polling"

# ───────────────────────────────────────────────────────────────
# INVARIANTS — any change violating these MUST be rejected
# ───────────────────────────────────────────────────────────────

invariants:
  - "zero-persistence: no fs writes ever — no logs, config, cache, keys, history, state files"
  - "plaintext-containment: plaintext allowed only in stdin-buffer, stdout-write, RAM; never in errors/args/env; on the wire, names ride inside noise frames"
  - "e2ee-only-transit: TALK crosses the wire exclusively inside noise transport frames (control-plane frames — teahouse JOIN/PAIRED — are plaintext by design and always were)"
  - "no-identity-layer: never add authentication, TOFU, key continuity, accounts, fingerprint checking. PRECISE MEANING UNDER v2 — one-time circle signing keys are session-scoped attribution capability, NOT identity; they never persist, never chain across circles, never accumulate trust"
  - "symmetry: never introduce server/client asymmetry or a node with more power than a peer. COROLLARY (v2) — circles are OWNERLESS; no admin role, no kick privilege, no one-voice enforcement"
  - "ephemeral-keys: every connection = fresh keypair; zero key reuse across anything"
  - "attribution-v2: v1 'attribution-by-topology' (a link only its owner has) is SUPERSEDED under flooding by 'attribution-by-one-time-signature' (speech verified against the gathering-published pk; direct-arrival contributions get the stronger direct lock)"
  - "rekey-v2: v1 'membership-join-only' is SUPERSEDED by 'membership-change' — join, leave AND plain freshening all rekey via the single topup frame; a departed member must not read NEW speech (old plaintext he already saw is history, not a leak)"
  - "secret-chain-red-line: NEVER persist a rolling secret to disk"

# ───────────────────────────────────────────────────────────────
# SETTLED DECISIONS — owner rulings 2026-10-13/14. HIGHEST authority
# beside invariants; re-proposing a rejected item = reject
# ───────────────────────────────────────────────────────────────

settled_decisions:
  kept:
    relay_circle: "flood circles are THE answer to >8 groups — connected-is-enough; the v1 key schedule is kept unchanged"
    compromised_device: "a seized device that stays in the circle keeps reading — this is INHERENT to group keys, NOT introduced by relay circles; physical-coercion defenses are a LATER, separate work item"
    leave_rekeys: "departures MUST rekey — 'that is how it works in real life too' — implemented as topup, not full re-gathering"
    one_time_signing: "ed25519 one-time circle signing keys (option B) — approved 2026-10-13; the only acceptable replacement for topological attribution"
    small_circle_markers: "relayed-contribution markers are unnecessary in small circles — 'people naturally know' (small circles are all-direct anyway; markers only ever render on relayed arrivals)"
    nested_circles: "halls-and-tables (大圈套小圈) — owner's own idea; no 'speaker' variant, pure nesting; tables never leak to hall or other tables"
    solo_circle: "a circle of one is allowed — circles absorb the entire 'broadcast/audience' concept"
    ownerless: "'there was never an owner — speech among equals is how reality works; those wanting a one-voice platform should use mainstream social media'"
    android_abi: "prebuilt libraries for all android ABIs (pending_work.P5)"
  rejected:                                  # do NOT re-propose
    in_memory_encryption: "rejected — encrypting memory with a key that must itself live in memory is circular; a costume, not a lock; exposure-minimization instead (memory_hygiene)"
    mls_adoption: "MLS/TreeKEM rejected for v2 — credential framework collides with no-identity-layer; dependency weight breaks single-binary; documented future direction only"
    loudspeaker_broadcast: "the /say loudspeaker design — cancelled before birth: who broadcasts? multiple broadcasters flooding? unmanageable, and 'only one voice' is unenforceable; solo circles absorb the whole use case"
    lan_discovery_family: "shout + hidden + lan broadcast removed — redundant; also removes the only unsolicited exposure source"
    random_backoff_recircle: "random-delay full re-gathering on leave — replaced by deterministic topup (smallest-r initiator)"
    listen_switch: "the single 'hear broadcasts?' toggle — evaporated with the loudspeaker it was designed to gate"
    per-listener_choice_in_rooms: "'paired rooms hearing the hall' sub-options — mooted by loudspeaker cancellation"

# ───────────────────────────────────────────────────────────────
# KNOWN LIMITATIONS — honest edges; do NOT paper over in docs
# ───────────────────────────────────────────────────────────────

known_limitations:
  no_liveness_probes: "established conversations have no keepalive — a vanished peer hangs the line until /quit; under v2 circles this upgrades from personal nuisance to circle impact (a zombie member's contribution stays in the key until topup)"
  seized_device_reads_on: "a seized in-circle device reads until its lines die + topup lands; old plaintext remains readable to old key holders (history, not a leak); physical-coercion defense is deferred future work"
  identity_bounded_by_direct_links: "contribution name-binding under flooding is first-seen + provenance — secrecy is mathematical, member-identity certainty is bounded by direct connections (group_v2.binding_honesty)"
  flood_frame_loss: "lost contributions ⇒ key mismatch ⇒ CircleMumble; recovery = re-gather (convergence patience is the real v2 scaling limit, ~50)"
  circle_race: "simultaneous gathering formation can leave members with mismatched keys — CircleMumble surfaces it, re-circling converges"
  zram_writeback_residue: "android mmd zram writeback lands idle pages on /data — key-page mlock (memory_hygiene) closes the key exposure; the 32MB file buffer stays an accepted, documented residue"
  snow_zeroize_gap: "snow 0.10 TransportState keys are not zeroized on drop — pending fork-vs-document decision (pending_work.P1)"
  cross_version_chat_one_way: "peers predating 0x08 silently drop padded lines; v2 frame kinds 0x09-0x0C inherit the same one-way property"
  nested_no_prior_art: "serverless nested circles have no known prior art — experimental label stays until hardened"

# ───────────────────────────────────────────────────────────────
# PENDING WORK — settled board, ordered; P5 parallelizable anytime
# ───────────────────────────────────────────────────────────────

pending_work:
  P1_memory_hygiene:
    status: not-started
    scope: "implement memory_hygiene: zeroize crate across the listed scope; mlock key pages; RLIMIT_CORE=0 + MADV_DONTDUMP; /proc/swaps startup warning; snow fork decision (vendored minimal ZeroizeOnDrop fork PREFERRED vs document residue) — owner to confirm fork"
    slices: "zeroize wrappers → key-page mlock → dump refusal → swap check → docs (threat tier table)"
  P2_flood_circle_core:
    status: not-started
    depends: []
    slices:
      - "0x09 declared contributions + flood forwarding + dedup + provenance (direct lock / first-seen) — distribution only, zero crypto change; existing tests re-parametrized to non-full-mesh topologies (star, chain)"
      - "one-time signing keys: 0x0A speech frame + gathering pk registry + ed25519-dalek dep; seal()/key schedule untouched"
      - "topup 0x0B: RM/ADD change-spec, smallest-r initiator, 3s cascade, validity rule, epoch serialization; voluntary leave 0x0C wired into /bye //quit paths"
      - "solo circle: drop the non-empty-convos guard"
      - "test matrix: star/chain topology circles, relayed-contribution forgery attack, leave-then-mumble, topup convergence, 50-member scale, solo-circle-then-join"
  P3_nested_circles:
    status: not-started
    depends: [P2_flood_circle_core]
    scope: "hall + table slots in Inner; circle_id on all v2 frames + (circle_id,…) dedup; line reuse; /gsay target argument + /circle slot selection; event surface extension (add-only FFI tags); compartmentalization tests"
  P4_removal_face_v2:
    status: not-started
    scope: "delete shout/hidden/lan listener + /shout + --hidden (discover.rs keeps card()); ffi create_hidden → deprecated alias; README.md/README_ZH.md full rewrite to v2 world; version 2.0.0; MUST NOT ship in the staged 1.0.1 hotfix"
  P5_android_abi:
    status: not-started
    scope: "cargo-ndk + NDK r28+ (16KB page alignment — verify LOAD p_align via llvm-readelf); targets aarch64-linux-android / armv7-linux-androideabi / x86_64-linux-android / i686-linux-android; api 24; verify 12 e2ee_* symbols per .so; GitHub Actions job + release artifacts; pure-rust deps only (no C cross-compile hazards)"
    parallelizable: true                     # no dependency on P1-P4

# ───────────────────────────────────────────────────────────────
# GLOSSARY — zh metaphor → mechanism
# ───────────────────────────────────────────────────────────────

glossary:
  耳朵:      inbound tcp accept loop (person::born acceptor; one ear task per conversation)
  出生:      Person::born — bind + spawn acceptor, returns (Person, Events)
  嘴:        outbound tcp connect (dial)
  见面:      noise nn handshake
  密谈:      noise transport phase
  挂断:      session teardown
  道别:      empty-plaintext goodbye frame
  转身即忘:  zero persistence
  自报家门:  unsigned self-reported name frame (noise-encrypted, kind 0x05)
  无名氏:    default name when -n omitted
  找人:      /dial outbound connect
  注意力:    focus — which conversation receives typed lines
  收摊:      conversation teardown by the ear task
  介绍人:    teahouse keeper's side job — udp address exchange, same port as tcp
  递拳头:    punch — repeated PUNCH frames to the peer's public endpoint until one comes back
  打洞:      udp hole punching; UdpPunch (one frame per datagram, echoes swallowed)
  名片:      /card — name + per-interface dial addresses (survives 2.0.0)
  茶馆:      "--courier two-person-room relay; keeper doubles as introducer on udp"
  暗号:      user-chosen pairing secret, SPAKE2-verified, chains at farewell
  凑话:      "contribution — each member's fresh 32B random; v2 adds declared name + sig-pk (0x09)"
  圈语:      CircleMumble — undecipherable group frame surfaced as a notice (never silence)
  补话:      "topup (0x0B) — ONE-frame membership change — someone leaves the table, the smallest-contribution member supplies a fresh sentence, the table re-keys"
  署名钥:    one-time ed25519 circle signing key — public half rides the contribution, private half dies with the process
  泛洪圈:    flood circle — connected-is-enough distribution; contributions and speech relayed minus the arrival line
  堂:        hall — the big shared circle slot
  桌:        table — the small private circle slot (content never reaches hall or other tables)
  一人成圈:  solo circle — a channel of one, joiners arrive via ADD topup; absorbs the entire 'broadcast' concept
  话筒:      the cancelled loudspeaker — kept here ONLY so future agents recognize it as rejected (settled_decisions.rejected)

# ───────────────────────────────────────────────────────────────
# VERIFY — how to check your work
# ───────────────────────────────────────────────────────────────

verify:
  build: "cargo build --release"
  test: "cargo test --release — 27 tests at v1.0.1 (5 unit + 21 integration + 1 pq suite), ~4.5s"
  ffi_build: "cargo build --release -p e2ee-ffi — target/release/libe2ee.so; verify: nm -D target/release/libe2ee.so | grep ' T e2ee_' (expect 12)"
  ffi_contract_test: "gcc -O2 -Wall -I include ffi/ffi_test.c -o /tmp/ffi_test -L target/release -l:libe2ee.so -Wl,-rpath,$PWD/target/release && /tmp/ffi_test — expect 'FFI OK'; then python3 ffi/ctypes_test.py"
  static_build: "cargo build --release --target x86_64-unknown-linux-musl — static-pie ~1.3MB"
  wire_confidentiality_check: "tcpdump during a two-instance conversation; grep -a for names/chat words in the pcap — MUST find nothing"
  yaml_selfcheck: "python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))'"
  planned_test_matrix_v2:                    # write these tests WITH the v2 implementation (pending_work.P2/P3)
    - "star and chain topologies form a working circle (non-full-mesh)"
    - "relayed-contribution forgery: injected fake (name, pk) pair rejected or provenance-marked; direct-arrival lock holds"
    - "leave → topup → old key mumbles on new speech; convergence within cascade window"
    - "simultaneous leavers serialize via epoch; same-leaver latecomer is a no-op"
    - "solo circle forms; joiner arrives via ADD topup and both sides rekey correctly"
    - "hall+table: table speech never decrypts under hall key; same line carries both"
    - "50-member flood circle gathers and speaks within patience budget"
  agent_preflight: "read this file fully, then the head-of-file KDoc of any file you touch; for v2 work re-read group_v2 + settled_decisions BEFORE proposing any design change — most ideas you will have were already settled or rejected here"

licensing:
  license: AGPL-3.0-only                    # no commercial exceptions, no dual licensing, ever
  non_commercial_stance: true               # full statement lives in the human READMEs
  commercial_exceptions: never              # do NOT offer, imply, or negotiate dual/commercial licensing on the author's behalf
  sponsorship_channels: none
  readme_license_note: "human READMEs state: AGPL-3.0 only — use must comply in full; resale ignoring AGPL obligations = copyright infringement. When editing READMEs, NEVER re-add 'commercial use requires written permission'-style clauses that conflict with AGPL grants"
  disclaimer_keys:
    - research-and-education-use-only
    - telephone-model-no-identity-auth      # active-MITM risk is accepted by design
    - unaudited-experimental-code           # not for sensitive communications
    - no-warranty-liability-capped-by-law
