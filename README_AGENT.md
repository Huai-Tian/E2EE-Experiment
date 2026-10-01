# ═══════════════════════════════════════════════════════════════
# README_AGENT — machine-facing project brief
#
# FILE CLASS : machine-facing brief. Audience: LLM agents / coding copilots.
# HUMANS     : stop here. Use README.md / README_ZH.md instead.
# FORMAT     : yaml-dialect. indent=2; keys are stable IDs; `#` comments are
#              NORMATIVE (constraints), not decoration. File body is pure YAML.
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
roles:
listen: { module: session::listen, metaphor: ear,  behavior: "tcp bind+accept; serves visitors serially; keeps listening after each conversation" }
dial:   { module: session::dial,   metaphor: mouth, behavior: "tcp connect; one conversation; process exits after" }
symmetry: peer-only                        # INVARIANT: no server/client split, no privileged node
conversation_flow:
- noise-nn handshake                     # dialer = initiator
- name exchange                          # first data frame each direction; utf8; unsigned
- bridge                                 # stdin→encrypt→tcp ‖ tcp→decrypt→stdout
- teardown                               # empty-plaintext frame | EOF | Ctrl-C
io_streams: { chat: stdout, events: stderr, input: stdin-line-based }

crypto:
suite: Noise_NN_25519_ChaChaPoly_BLAKE2s
static_keys: none                          # INVARIANT: ephemeral per-connection only
identity_layer: none                       # INVARIANT: no auth / TOFU / trust store / accounts
auth_model: telephone                      # secrecy only; identity judged by humans post-decryption
pfs: implicit                              # fresh keys per connection ⇒ forward secrecy for free

wire_protocol:
transport: tcp
frame: "{u16_be length}{payload}"
frame_cap: 65535
handshake_msgs: 2                          # →e | ←e,ee ; each snow message = one frame
first_app_frame: utf8_name                 # both directions, immediately after handshake
chat_frame: "one utf8 line per frame"
hangup: "empty plaintext frame (wire = bare 16B AEAD tag) | tcp close"
clean_eof: "read_frame returns Ok(None) iff EOF lands exactly on frame boundary"

modules:
src/main.rs:    "cli: subcommand listen|dial, optional positional addr, optional -n name; default addr 0.0.0.0:7777; help text inline; exit codes 0/1/2"
src/wire.rs:    "framing primitives; generic over AsyncRead/AsyncWrite; distinguishes clean EOF vs mid-frame break"
src/session.rs: "listen/dial/converse/handshake/send; tokio select! bridges ear-task vs stdin; farewell wait 800ms"

invariants:                                  # ANY change violating these MUST be rejected
- zero-persistence: no fs writes ever — no logs, config, cache, keys, history, state files
- plaintext-containment: plaintext allowed only in stdin-buffer, stdout-write, RAM; never in errors/args/env
- e2ee-only-transit: application data crosses tcp exclusively inside noise transport frames
- no-identity-layer: never add authentication, TOFU, key continuity, accounts, fingerprint checking
- symmetry: never introduce server/client asymmetry or a node with more power than a peer
- ephemeral-keys: every connection = fresh keypair; zero key reuse across anything
- attribution-by-topology: displayed name MUST come from the receiving link, never from a self-claim inside plaintext

permitted_extensions:                       # extend only along the metaphor
nat_traversal:    { metaphor: introducer, rule: "udp hole-punch coordination; must never see plaintext" }
ciphertext_relay: { metaphor: courier,   rule: "forward opaque frames only; store nothing, log nothing" }
ipv6:             { metaphor: same-room }
multiparty:       { metaphor: small-gathering, rule: "design settled — see group_chat section" }

forbidden_extensions:                       # belong to a different project, not this one
- offline message queue                   # breaks ephemerality
- history / sync / multi-device           # breaks zero-persistence
- identity verification of any kind       # breaks no-identity-layer
- accounts / registration                 # breaks symmetry + no-identity-layer

group_chat:                                 # STATUS: designed, NOT implemented
status: designed-not-implemented
prerequisite: "harden and test the 1:1 crypto core first — owner's explicit sequencing"
topology: full-mesh                       # every member holds a direct link to every other; N² links; practical cap ~8
joining:
- newcomer dials any current member     # pairwise Noise NN per link, unchanged
- introducer sends address list ONLY    # never relays conversation frames
- newcomer dials each remaining member  # independent NN handshake per link
membership: presence-based                # member == live connection; no roster stored anywhere
group_key_schedule:                       # the room language
contribute: "each member generates fresh 32-byte random r_i; broadcasts to all over its noise links"
derive: "K = HKDF(byte-sorted concatenation of all r_i)"  # order-independent; no member list needed
per_sender_subkey: "s_i = HKDF(K, r_i)" # r_i doubles as sender label; prevents nonce collision under shared key
send: "encrypt ONCE with s_i; identical ciphertext fanned out on every link"
noise_role_after_join: escort-only        # pairwise channels wire the mesh and escort the negotiation
rekey_policy:
trigger: membership-join-only           # never on leave
rationale: "leaver has no live link to listen; zero persistence leaves nothing to decrypt later; next join re-keys anyway"
eviction: "remaining members re-run contribution = fresh gathering; same mechanism as join, nothing new"
race_handling: "simultaneous joiners may yield inconsistent member sets; wait ~500ms of membership silence before contributing; AEAD decrypt failure ⇒ re-contribute"
honest_limitation: "any member can derive others' subkeys (r_i is broadcast) but cannot impersonate — injection requires a link only its owner has; crypto gives secrecy, topology gives attribution"

verify:
build: "cargo build --release"
binary: "target/release/E2EE-Experiment"
static_build: "rustup target add x86_64-unknown-linux-musl && cargo build --release --target x86_64-unknown-linux-musl"
smoke_test: |
two local instances: `listen 127.0.0.1:17777 -n A`, then `dial 127.0.0.1:17777 -n B`
expect: each side prefixes received lines with the peer name; "/quit" → peer prints its 挂断了 goodbye and exits

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
耳朵:      inbound tcp accept loop (listen)
嘴:        outbound tcp connect (dial)
见面:      noise nn handshake
密谈:      noise transport phase
挂断:      session teardown
道别:      empty-plaintext goodbye frame
转身即忘:  zero persistence
自报家门:  unsigned self-reported name frame
无名氏:    default name when -n omitted
初次见面:  no key continuity — every connection starts from scratch
