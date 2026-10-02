# ═══════════════════════════════════════════════════════════════
# README_AGENT — machine-facing project brief
#
# FILE CLASS : machine-facing brief. Audience: LLM agents / coding copilots.
# HUMANS     : stop here. Use README.md / README_ZH.md instead.
# FORMAT     : yaml-dialect. indent=2; keys are stable IDs; `#` comments are
#              NORMATIVE (constraints), not decoration. File body is pure YAML.
#              STRUCTURAL INVARIANT: the whole file MUST parse as ONE valid
#              YAML document — lost indentation silently destroys meaning
#              (a group_chat section once collapsed into orphan keys this way).
# RATIONALE  : english keys+values tokenize efficiently across model families;
#              glossary maps the chinese person-metaphors used in code comments.
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
maturity: mvp
ui_language: zh                            # user-facing strings are chinese; keep style
comment_language: zh-with-person-metaphors # see glossary at bottom

doc_index:                                  # entry alignment across the 3 docs
zh: README_ZH.md
en: README.md
agent: README_AGENT.md                    # this file

architecture:
binary_count: 1                            # INVARIANT: deliverable is exactly one executable
deps:
- tokio
- snow
- thiserror
- if-addrs
lifecycle: single-startup-person           # INVARIANT: no role subcommands; one process is born with both ears and mouth
commands:                                  # in-session, parsed by CLI handle_command (src/main.rs)
dial:  { syntax: "/dial ADDR:PORT",      behavior: "outbound connect + NN handshake as initiator; becomes focus; 5s connect timeout" }
talk:  { syntax: "/talk id|name-prefix", behavior: "switch input focus; ambiguous prefix lists candidates" }
list:  { syntax: "/list",                behavior: "enumerate live conversations, mark focused" }
card:  { syntax: "/card",                behavior: "print this person's card: one 'name  ip:port' line per network interface" }
shout: { syntax: "/shout",               behavior: "udp-broadcast who-is-there on the lan; listeners reply name + dial addr; ~1.2s collect window" }
bye:   { syntax: "/bye",                 behavior: "empty-plaintext goodbye to focused conversation; ear task completes teardown" }
quit:  { syntax: "/quit | Ctrl-D",       behavior: "goodbye to all conversations, then exit 0" }
sessions: multiple-concurrent-1to1         # implemented 1:1 feature; ALSO the transport skeleton for group_chat — but group CONTENT crypto is the group key, NOT per-link e2ee; see group_chat
focus_model: "typed lines go to the focused conversation; focus set by /dial or /talk; incoming conversations never steal focus"
symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node
packaging: lib-plus-bin                    # single package: lib `e2ee` (for embedding) + bin `E2EE-Experiment` (first consumer); future C ABI deferred until verbs stabilize
api_style: "library NEVER prints nor reads stdin; actions in = metaphor-verb methods (born/dial/speak/talk_to/roster/card/shout/bye/leave); experiences out = Event stream (tokio mpsc); Event/LeaveReason/PersonError are non_exhaustive; errors carry semantics, CLI renders prose"
conversation_flow:
- noise-nn handshake                     # dialer = initiator
- name exchange                          # first data frame each direction; utf8; unsigned
- bridge                                 # stdin→encrypt→tcp ‖ tcp→decrypt→stdout
- teardown                               # empty-plaintext frame | EOF | Ctrl-C
io_streams: { chat: stdout, events: stderr, input: stdin-line-based }

crypto:
suite: Noise_NN_25519_ChaChaPoly_BLAKE2s   # implemented 1:1 layer
static_keys: none                          # INVARIANT: ephemeral per-connection only
identity_layer: none                       # INVARIANT: no auth / TOFU / trust store / accounts
auth_model: telephone                      # secrecy only; identity judged by humans post-decryption
pfs: implicit                              # fresh keys per connection ⇒ forward secrecy for free
group_layer: designed-not-implemented      # one negotiated group key + per-sender subkeys — see group_chat; NOT "many independent 1:1 e2ee sessions"
pq_hybrid: designed-with-pake-batch        # see pq_hybrid block below; ships together with PAKE (same handshake function)
mid_session_swap: impossible-by-aead       # transport keys live only in the two endpoint processes; an inserted party cannot forge a valid frame — decryption fails loudly and the link drops; wire-cutting = forced hangup, not impersonation. The ONLY exposure window is the handshake instant itself.

wire_protocol:
transport: tcp
frame: "{u16_be length}{payload}"
frame_cap: 65535
handshake_msgs: 2                          # →e | ←e,ee ; each snow message = one frame
first_app_frame: utf8_name                 # both directions, immediately after handshake
chat_frame: "one utf8 line per frame"
hangup: "empty plaintext frame (wire = bare 16B AEAD tag) | tcp close"
clean_eof: "read_frame returns Ok(None) iff EOF lands exactly on frame boundary"
lan_discovery:                             # side channel, udp — never carries conversation
shout_port: 37777
probe: "E2EEPROBE1 datagram; broadcast to 255.255.255.255 and 127.0.0.1 on the shout port"
reply: 'E2EEREPLY1\n{name}\n{tcp_port} — unicast to the prober'
listener: "bound at born(); a reflex, no events; bind failure tolerated (a second same-host instance cannot co-bind; discovery serves different machines)"
self_filter: "drop a reply iff name == my name AND port == my port AND reply src ip is one of my interface ips"

modules:
src/lib.rs:     "crate root: pub mod person + wire + discover; re-exports Person/Events/Event/LeaveReason/PersonError/RosterEntry/Discovered; lib-first packaging documented"
src/person.rs:  "core person model: born/dial/speak/talk_to/roster/card/shout/bye/leave; Inner{my_name,listen_port,convos,focus,next_id,events}; free greet() (inbound failures → MeetFailed event, dial failures → PersonError); one ear task per conversation tears itself down; inbound arrivals never steal focus; ZERO printing, ZERO stdin"
src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; distinguishes clean EOF vs mid-frame break"
src/discover.rs: "lan presence: card() interface enumeration via if-addrs; shout() udp probe/reply with self-echo filter; listen_for_shouts() reflex listener spawned at born()"
src/main.rs:    "thin CLI renderer: arg parse + select!{stdin lines, event stream}; maps PersonError/Event to zh prose; /talk name-prefix resolution lives here"

invariants:                                  # ANY change violating these MUST be rejected
- zero-persistence: no fs writes ever — no logs, config, cache, keys, history, state files
- plaintext-containment: plaintext allowed only in stdin-buffer, stdout-write, RAM; never in errors/args/env
- e2ee-only-transit: application data crosses tcp exclusively inside noise transport frames
- no-identity-layer: never add authentication, TOFU, key continuity, accounts, fingerprint checking
- symmetry: never introduce server/client asymmetry or a node with more power than a peer
- ephemeral-keys: every connection = fresh keypair; zero key reuse across anything
- attribution-by-topology: displayed name MUST come from the receiving link, never from a self-claim inside plaintext
- discovery-side-channel: lan discovery (shout/card) carries name and address only — never conversation plaintext, never secrets

permitted_extensions:                       # extend only along the metaphor
nat_traversal:    { metaphor: introducer, rule: "udp hole-punch coordination; must never see plaintext" }
ciphertext_relay: { metaphor: courier,   rule: "forward opaque frames only; store nothing, log nothing — concretized as the teahouse, see discovery_and_pairing" }
ipv6:             { metaphor: same-room }
multiparty:       { metaphor: small-gathering, rule: "design settled — see group_chat section" }
ffi_bindings:     { metaphor: teaching-the-verbs, rule: "C ABI mirrors the same verbs; add no hidden state, widen no trust surface; only after API stabilizes" }

forbidden_extensions:                       # belong to a different project, not this one
- offline message queue                   # breaks ephemerality
- history / sync / multi-device           # breaks zero-persistence
- identity verification of any kind       # breaks no-identity-layer
- accounts / registration                 # breaks symmetry + no-identity-layer

group_chat:                                 # STATUS: designed, NOT implemented
status: designed-not-implemented
scheme: group-key                         # SETTLED DECISION: one negotiated group key + per-sender subkeys (see group_key_schedule)
rejected_alternative: >-                  # do NOT reintroduce; owner explicitly settled on group-key
pairwise re-encryption per link — the same plaintext Noise-encrypted separately
on every link, i.e. "group = many independent 1:1 e2ee sessions". Considered
and rejected; the current multiple-concurrent-1to1 sessions are TRANSPORT
ONLY, not the group content crypto.
prerequisite: "harden and test the 1:1 crypto core first — owner's explicit sequencing"
topology: full-mesh                       # every member holds a direct link to every other; N² links; practical cap ~8
joining:
- newcomer dials any current member     # pairwise Noise NN per link, unchanged
- introducer sends address list ONLY    # never relays conversation frames
- newcomer dials each remaining member  # independent NN handshake per link
membership: presence-based                # member == live connection; no roster stored anywhere
group_key_schedule:                       # the room language — THE group content crypto
contribute: "each member generates fresh 32-byte random r_i; broadcasts to all over its noise links"
derive: "K = HKDF(byte-sorted concatenation of all r_i)"   # order-independent; no member list needed
per_sender_subkey: "s_i = HKDF(K, r_i)" # r_i doubles as sender label; prevents nonce collision under shared key
send: "encrypt ONCE with s_i; identical ciphertext fanned out on every link"
noise_role_after_join: escort-only        # pairwise channels wire the mesh and escort the negotiation; they do NOT carry per-link content encryption
rekey_policy:
trigger: membership-join-only           # never on leave
rationale: "leaver has no live link to listen; zero persistence leaves nothing to decrypt later; next join re-keys anyway"
eviction: "remaining members re-run contribution = fresh gathering; same mechanism as join, nothing new"
race_handling: "simultaneous joiners may yield inconsistent member sets; wait ~500ms of membership silence before contributing; AEAD decrypt failure ⇒ re-contribute"
honest_limitation: "any member can derive others' subkeys (r_i is broadcast) but cannot impersonate — injection requires a link only its owner has; crypto gives secrecy, topology gives attribution"

discovery_and_pairing:                      # how two people (possibly behind NAT walls) find each other
status: partially-implemented             # card+shout live; teahouse / PAKE / secret-chain designed only
principle: "the system forgets, humans remember — the address book lives OUTSIDE the system (paper, memory); same shape as 'recognizing the person is your job'"
rungs_cheapest_first:
- { name: shout,    status: implemented, metaphor: same-room-call,    mechanism: "udp broadcast who-is-there; lan listeners answer name + dial addr" }
- { name: card,     status: implemented, metaphor: business-card,     mechanism: "print name + ip:port per interface; hand it over out-of-band" }
- { name: teahouse, status: designed,    metaphor: courier-who-introduces, mechanism: "see teahouse below" }
teahouse:                                 # crosses two NAT walls; both sides dial OUT (a NAT never blocks leaving) ⇒ always works
mode: "--courier on the same binary"    # deployed on any public-ip machine; a tiny vps suffices
not_privileged: "no identity, no storage, no plaintext; anyone can open one; no teahouse is nobler than another — symmetry intact"
room_number: "courier-assigned, public, unique per live pairing; prevents cross-talk only — it is NOT a secret"
pairing_protocol: "both sides dial the teahouse quoting the same room number; the teahouse splices the two lines; the Noise handshake then runs end-to-end THROUGH the spliced line — the teahouse relays frames it cannot read"
sees: "who / when / how many bytes"
sees_not: "any content — it sits outside the encryption loop; frames are opaque bytes"
pairing_secret:                           # user-chosen; NEVER system-generated, NEVER stored
ownership: "humans choose it and carry it out-of-band (phone, face to face); its strength is a human responsibility — same shape as recognizing a voice"
separation: "room number (public, anti-cross-talk) and secret (private, anti-impersonation) are ALWAYS two different things"
pake: "SPAKE2 — the secret never crosses the wire in derivable form; a wrong secret fails mathematically; offline brute-force does not exist; online guessing is rate-limitable at the teahouse"
key_schedule: "PAKE output only ignites the Noise handshake (PSK mode); ALL conversation keys are fresh per-session ephemerals — the secret never talks; everything burns at session end"
secret_chain:                             # automatic renewal, within RAM only
mechanism: "at farewell, reserve the NEXT secret inside the already-authenticated channel — offer + ack; an unacknowledged reservation is dropped (failure loses convenience, never security)"
lifespan: "intersection of both process lifetimes; a restart returns to the one out-of-band reading"
trust_root: "the single out-of-band first secret; every later secret is born inside the channel the previous one protected"
red_line: "NEVER persist a rolling secret to disk — a persisted secret is a long-term credential, i.e. the deleted 'face'; zero-persistence forbids it"
multiparty_over_teahouse:                 # a room seats exactly TWO; the gathering weaves its own mesh
one_room_per_link: "N members = N(N-1)/2 two-person rooms; the introducer brokers room number + secret to both sides over existing encrypted links"
unforgeable: "pairwise Noise per link makes relayed frames unforgeable and uncorrelatable across rooms"
group_keys_unchanged: "the group-key schedule lives one floor above the wiring — the teahouse changes nothing (see group_chat)"

pq_hybrid:                                  # STATUS: designed, NOT implemented; ships in the same batch as PAKE (both touch only the handshake function)
status: designed-not-implemented
threat: "HNDL — harvest now, decrypt later: a passive recorder stores today's traffic until a quantum computer (Shor) breaks X25519"
why_critical_here: "the handshake is the WHOLE crypto moment (no ratchets, one key per session) AND the secret chain forwards each next secret through the previous session — a future-broken handshake leaks the next secret, turning the recorder from an ear into a mouth (impersonation)"
scheme: hybrid                            # SETTLED DECISION: X25519 AND ML-KEM-768 combined
combined_rule: "attacker must break BOTH assumptions to win; either one surviving keeps the session secret"
red_line: "NEVER pure-PQ replacement — new math is young (SIKE fell to a classical laptop in 2022); the hybrid keeps the 40-year classical lock as a floor"
implementation_options:
- "A: snow with pqc feature (hybrid Noise ciphersuite string) if available in the current snow version"
- "B: manual WireGuard-style hybrid — ml-kem crate encapsulation, shared secret fed as PSK into Noise_NNpsk0 (the SAME door PAKE uses)"
scope:
handshake: hybrid                       # the only place that changes
pake: classical                         # a future-broken PAKE recording only ever leaks an already-consumed secret
group_keys: unaffected                  # live one floor above, over the (now hybrid) noise links
secret_chain: protected_transitively    # every next secret is born inside a hybrid envelope
costs: "≈ +2.2 KB per handshake (fits u16 frames trivially); ≈ +200-300 KB binary; tens of µs per handshake"
priority: same-batch-as-pake              # PAKE stops tonight's active impersonator, PQ stops tomorrow's passive reader; neither substitutes the other

verify:
build: "cargo build --release"
binary: "target/release/E2EE-Experiment"
static_build: "rustup target add x86_64-unknown-linux-musl && cargo build --release --target x86_64-unknown-linux-musl"
yaml_selfcheck: "pip install pyyaml && python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))' — MUST succeed; the whole file is one YAML document (the header block is comments)"
smoke_test: |
local instances on distinct ports: `... -n A 127.0.0.1:17777`, `... -n B 127.0.0.1:17778`, then B sends `/dial 127.0.0.1:17777`
expect: each side prefixes received lines with the peer name; `/bye` → peer prints 挂断了
three-instance: second inbound connection does NOT steal focus (prints 找上门来 hint); `/talk <id>` switches; `/quit` goodbyes all
card: `/card` prints one `name  ip:port` line per interface
shout: B `/shout` discovers A (name + dial addr); same-host note: only the FIRST instance binds shout port 37777 — later instances listen-deafen but can still shout

licensing:
license: AGPL-3.0
non_commercial: true                      # full statement lives in the human READMEs
sponsorship_channels: none
resale_prohibited: true
disclaimer_keys:                          # machine summary of the human disclaimer
- research-and-education-use-only
- telephone-model-no-identity-auth      # active-MITM risk is accepted by design
- unaudited-experimental-code           # not for sensitive communications
- no-warranty-liability-capped-by-law

glossary:                                   # chinese metaphor → mechanism (used in code comments)
耳朵:      inbound tcp accept loop (person::born acceptor; one ear task per conversation)
出生:      Person::born — bind + spawn acceptor, returns (Person, Events)
嘴:        outbound tcp connect (dial)
见面:      noise nn handshake
密谈:      noise transport phase
挂断:      session teardown
道别:      empty-plaintext goodbye frame
转身即忘:  zero persistence
自报家门:  unsigned self-reported name frame
无名氏:    default name when -n omitted
初次见面:  no key continuity — every connection starts from scratch
找人:      /dial outbound connect
注意力:    focus — which conversation receives typed lines
收摊:      conversation teardown by the ear task
名片:      /card — name + per-interface dial addresses
喊一嗓子:  /shout — lan udp broadcast discovery
茶馆:      "--courier two-person-room relay (designed)"
暗号:      user-chosen pairing secret, carried out-of-band (designed)
