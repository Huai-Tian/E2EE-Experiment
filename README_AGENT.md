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
- snow                                   # features: hfs + pqclean_kyber1024
- thiserror
- if-addrs
- spake2                                 # PAKE (pairing secret)
- getrandom
- sha2                                   # group key schedule
- hkdf                                   # group key schedule
- chacha20poly1305                       # group AEAD
lifecycle: single-startup-person           # INVARIANT: no role subcommands; one process is born with both ears and mouth
other_job: "--courier — the same binary as a teahouse: room numbers are GUEST-CHOSEN (agreed out-of-band, same channel as secrets); it wires same-room arrivals, relays opaque bytes; no identity, no storage, no plaintext"
commands:                                  # in-session, parsed by CLI handle_command (src/main.rs)
dial:  { syntax: "/dial ADDR:PORT [secret]", behavior: "outbound connect + hybrid NN handshake as initiator; optional secret → PAKE first; auto-uses chained secret for that addr (burned only when actually verified: PAKE masks exchanged; a failed dial keeps the chained secret); 5s connect timeout" }
meet:  { syntax: "/meet COURIER ROOM [secret]", behavior: "enter a teahouse room; roles assigned by courier (later arrival initiates); optional secret → PAKE inside the room; teahouse lines never chain (one-shot rooms, no anchor)" }
punch: { syntax: "/punch INTRODUCER TAG [secret]", behavior: "register the same tag with the introducer (the teahouse keeper's UDP side-job, same port); he exchanges public endpoints, both sides hole-punch; noise handshake then runs over the punched udp line; roles: later registrant initiates; best-effort — on failure the user re-routes via /meet MANUALLY (no automatic fallback)" }
await: { syntax: "/await SECRET",        behavior: "arm the password challenge for the NEXT inbound arrival (one-shot, consumed on use)" }
talk:  { syntax: "/talk id|name-prefix", behavior: "switch input focus; ambiguous prefix lists candidates" }
list:  { syntax: "/list",                behavior: "enumerate live conversations, mark focused" }
card:  { syntax: "/card",                behavior: "print this person's card: one 'name  ip:port' line per network interface" }
shout: { syntax: "/shout",               behavior: "udp-broadcast who-is-there on the lan; listeners reply name + dial addr; ~1.2s collect window" }
circle: { syntax: "/circle",             behavior: "form a gathering: contribute-and-derive group key over ALL current conversations (requires full mesh; re-run after new joins = rekey)" }
gsay:  { syntax: "/gsay TEXT",           behavior: "speak in the circle: seal ONCE with own subkey, fan out identical blob on every link" }
bye:   { syntax: "/bye",                 behavior: "offer next secret, then empty-plaintext goodbye to focused conversation" }
quit:  { syntax: "/quit | Ctrl-D",       behavior: "goodbye to all conversations (each with secret offer), then exit 0" }
sessions: multiple-concurrent-1to1         # implemented 1:1 feature; ALSO the transport skeleton for group_chat — but group CONTENT crypto is the group key, NOT per-link e2ee; see group_chat
focus_model: "typed lines go to the focused conversation; focus set by /dial or /talk; incoming conversations never steal focus"
symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node
packaging: workspace-three-gates            # root pkg = rust lib `e2ee` + bin; ffi/ member = cdylib gate (libe2ee.so / e2ee.dll, same lib name) — see ffi block
api_style: "library NEVER prints nor reads stdin; actions in = metaphor-verb methods (born/dial/speak/talk_to/roster/card/shout/bye/leave); experiences out = Event stream (tokio mpsc); Event/LeaveReason/PersonError are non_exhaustive; errors carry semantics, CLI renders prose"
conversation_flow:
- noise-nn handshake                     # dialer = initiator
- name exchange                          # first data frame each direction; utf8; unsigned
- bridge                                 # stdin→encrypt→tcp ‖ tcp→decrypt→stdout
- teardown                               # empty-plaintext frame | EOF | Ctrl-C
io_streams: { chat: stdout, events: stderr, input: stdin-line-based }

crypto:
suite_no_secret: "Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s"      # implemented: hybrid X25519+Kyber1024
suite_with_secret: "Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s" # implemented: PAKE output as PSK
pake: "SPAKE2 (spake2 crate, Ed25519Group) — masks exchanged, secret never crosses the wire; wrong secret → loud failure (error mentions 暗号)"
static_keys: none                          # INVARIANT: ephemeral per-connection only
identity_layer: none                       # INVARIANT: no auth / TOFU / trust store / accounts
auth_model: telephone                      # secrecy by default; PAKE = optional per-meeting human-carried secret; identity still judged by humans post-decryption
pfs: implicit                              # fresh keys per connection ⇒ forward secrecy for free
group_layer: implemented-v1                # one negotiated group key + per-sender subkeys (see group_chat)
pq_hybrid: implemented                     # X25519 AND Kyber1024, both must break; NEVER pure-PQ
mid_session_swap: impossible-by-aead       # transport keys live only in the two endpoint processes; an inserted party cannot forge a valid frame — decryption fails loudly and the link drops; wire-cutting = forced hangup, not impersonation.

wire_protocol:
transport: "tcp (or a punched udp line — same framing, one frame per datagram)"
frame: "{u16_be length}{payload}"
frame_cap: 65535
handshake_msgs: 2                          # →e(+e1,kem) | ←e,ee(+ekem1) ; each snow message = one frame; ~1.6KB each (Kyber1024 inside; ≈3.2KB per handshake)
pake_msgs: 2                               # before noise iff secret present: SPAKE2 masks, one frame each (~65B)
meet_timeouts: "every meeting stage (pake / noise handshake / name exchange) bounded at 10s (person::MEET_TIMEOUT) — silent connections are dropped, never held; courier JOIN frame likewise bounded at 10s (courier::JOIN_TIMEOUT), per-connection, accept loop never blocks"
first_app_frame: "kind-tagged encrypted NAME frame (0x05 + utf8 name) — both directions, immediately after handshake; names are application data and NEVER cross the wire in plaintext (the teahouse must stay blind)"
app_frame_kinds:                           # first byte of every encrypted payload
chat: "0x00 + utf8 line"
offer_secret: "0x01 + 32B next-secret (sent before farewell)"
ack_secret: "0x02 (acknowledge an offer; sender then chains it)"
contrib: "0x03 + 32B gathering contribution"
circle_chat: "0x04 + nonce(12B) ‖ group-AEAD (same blob fanned out on every link)"
name: "0x05 + utf8 name (first app frame; consumed by register_convo, ear never sees it)"
unknown_kinds: "silently skipped — forward compatibility"
hangup: "empty plaintext frame (wire = bare 16B AEAD tag) | tcp close"
clean_eof: "read_frame returns Ok(None) iff EOF lands exactly on frame boundary"
teahouse_control:                          # plaintext frames on the courier line, before the splice
join: "'JOIN <room>' — first frame from each arrival; rooms are guest-chosen (out-of-band), NOT courier-assigned; same-number collisions = cross-wired pairs (pick unguessable rooms)"
paired: "'PAIRED R' to the first arrival (responder), 'PAIRED I' to the second (initiator) — roles for the noise handshake"
lan_discovery:                             # side channel, udp — never carries conversation
shout_port: 37777
probe: "E2EEPROBE1 datagram; broadcast to 255.255.255.255 and 127.0.0.1 on the shout port"
reply: 'E2EEREPLY1\n{name}\n{tcp_port} — unicast to the prober'
listener: "bound at born(); a reflex, no events; bind failure tolerated (a second same-host instance cannot co-bind; discovery serves different machines)"
self_filter: "drop a reply iff name == my name AND port == my port AND reply src ip is one of my interface ips"
introducer_and_punch:                      # side channel, udp — addresses only, never plaintext
introducer: "courier::serve_introducer — bound on the SAME port number as the teahouse tcp (udp and tcp namespaces do not collide); plaintext signaling INTRO1 tag / INTRO2 R|I peer; tag pairs are one-shot, ttl 90s, duplicate registration from same addr = refresh; waiting-tag map capped at 1024 entries (MAX_WAITING_TAGS) — unique-tag floods cannot grow memory"
punch: "punch::meet_via — register with retry (3s), connect to peer's public endpoint, exchange PUNCH frames (~150ms apart, 8s window); on first received punch the hole is open; stray punch echoes are swallowed by the reader forever; INTRO2 accepted ONLY from the introducer's own address (forged referrals are dropped)"
transport: "UdpPunch implements AsyncRead+AsyncWrite: one frame per datagram (frames never span datagrams), punch-echo datagrams discarded, max frame 65507B (udp payload limit — a >64KB chat line fails loudly); two background porters (recv→channel, channel→send) die with the halves; NOTE: no liveness detection — a vanished peer leaves the line hanging until /quit"
fallback: "handshake frames ~1.6KB travel as ip fragments; loss = failure; on failure the USER re-routes via /meet manually — no automatic fallback"

modules:
src/lib.rs:     "crate root: pub mod circle + courier + discover + person + punch + wire; re-exports Person/Events/Event/LeaveReason/PersonError/RosterEntry/Discovered"
src/person.rs:  "core person model: born/listen_addr/expect_secret/dial/dial_secret/meet_at_teahouse[_secret]/punch[_secret]/speak/circle_speak/form_circle/talk_to/roster/card/shout/bye/leave; Inner{my_name,listen_port,listen_addr,convos,focus,next_id,events,expecting_secret,chained_secrets,pending_offers,gathering}; handshake<S> is GENERIC over AsyncRead+AsyncWrite+Unpin (tcp and UdpPunch alike); convo halves are BoxedWriter/BoxedReader; ear() dispatches on frame_kind; ZERO printing, ZERO stdin. Security-critical internals: HandshakeFailure/GreetFailure carry a `burned` flag (secret consumed iff SPAKE2 masks were actually exchanged — port scans never burn an armed secret); register_convo encrypts the NAME frame like all app data; Convo.chainable gates secret-chain offers (teahouse lines never offer); dial() keeps the chained secret unless it was actually verified"
src/punch.rs:   "introducer client + punch transport: meet_via (register w/ retry, connect, punch loop, role; INTRO2 source-checked), UdpPunch/PunchReader/PunchWriter (frame-per-datagram, punch-echo swallowing); see wire_protocol.introducer_and_punch"
src/circle.rs:  "group key schedule: derive_group_key (HKDF over byte-sorted contributions), derive_sender_subkey, seal/open (ChaCha20Poly1305, nonce = counter ‖ sender-tag); unit tests for order-independence and subkey isolation"
src/courier.rs: "teahouse: serve() wires same-room arrivals (rooms GUEST-CHOSEN, never assigned; PAIRED R/I role assignment; per-connection JOIN frame bounded at 10s so the accept loop never blocks), splices raw bytes both ways; PLUS serve_introducer on the same port number over udp (INTRO1/INTRO2, tag pairing, ttl 90s, waiting-tag cap 1024)"
src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; distinguishes clean EOF vs mid-frame break"
src/discover.rs: "lan presence: card() interface enumeration via if-addrs; shout() udp probe/reply with self-echo filter; listen_for_shouts() reflex listener spawned at born()"
src/main.rs:    "thin CLI renderer: arg parse (incl. --courier) + select!{stdin lines, event stream}; maps PersonError/Event to zh prose; safe() sanitizes ALL remote-controlled strings (control chars → '·') incl. /shout replies; /talk name-prefix resolution lives here"
ffi/src/lib.rs:   "cdylib gate (see ffi block): 11 exported e2ee_* symbols over the same Person; per-handle tokio runtime"
include/e2ee.h:   "C header — MUST change together with ffi/src/lib.rs (frozen ABI)"
ffi/ffi_test.c:   "C contract test: full lifecycle incl. PAKE, secret chain, wrong-secret and no-focus error paths; prints FFI OK"
ffi/ctypes_test.py: "python ctypes round-trip: create/local_addr/await_secret/dial/speak/poll/destroy — proves any-C-language reachability"
tests/conversation.rs: "14 integration tests: real Persons on ephemeral ports, event-stream-only assertions — talk/farewell/focus/card/teahouse(×2)/secret(×2)/secret-chain/circle/punch + scanner-does-not-burn-secret/mute-connection-does-not-block-teahouse/teahouse-no-chain"
tests/pq.rs:       "hybrid suite round-trip: NNhfs + NNpsk0+hfs both parse, handshake, and transport"

invariants:                                  # ANY change violating these MUST be rejected
- zero-persistence: no fs writes ever — no logs, config, cache, keys, history, state files
- plaintext-containment: plaintext allowed only in stdin-buffer, stdout-write, RAM; never in errors/args/env
- e2ee-only-transit: application data crosses the wire (tcp or punched udp) exclusively inside noise transport frames
- no-identity-layer: never add authentication, TOFU, key continuity, accounts, fingerprint checking
- symmetry: never introduce server/client asymmetry or a node with more power than a peer
- ephemeral-keys: every connection = fresh keypair; zero key reuse across anything
- attribution-by-topology: displayed name MUST come from the receiving link, never from a self-claim inside plaintext
- discovery-side-channel: lan discovery (shout/card) carries name and address only — never conversation plaintext, never secrets

permitted_extensions:                       # extend only along the metaphor
nat_traversal:    { metaphor: introducer, status: implemented-v1, rule: "udp signaling carries addresses only; must never see plaintext; punch path is best-effort — on failure the user re-routes via the teahouse manually" }
ciphertext_relay: { metaphor: courier,   rule: "forward opaque frames only; store nothing, log nothing — concretized as the teahouse, see discovery_and_pairing" }
ipv6:             { metaphor: same-room }
multiparty:       { metaphor: small-gathering, rule: "design settled — see group_chat section" }
ffi_bindings:     { metaphor: teaching-the-verbs, status: implemented-v1, rule: "see ffi block; add-only, frozen signatures forever" }

forbidden_extensions:                       # belong to a different project, not this one
- offline message queue                   # breaks ephemerality
- history / sync / multi-device           # breaks zero-persistence
- identity verification of any kind       # breaks no-identity-layer
- accounts / registration                 # breaks symmetry + no-identity-layer

group_chat:                                 # STATUS: implemented (v1)
status: implemented-v1
v1_boundaries: "full mesh is manual (dial each peer before /circle); rekey = re-run /circle after new joins; automatic introduction brokering is future work"
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
contribute: "each member generates fresh 32-byte random r_i; broadcasts to all over its noise links (0x03 frames; receipt self-propagates: first receipt makes a non-member contribute too)"
derive: "K = HKDF(byte-sorted concatenation of all r_i)"   # order-independent; no member list needed; circle.rs::derive_group_key
per_sender_subkey: "s_i = HKDF(K, r_i)" # r_i doubles as sender label; prevents nonce collision under shared key
send: "encrypt ONCE with s_i (ChaCha20Poly1305, nonce = counter ‖ sender-tag); identical blob fanned out on every link inside 0x04 frames"
recv: "decrypt with s_j derived from the RECEIVING link's contribution r_j — attribution by topology, implemented"
noise_role_after_join: escort-only        # pairwise channels wire the mesh and escort the negotiation; they do NOT carry per-link content encryption
rekey_policy:
trigger: membership-join-only           # never on leave
rationale: "leaver has no live link to listen; zero persistence leaves nothing to decrypt later; next join re-keys anyway"
eviction: "remaining members re-run contribution = fresh gathering; same mechanism as join, nothing new"
race_handling: "simultaneous /circle runs may yield inconsistent member sets (a later form_circle replaces the gathering wholesale); AEAD decrypt failure ⇒ CircleMumble event (a notice, never silence) — re-run /circle to converge"
honest_limitation: "any member can derive others' subkeys (r_i is broadcast) but cannot impersonate — injection requires a link only its owner has; crypto gives secrecy, topology gives attribution"

discovery_and_pairing:                      # how two people (possibly behind NAT walls) find each other
status: implemented                        # shout/card/teahouse/PAKE/secret-chain all live
principle: "the system forgets, humans remember — the address book lives OUTSIDE the system (paper, memory); same shape as 'recognizing the person is your job'"
rungs_cheapest_first:
- { name: shout,    status: implemented, metaphor: same-room-call,    mechanism: "udp broadcast who-is-there; lan listeners answer name + dial addr" }
- { name: card,     status: implemented, metaphor: business-card,     mechanism: "print name + ip:port per interface; hand it over out-of-band" }
- { name: teahouse, status: implemented, metaphor: courier-who-introduces, mechanism: "same binary --courier; JOIN room / PAIRED R-I / raw splice" }
teahouse:                                 # crosses two NAT walls; both sides dial OUT (a NAT never blocks leaving) ⇒ always works
mode: "--courier on the same binary"    # deployed on any public-ip machine; a tiny vps suffices
not_privileged: "no identity, no storage, no plaintext; anyone can open one; no teahouse is nobler than another — symmetry intact"
room_number: "GUEST-CHOSEN (out-of-band, like the secret), public; NOT courier-assigned — two pairs picking the same number get cross-wired, so rooms must be picked unguessable; it is NOT a secret"
pairing_protocol: "both sides dial the teahouse quoting the same room number; the teahouse splices the two lines; the Noise handshake then runs end-to-end THROUGH the spliced line — the teahouse relays frames it cannot read"
sees: "who / when / how many bytes"
sees_not: "any content — it sits outside the encryption loop; frames are opaque bytes (names included: the NAME frame is Noise-encrypted)"
pairing_secret:                           # user-chosen seed; rolling secrets are system-generated INSIDE the channel
ownership: "humans choose the FIRST secret and carry it out-of-band (phone, face to face); its strength is a human responsibility — same shape as recognizing a voice; rolling secrets in the chain are the system's job, generated inside the authenticated channel, RAM only"
separation: "room number (public, anti-cross-talk) and secret (private, anti-impersonation) are ALWAYS two different things"
pake: "SPAKE2 — the secret never crosses the wire in derivable form; a wrong secret fails mathematically; offline brute-force does not exist; online guessing is one-shot per arming: a verified-wrong guess burns the armed secret, a connection that never reaches PAKE does NOT (scanner-proof)"
key_schedule: "PAKE output only ignites the Noise handshake (PSK mode); ALL conversation keys are fresh per-session ephemerals — the secret never talks; everything burns at session end"
secret_chain:                             # automatic renewal, within RAM only
mechanism: "at farewell, reserve the NEXT secret inside the already-authenticated channel — offer + ack; an unacknowledged reservation is dropped (failure loses convenience, never security); an incoming offer never clobbers an already-armed /await slot (no arm, no ack — the offer expires)"
anchor: "the chain hangs on a peer ADDRESS: direct dials work; punched lines anchor to the peer's public NAT endpoint (may be gone next session — the reservation sits unused, harmless); teahouse lines never chain (Convo.chainable = false — one-shot rooms, no anchor; otherwise the armed secret would block the next plain /meet)"
dial_burn_semantics: "dial() auto-uses the chained secret for that addr and removes it ONLY when actually verified (handshake success, or PAKE masks exchanged) — a dial that never connects keeps the reservation for the next try"
lifespan: "intersection of both process lifetimes; a restart returns to the one out-of-band reading"
trust_root: "the single out-of-band first secret; every later secret is born inside the channel the previous one protected"
red_line: "NEVER persist a rolling secret to disk — a persisted secret is a long-term credential, i.e. the deleted 'face'; zero-persistence forbids it"
multiparty_over_teahouse:                 # a room seats exactly TWO; the gathering weaves its own mesh
one_room_per_link: "N members = N(N-1)/2 two-person rooms; the introducer brokers room number + secret to both sides over existing encrypted links"
unforgeable: "pairwise Noise per link makes relayed frames unforgeable and uncorrelatable across rooms"
group_keys_unchanged: "the group-key schedule lives one floor above the wiring — the teahouse changes nothing (see group_chat)"

pq_hybrid:                                  # STATUS: implemented
status: implemented                        # snow 0.10 features hfs + pqclean_kyber1024; suites in crypto block
threat: "HNDL — harvest now, decrypt later: a passive recorder stores today's traffic until a quantum computer (Shor) breaks X25519"
why_critical_here: "the handshake is the WHOLE crypto moment (no ratchets, one key per session) AND the secret chain forwards each next secret through the previous session — a future-broken handshake leaks the next secret, turning the recorder from an ear into a mouth (impersonation)"
scheme: hybrid                            # SETTLED DECISION: X25519 AND Kyber1024 combined (snow pqclean_kyber1024; ML-KEM-768 crate swap is a drop-in when snow ships it)
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
costs: "≈ +3.1 KB per handshake (initiator 1600B + responder 1632B, measured; classical NN is ~64B); ≈ +200-300 KB binary; tens of µs per handshake"
priority: same-batch-as-pake              # PAKE stops tonight's active impersonator, PQ stops tomorrow's passive reader; neither substitutes the other

ffi:                                        # STATUS: implemented (v1 core surface)
status: implemented-v1
layout: "workspace member ffi/ (package e2ee-ffi, cdylib crate name e2ee) over the root rlib; header include/e2ee.h; contract test ffi/ffi_test.c; python proof ffi/ctypes_test.py"
surface: "11 exported symbols: e2ee_version / e2ee_person_create / e2ee_person_destroy / e2ee_local_addr / e2ee_last_error / e2ee_await_secret / e2ee_dial / e2ee_speak / e2ee_bye / e2ee_leave / e2ee_poll"
freeze_rules:                             # HARD — ABI is a published contract
- "include/e2ee.h and ffi/src/lib.rs MUST change together (same commit)"
- "add-only: new functions allowed; changing an existing signature or struct layout NEVER"
- "e2ee_event layout frozen: fixed 64B name / 512B text buffers, truncating copy, always NUL-terminated"
- "unknown Event variants map to tag 0 (downstream non_exhaustive arm) — old consumers survive new events"
model: "opaque handle; per-handle private tokio runtime (multi_thread) so ears stay open between calls; destroy = farewell + runtime drop (all background tasks die, port released)"
returns: "0 = ok, -1 = fail; human-readable message via e2ee_last_error (thread-local, zh, valid until next call on that thread)"
poll: "returns 1 = event, 0 = timeout, -1 = error; events you skip are DISCARDED; timeout_ms < 0 blocks forever, 0 non-blocking"
threading: "handle lifetime (create/destroy) managed serially on one thread; never destroy while another thread is still polling"
secrets: "dial's secret param is the SPAKE2 pairing secret; NULL = plain handshake (auto-uses a chained secret if one exists for that address)"
not_yet_exposed: "teahouse / circle / shout / card / roster — add on demand under the same freeze rules"

verify:
build: "cargo build --release"
binary: "target/release/E2EE-Experiment"
test: "cargo test --release — 17 tests total: 2 circle unit + 14 integration (conversation.rs: talk/farewell/focus/card/teahouse×2/secret×2/secret-chain/circle/punch + scanner-does-not-burn-secret/mute-teahouse/teahouse-no-chain) + 1 pq suite; in-process real persons on ephemeral localhost ports; ~4s"
ffi_build: "cargo build --release -p e2ee-ffi — target/release/libe2ee.so (~1.0MB, measured 999KB); windows: cargo build --release -p e2ee-ffi --target x86_64-pc-windows-gnu → e2ee.dll (~860KB, measured 881KB); verify symbols: nm -D target/release/libe2ee.so | grep ' T e2ee_' (expect 11)"
ffi_contract_test: "gcc -O2 -Wall -I include ffi/ffi_test.c -o /tmp/ffi_test -L target/release -l:libe2ee.so -Wl,-rpath,$PWD/target/release && /tmp/ffi_test — expect last line 'FFI OK'; then python3 ffi/ctypes_test.py — expect 'ctypes OK'"
static_build: "cargo build --release --target x86_64-unknown-linux-musl — static-pie ~1.3MB (measured 1,320KB, Kyber1024 included), runs anywhere"
windows_build: "cargo build --release --target x86_64-pc-windows-gnu — PE32+ .exe ~1.0MB (measured 1,046KB; compile-verified; runtime untested without a Windows host)"
wire_confidentiality_check: "tcpdump -i lo -w /tmp/lo.pcap 'tcp port 17777' during a two-instance conversation, then grep -a for names/chat words in the pcap — MUST find nothing (names and chat are Noise-encrypted; pre-0x05 the name frame crossed the wire in cleartext)"
yaml_selfcheck: "pip install pyyaml && python3 -c 'import yaml; yaml.safe_load(open(\"README_AGENT.md\"))' — MUST succeed; the whole file is one YAML document (the header block is comments)"
smoke_test: |
local instances on distinct ports: `... -n A 127.0.0.1:17777`, `... -n B 127.0.0.1:17778`, then B sends `/dial 127.0.0.1:17777`
expect: each side prefixes received lines with the peer name; `/bye` → peer prints 挂断了
three-instance: second inbound connection does NOT steal focus (prints 找上门来 hint); `/talk <id>` switches; `/quit` goodbyes all
chain-reuse via CLI (regression: CLI once bypassed dial()'s chain consumption): A `/await s1` → B `/dial A s1` → B `/bye` → B `/dial A` (no secret) → expect 对话 #2 — NOT "input error"
circle via CLI: three instances dial each other (full mesh), one `/circle`, all print 圈子铸成了, `/gsay` heard by both others
punch via CLI: run `--courier 127.0.0.1:18888` (now also introducer on udp), two instances `/punch 127.0.0.1:18888 tag [secret]`, expect 对话开始 + bidirectional talk over the punched udp line
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

known_limitations:                          # honest edges of v1 — do NOT paper over these in docs
no_liveness_probes: "established conversations have no keepalive/idle timeout — a peer vanishing without close (or over punched udp, which has no EOF) leaves the line hanging until /quit; meeting stages ARE bounded (10s each)"
introducer_referral_unauthenticated: "anyone knowing the tag can register as the 'peer' — the tag is a rendezvous, not an identity; meet_via only accepts INTRO2 from the introducer itself, but the introducer trusts tag-holders; bring a secret for authentication"
teahouse_pairs_by_room_number: "same number = cross-wired pairs; rooms must be picked unguessable"
group_membership_by_convention: "manual full mesh; simultaneous re-/circle can leave members with mismatched group keys — surfaced as CircleMumble, fixed by re-circling; no roster arbitrates (on purpose)"
one_gathering_per_process: "/circle covers ALL live conversations; a process cannot sit in two circles"

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
介绍人:    teahouse keeper's side job — udp address exchange (INTRO1 tag → INTRO2 R/I peer), same port as tcp
递拳头:    punch — repeated PUNCH frames to the peer's public endpoint until one comes back
打洞:      udp hole punching; the resulting line is a UdpPunch (one frame per datagram, echoes swallowed)
名片:      /card — name + per-interface dial addresses
喊一嗓子:  /shout — lan udp broadcast discovery
茶馆:      "--courier two-person-room relay (implemented); keeper also serves as introducer on udp"
暗号:      user-chosen pairing secret, carried out-of-band; SPAKE2-verified, chains at farewell (implemented)
