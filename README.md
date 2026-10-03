# E2EE-Experiment
[简体中文](README_ZH.md) | English | [AI collaboration doc](README_AGENT.md)

> One binary. It listens like a person, speaks like a person, and forgets like a person.

**E2EE-Experiment** takes a single metaphor literally: *the executable is a person.*
No server, no accounts, no database — just a program that can hold up its end of a
private conversation, then remember nothing.

> **Using an AI assistant (Copilot / Claude / GPT ...) to develop this project?**
> Read [README_AGENT.md](README_AGENT.md) first — it is written specifically for AI
> and carries the project's design invariants and hard red lines. Skipping it and
> editing code directly is very likely to break the zero-persistence core.

## The model

| A person                             | This program                                                   |
|--------------------------------------|----------------------------------------------------------------|
| Ears                                 | open from birth — listening from the moment the process starts |
| A mouth                              | `/dial` — reach out to anyone, anytime, no restart needed      |
| A language only the two of you speak | Noise hybrid E2EE (X25519 + Kyber1024)                         |
| Turning away and forgetting          | Zero persistence — nothing is ever written to disk             |
| Recognizing who you're talking to    | **Your job**, after decryption                                 |

The last row is deliberate. This is the *telephone model* of security: the wire is
private, but the person on the other end introduces themselves however they like.
The system guarantees **secrecy**, never **identity**. Recognizing a voice — through
shared secrets, in-jokes, a familiar tone — is a human act, and it stays human.

## Design invariants

The soul of the project. No change may violate these.

1. **One binary.** `cargo build --release` produces a single executable. Build for
   `x86_64-unknown-linux-musl` to get a fully static file; copy it to any machine
   and it runs.
2. **Zero persistence.** No config, no logs, no keys, no history. The process is the
   lifespan — when it exits, everything it ever knew is gone. Messages live in
   memory only, like spoken words.
3. **No identity layer.** No accounts, no fingerprints to compare, no
   trust-on-first-use. Every connection is a first meeting; the name is a
   self-reported alias with nothing behind it.
4. **Symmetry.** Every copy of the binary is equal. There is no server/client split
   and no privileged node — which means there is no node worth subpoenaing.
5. **Ephemerality.** Every connection performs a fresh Noise NN handshake with
   brand-new ephemeral keys. Forward secrecy comes for free: when the conversation
   ends, the keys die with it.
6. **Attribution by topology.** The name on your screen is what the receiving
   link says it is — never a self-claim inside the plaintext. Crypto provides
   secrecy; topology provides attribution.

## Honest threat model

**Protects against**: anyone on the wire (ISP, Wi-Fi snooper, backbone tap) —
they see Noise-encrypted frames and nothing else.

**Does not protect against (by default)**:

- **An active man-in-the-middle** can silently relay the handshake and read
  everything. This is the accepted cost of the telephone model. **Optional
  mitigation now built in**: agree on a secret out-of-band and use
  `/dial ADDR secret` (or `/await secret` before someone arrives) — a PAKE
  (SPAKE2) proves both sides hold the same secret without ever transmitting
  it; a wrong secret fails loudly.
- **The other end.** They can copy, paste, screenshot, and remember. E2EE binds
  the channel, not the person.

One process is one person for its whole life: born with ears open, free to walk
up to anyone at any moment (`/dial`), and able to hold several independent 1:1
conversations at once — `/talk` moves your attention between them. Those parallel
pairwise links are exactly the skeleton the gathering will later be built on.

## Quick start

```bash
cargo build --release

# Terminal 1 — Alice is born, listening on 7777
./target/release/E2EE-Experiment -n Alice

# Terminal 2 — Bob is born on his own port, then walks over to Alice
./target/release/E2EE-Experiment -n Bob 127.0.0.1:7778
/dial 127.0.0.1:7777
```

Type a line, press enter, and it appears on the other side (encrypted in transit).
In-session: `/dial` to start a conversation, `/talk` to switch attention among
several, `/list` to see who's around, `/card` to print your dial addresses,
`/shout` to scan the LAN, `/bye` to say goodbye to the focused one,
`/quit` or Ctrl-D to bid everyone farewell and leave; Ctrl-C walks away
immediately. Born with `--hidden`, a person never answers `/shout` probes at all —
reachable only by exact address or rendezvous.

Chat goes to stdout, connection events to stderr — so the conversation itself
stays pipeable.

## A library in a CLI's clothing

The core is a Rust library (crate `e2ee`); this binary is merely its first
consumer. The library never prints and never reads stdin: actions go in as
metaphor-verb methods — `born` / `born_hidden` / `dial` / `speak` / `talk_to` /
`bye` / `leave` — and everything the person experiences flows out as an event
stream (`Met`, `Heard`, `Left`, ...). The same person can live inside any program.

Embedding paths, both live: Rust programs depend on this crate directly; any
language that speaks C links `libe2ee.so` / `e2ee.dll` — workspace member
`ffi/`, header `include/e2ee.h`. The ABI is deliberately tiny (12 functions:
create/destroy, hidden-create, dial, await-secret, speak, bye, poll, ...),
per-handle runtime, add-only frozen. `ffi/ffi_test.c` is the executable
contract; Python speaks it too via `ffi/ctypes_test.py`.

## Testing & platforms

`cargo test` runs 27 tests: real persons on ephemeral localhost
ports, asserting only on the public event stream — bidirectional talk, clean
farewell, focus never stolen by arrivals, teahouse pairing and room isolation,
PAKE success/failure, secret-chain renewal, three-person circle chat, a
punched (UDP hole-punched) direct conversation, an armed secret surviving
port scans, the teahouse shrugging off silent connections, teahouse lines
not chaining, a hidden person staying reachable by exact address, an
open desk (`--no-chain`) never locking strangers out, a stable guard
(`--guard`) shrugging off endless wrong-key dials, lossless binary file
transfer (over direct AND punched lines, with the 32 MB cap enforced), and
padded chat lines round-tripping byte-exact — plus unit tests of the group
key schedule, the hybrid PQ suites, and chat-frame parsing (both layouts). The C ABI has
its own contract test (`ffi/ffi_test.c` — full lifecycle including PAKE,
secret chain, hidden mode, and error paths) and a Python ctypes round-trip.
Linux is fully tested here, including a fully static musl build (no runtime
deps, Kyber1024 inside); the FFI gate cross-compiles to `e2ee.dll` (all 12
symbols exported).

## Wire protocol

- **Transport**: TCP (or a UDP hole-punched line — same framing, one frame per
  datagram)
- **Framing**: every message (handshake or data) is `u16 big-endian length + payload`
- **PAKE, when a secret is in play (first)**: both sides exchange one SPAKE2
  mask each (~65 B frames); the secret itself never crosses the wire in any
  derivable form
- **Handshake**: Noise hybrid — `Noise_NNhfs_25519+Kyber1024_ChaChaPoly_BLAKE2s`
  without a secret, `Noise_NNpsk0+hfs_25519+Kyber1024_ChaChaPoly_BLAKE2s` with
  one (the PAKE output is the PSK). X25519 and Kyber1024 run together in every
  handshake — an attacker must break both. Each handshake message is ~1.6 KB
  plus 0–255 bytes of random padding (measured live: every handshake lands on
  a different size) — the padding blurs the exact-size fingerprint and is
  ignored by peers, old and new. The side that `/dial`s is the initiator; the
  teahouse/introducer assigns roles on their paths (later arrival initiates)
- **First data frame, both directions**: the self-reported name — encrypted
  like everything else (kind `0x05`), UTF-8, unsigned, purely cosmetic
- **Chat frames**: every encrypted payload starts with a kind byte —
  `0x01` next-secret offer · `0x02` offer ack ·
  `0x03` gathering contribution · `0x04` group ciphertext · `0x05` name ·
  `0x06` file head (name ‖ total size) · `0x07` file chunk (seq ‖ ≤32KB data) ·
  `0x08` chat line (text length u16 ‖ text ‖ 0–255 random bytes — what the
  CLI sends; a frame's size no longer hugs the text's length, measured: the
  same sentence eight times lands on eight different sizes; receivers drop
  the pad; no negotiation). `0x00` is the legacy bare-text chat line, still
  understood for old peers. Peers predating `0x08` silently drop padded
  lines — upgrade both ends together.
  Unknown kinds are skipped silently (forward compatibility)
- **Hang-up**: an empty plaintext frame (a bare AEAD tag on the wire), or simply
  closing the connection
- **Timeouts**: every meeting stage (PAKE, handshake, name exchange) is bounded
  at 10 s — a connection that says nothing is dropped, never held
- **Side channel — LAN discovery (UDP)**: `/shout` broadcasts a probe to port
  37777; every person listening answers with `name + dial address`. Carries
  presence only — never conversation, never secrets.

## Handing things over — files (v1 built)

Words are not all two people exchange. `/file <path>` hands a file to the
focused conversation: split into 32 KB chunks, each sealed as its own Noise
frame, reassembled at the far end — **bytes never touch the text lane**, so
binary arrives byte-for-byte (no UTF-8 mangling). Chunk size is chosen so a
chunk plus framing fits a UDP datagram: files traverse direct dials, teahouse
lines AND punched lines alike. Chunk boundaries are randomized within
8–32 KB, so the fixed-32KB shape doesn't show (the receiver is size-agnostic —
no negotiation needed); the total-volume shape is the application's business
(pad your container format if it matters). A 32 MB cap (send and receive alike) bounds the
in-RAM reassembly — the library never touches the disk; bytes surface as a
`FileArrived` event and the consumer decides where they land (the CLI saves
into `./received/`, with sanitized, collision-proofed names). One file in
flight per conversation at a time; a peer's oversized offer is declined
loudly, never buffered; a chunk that arrives out of order voids the transfer.

## The gathering — group chat (v1 built)

People also sit in circles. Everyone dials everyone (full mesh), then `/circle`
runs the contribute-and-derive ritual; `/gsay` speaks with your own subkey,
one ciphertext fanned out to every link. New joiner? Re-run `/circle` — that
*is* the rekey rule. The name on your screen always comes from the receiving
link; crypto provides secrecy, topology provides attribution.

- **Topology — a circle, not a stage.** Every member connects directly to every
  other (full mesh). Any scheme that saves links — a ring, a "moderator" — must
  decrypt and re-tell, and thus becomes a listener. A circle of N needs N² links,
  which caps a gathering at roughly 8 people. Not a defect: five hundred people in
  a circle is a lecture, and lectures need a stage — a server — a different project.
- **Joining — by introduction.** The newcomer dials any member; the introducer
  hands over the address list and nothing else — never relays a word of the
  conversation. The newcomer then handshakes independently with everyone.
- **Membership — presence.** A member is a live connection; no roster is stored
  anywhere. Leave and you are out; the gathering dies with its last member.
- **The room language — one group key.** Each member contributes a fresh 32-byte
  random value, broadcasts it over their Noise links, and everyone derives
  `K = HKDF(byte-sorted concatenation of all contributions)` — no one dictates the
  key, no one can predict it. Pairwise Noise shrinks to a single duty: wiring the
  mesh and escorting this negotiation.
- **Speaking — encrypt once, send everywhere.** Each sender derives a personal
  subkey `s_i = HKDF(K, r_i)` — their own contribution doubles as their sender
  label, and per-sender subkeys prevent nonce collisions under the shared key.
  One ciphertext, fanned out on every link: join the group, and your message goes
  to everyone at once.
- **Re-keying — on arrival, never on departure.** A leaver holds no live link to
  listen with, zero persistence leaves nothing to decrypt later, and the next join
  re-keys anyway. Eviction means the remaining members start a fresh gathering —
  which is the join mechanism itself; nothing new has to be invented.
- **Attribution — the link is the signature.** Every frame still leaves on the
  sender's own links, so no member can put words in another's mouth. Hard rule:
  the displayed name always comes from the receiving link, never from a claim
  inside the plaintext. Crypto provides secrecy; topology provides attribution.
- **Honest limitation.** Contributions are broadcast, so any member can derive
  others' subkeys — but that grants no impersonation ability, because injection
  needs a link only its owner has. If members re-`/circle` at cross purposes,
  some will hold mismatched group keys; an undecipherable group frame surfaces
  as a "mumble" notice (not silence) — re-run `/circle` to converge.

## Finding each other — discovery & pairing (built)

You already know everyone is *in* — listening from birth. The question is where
to find them. Zero persistence forces a clean split of labor: **the system
forgets, humans remember.** The address book lives outside the system — on your
paper, in your memory. Same shape as "recognizing the person is your job":
*remembering where they live is your job too.*

- **Same-room shout — `/shout` (built).** A UDP "anyone there?" broadcast on
  the LAN; everyone listening answers with a name and a dial address. An active
  query, not a standing beacon — no answer, no one home.
- **Hidden — `--hidden` (built).** Some people answer no shouts. Born with
  `--hidden`, a person never replies to LAN probes: broadcast cannot detect
  their existence — the shout-reflex ear simply isn't there. Nothing else
  changes: the TCP ear stays open, so anyone holding the exact address (from a
  card) can still dial, and teahouse/introducer rendezvous work as usual.
  Hiding is one-way: a hidden person may still `/shout` to find others — but
  the probe itself announces the prober's IP to everyone listening, so a truly
  hidden person stays quiet.
- **A card — `/card` (built).** Prints your name and `ip:port` for every
  interface. Copy it down, read it over a phone, hand it over. Works anywhere
  the address is reachable.
- **The teahouse — `--courier` (built).** For two people behind two NAT
  walls. The same binary in its other job, deployed on any machine with a
  public IP: guests pick their own room numbers (agreed out-of-band, the same
  channel as the secret), and the teahouse simply wires same-number arrivals
  together. Not a privileged server — no identity, no storage, no plaintext;
  anyone can open one, and no teahouse is nobler than another. Both sides dial
  out, and a NAT never blocks leaving, so this route always works. The teahouse
  sees who, when, and how many bytes — never a word.
- **The introducer — `/punch` (built).** The keeper's side job: the same
  port, UDP side. Two people who both want a *direct* line register the same
  tag; the introducer tells each the other's public address and steps aside.
  Then both knock on each other's doors at the same time — each NAT, believing
  its own person left first, holds the door open. The Noise handshake runs
  over that punched line; the introducer never touches a byte of it. Best
  effort (the ~1.6 KB handshake messages travel as IP fragments) — if the wall
  is too strict, you simply re-meet via `/meet` through the teahouse; nothing
  falls back automatically.

**Room number + secret (built).** The room number is public — user-chosen
out-of-band; pick it unguessable, exactly like a secret, or two pairs may
collide and get wired to each other. The secret is private — also user-chosen,
also carried out-of-band. The system never mandates or stores your first
secret; its strength is a human responsibility, exactly like recognizing a
voice. (Rolling secrets in the chain are the exception that proves the rule:
the system generates them, but only inside an already-authenticated channel,
and they never touch disk.) The two are always separate things.

**PAKE — proving the secret without showing it (built).** SPAKE2 lets two
holders of the same secret authenticate mathematically while the secret never
crosses the wire in any derivable form. A wrong secret fails cleanly; guessing
can only happen online, and the armed challenge is one-shot — one guess per
arming, then it's spent (a connection that never reaches the PAKE exchange
doesn't spend it, so port scans can't burn your arrangement); offline
brute-force does not exist.

**Stable guard — `--guard` (built).** A receiver that must authenticate every
caller arms a MACHINE key instead: every arrival must pass PAKE, and the
arming is **never consumed** — wrong-key dials fail loudly, as many as an
attacker cares to make, without ever locking the desk. (One-shot `/await`
burns precisely to protect low-entropy human secrets from online guessing; a
32-byte machine key needs no such protection — guessing is infeasible, so
endless re-arming costs nothing.) Pair with `--no-chain` on open desks so
farewells leave no lock either: `--guard KEY --no-chain` is the armed-desk
shape.

**The secret only ignites (built).** A successful PAKE yields a key with
exactly one duty: escort the Noise handshake (PSK mode). Every conversation
key is a fresh per-session ephemeral — the secret never talks. A weak secret
only hurts that one handshake instant, and everything burns at session end.

**Hybrid post-quantum handshakes (built).** A passive recorder can store
today's traffic and wait for a quantum computer to break X25519 — "harvest
now, decrypt later". In this design the single handshake IS the whole crypto
moment (no ratchets), and worse: the secret chain passes each next secret
through the previous session, so a broken handshake eventually leaks *the next
secret* — the recorder turns from an ear into a mouth, able to impersonate.
Therefore the handshake goes hybrid: X25519 **and** Kyber1024 together, and
the attacker must break **both** to win. Never pure-PQ replacement — new math
is young (SIKE fell to a laptop in 2022), the hybrid keeps the 40-year-old
lock as a floor. Costs ≈3.2 KB per handshake (two ~1.6 KB frames). PAKE itself
stays classical: its recording only ever leaks an already-consumed secret.

**The secret chain (built).** At farewell, the next secret may be reserved
inside the already-authenticated channel — offer plus acknowledgment, and an
unacknowledged reservation is dropped: failure loses convenience, never
security. The chain lives only in RAM; its lifespan is the intersection of both
process lifetimes, and a restart returns you to the one out-of-band reading.
The chain anchors to an address: it works on direct dials, may outlive the NAT
mapping on punched lines (the reservation simply sits unused), and teahouse
lines don't chain at all — rooms are one-shot, there is no anchor to hang a
chain on. A receiver that stays open to strangers — a feedback line, say —
opts out entirely with `--no-chain`: no reservations sent, none accepted;
anyone can walk in, and every farewell leaves no lock. Rolling secrets are
never written to disk — a persisted secret is a
long-term credential, which is the "face" this project deleted on purpose.

**A room seats two (built).** The teahouse never hosts a group. A gathering
weaves its own mesh — one two-person room per link, with the introducer
brokering room number and secret to both sides over existing encrypted links.
Pairwise Noise per link makes relayed frames unforgeable and uncorrelatable
across rooms, and the group-key schedule above lives one floor above the
wiring — unchanged.

## Known limitations

Honesty first — v1 has these sharp edges:

- **No liveness probes.** A peer that vanishes without closing (or over a
  punched UDP line, which has no EOF at all) leaves the line and its `/list`
  entry hanging until you `/quit`. Meeting stages are all time-bounded, but
  established conversations are not.
- **The introducer's referral is unauthenticated.** Anyone who knows the tag
  can register as your "peer" — the tag is a rendezvous, not an identity.
  `meet_via` only accepts referrals from the introducer itself, but the
  introducer trusts whoever shows up with the tag. Bring a secret if you need
  to know who's on the other end.
- **The teahouse pairs by room number alone.** Two pairs that pick the same
  number get cross-wired; pick unguessable room numbers.
- **Group membership is consistency-by-convention.** The full mesh is manual,
  and simultaneous re-`/circle` runs can leave members with different group
  keys (surfaced as mumble notices, fixed by re-circling). There is no
  membership list to arbitrate — on purpose.
- **One gathering at a time, per person.** `/circle` covers all live
  conversations; you cannot sit in two circles with the same process.
- **Hiding is not invisibility.** `--hidden` silences the shout-reflex only;
  your TCP listen port still answers dials, and a port scanner can still find
  the open port (though nothing tells it what it is). True unreachability
  means `/await secret` as well.
- **Cross-version chat is one-way.** The padded chat frame (`0x08`) is
  invisible to peers running pre-0x08 builds (they skip unknown kinds
  silently — no garbage, just no line). Old→new still reads fine (`0x00`
  stays understood). Upgrade both ends together; group-chat frames are not
  padded.

## Where this could go

| Idea                       | Metaphor                           | Constraint it must respect           |
|----------------------------|------------------------------------|--------------------------------------|
| IPv6-first direct dials    | Two people in the same room        | Already how it works                 |
| Growing the C-ABI surface  | Teaching the verbs more languages  | Add-only; frozen signatures forever  |

Anything that stores messages, verifies identity, or introduces a privileged node
breaks the model — it belongs in a different project.

## 🚫 Non-Commercial Statement

This project is initiated by the developer out of personal interest and for technical research purposes, and is **non-commercial** in nature:

- **Permanently Free**:
  This project is completely free, with **no paid features, memberships, subscriptions, or in-app purchases**. All features are fully accessible to all users.
- **No Sponsorship Channels**:
  The author has **never opened any sponsorship channels**, nor does the author **accept any financial donations** — to maintain the project's neutrality and purity.
- **Non-Profit Purpose**:
  This project involves no commercial operations, and the author derives no direct or indirect financial benefit from it.
- **Research-Oriented**:
  This project is consistently positioned for **cryptographic protocol research and educational exchange** — providing a research tool for the community, not a commercial product. Any commercial use of this project is the user's own initiative and is unrelated to this project.
- **Resale Prohibited**:
  Resale, redistribution for profit, or commercial use of this project is strictly prohibited. Please obtain it only from this repository (GitHub) or other officially designated channels. The developer assumes no responsibility for any issues arising from unofficial sources.

## ⚖️ Disclaimer

- **Purpose Limitation**:
  This project is intended for **cryptographic protocol research, software testing, and educational purposes** only.
  Do not use this project for any illegal purposes (including but not limited to unauthorized interception, harassment, or transmitting unlawful content).
- **Consequences Warning**:
  The use of end-to-end encrypted communication tools **may be regulated by the laws of your jurisdiction**.
  This system provides secrecy, not identity verification (the telephone model) — an active man-in-the-middle can relay conversations, and recognizing the other end is the user's own responsibility.
  Assess the risks before using. **The developers and contributors assume no responsibility for any account bans, legal liability, or other consequences arising from use.**
- **Experimental Status**:
  This is experimental research code. The cryptographic primitives come from mature libraries (Noise via `snow`), but the protocol composition has **not undergone a third-party security audit**. Do not use it for genuinely sensitive communications.
- **No Warranty**:
  This software is provided under its license terms, **without any express or implied warranty**, including but not limited to merchantability, fitness for a particular purpose, and non-infringement.
- **Limitation of Liability**:
  To the maximum extent permitted by applicable law, **in no event shall the authors or contributors be liable for any direct, indirect, incidental, special, or consequential damages arising from, or in any way related to, the use or inability to use this software**, whether advised of the possibility.
- **User Responsibility**:
  Users bear all legal responsibility arising from their use of this project.
- **Final Interpretation**:
  The author of this project reserves the right of final interpretation of this disclaimer.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
