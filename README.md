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
| A language only the two of you speak | Noise NN end-to-end encryption                                 |
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

**Does not protect against**:

- **An active man-in-the-middle** can silently relay the handshake and read
  everything. This is the accepted cost of the telephone model: encryption is not
  authentication. Your mitigation is human — talk about something only the real
  person would know.
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
immediately.

Chat goes to stdout, connection events to stderr — so the conversation itself
stays pipeable.

## A library in a CLI's clothing

The core is a Rust library (crate `e2ee`); this binary is merely its first
consumer. The library never prints and never reads stdin: actions go in as
metaphor-verb methods — `born` / `dial` / `speak` / `talk_to` / `bye` /
`leave` — and everything the person experiences flows out as an event stream
(`Met`, `Heard`, `Left`, ...). The same person can live inside any program.

Embedding path: Rust programs depend on this crate directly; once the verbs
stabilize, the same API will be exposed as a C ABI (`.dll` / `.so`) for
Windows and Linux, callable from any language that speaks C.

## Wire protocol

- **Transport**: TCP
- **Framing**: every message (handshake or data) is `u16 big-endian length + payload`
- **Handshake**: Noise `NN` (`Noise_NN_25519_ChaChaPoly_BLAKE2s`), the side that
  `/dial`s is the initiator
  - `-> e` · `<- e, ee`
- **First data frame, both directions**: the self-reported name (UTF-8, unsigned,
  purely cosmetic)
- **Chat frames**: one UTF-8 line each, encrypted as Noise transport messages
- **Hang-up**: an empty plaintext frame (a bare AEAD tag on the wire), or simply
  closing the connection
- **Side channel — LAN discovery (UDP)**: `/shout` broadcasts a probe to port
  37777; every person listening answers with `name + dial address`. Carries
  presence only — never conversation, never secrets.

## The gathering — group chat (designed, not yet built)

People also sit in circles. The design below is settled; implementation deliberately
waits until the 1:1 crypto core is hardened and tested.

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
  needs a link only its owner has.

## Finding each other — discovery & pairing (partly built)

You already know everyone is *in* — listening from birth. The question is where
to find them. Zero persistence forces a clean split of labor: **the system
forgets, humans remember.** The address book lives outside the system — on your
paper, in your memory. Same shape as "recognizing the person is your job":
*remembering where they live is your job too.*

- **Same-room shout — `/shout` (built).** A UDP "anyone there?" broadcast on
  the LAN; everyone listening answers with a name and a dial address. An active
  query, not a standing beacon — no answer, no one home.
- **A card — `/card` (built).** Prints your name and `ip:port` for every
  interface. Copy it down, read it over a phone, hand it over. Works anywhere
  the address is reachable.
- **The teahouse — `--courier` (designed).** For two people behind two NAT
  walls. The same binary in its other job, deployed on any machine with a
  public IP: it assigns room numbers and splices two outbound lines together.
  Not a privileged server — no identity, no storage, no plaintext; anyone can
  open one, and no teahouse is nobler than another. Both sides dial out, and a
  NAT never blocks leaving, so this route always works. The teahouse sees who,
  when, and how many bytes — never a word.

**Room number + secret (designed).** The room number is public — teahouse
assigned, unique, prevents cross-talk. The secret is private — user-chosen,
carried out-of-band. The system never generates, stores, or mandates secrets;
their strength is a human responsibility, exactly like recognizing a voice. The
two are always separate things.

**PAKE — proving the secret without showing it (designed).** SPAKE2 lets two
holders of the same secret authenticate mathematically while the secret never
crosses the wire in any derivable form. A wrong secret fails cleanly; guessing
can only happen online, where the teahouse can rate-limit it; offline
brute-force does not exist.

**The secret only ignites (designed).** A successful PAKE yields a key with
exactly one duty: escort the Noise handshake (PSK mode). Every conversation
key is a fresh per-session ephemeral — the secret never talks. A weak secret
only hurts that one handshake instant, and everything burns at session end.

**Hybrid post-quantum handshakes (designed).** A passive recorder can store
today's traffic and wait for a quantum computer to break X25519 — "harvest
now, decrypt later". In this design the single handshake IS the whole crypto
moment (no ratchets), and worse: the secret chain passes each next secret
through the previous session, so a broken handshake eventually leaks *the next
secret* — the recorder turns from an ear into a mouth, able to impersonate.
Therefore the handshake goes hybrid: X25519 **and** ML-KEM-768 together, and
the attacker must break **both** to win. Never pure-PQ replacement — new math
is young (SIKE fell to a laptop in 2022), the hybrid keeps the 40-year-old
lock as a floor. Costs ~2 KB per handshake. PAKE itself stays classical: its
recording only ever leaks an already-consumed secret.

**The secret chain (designed).** At farewell, the next secret may be reserved
inside the already-authenticated channel — offer plus acknowledgment, and an
unacknowledged reservation is dropped: failure loses convenience, never
security. The chain lives only in RAM; its lifespan is the intersection of both
process lifetimes, and a restart returns you to the one out-of-band reading.
Rolling secrets are never written to disk — a persisted secret is a long-term
credential, which is the "face" this project deleted on purpose.

**A room seats two (designed).** The teahouse never hosts a group. A gathering
weaves its own mesh — one two-person room per link, with the introducer
brokering room number and secret to both sides over existing encrypted links.
Pairwise Noise per link makes relayed frames unforgeable and uncorrelatable
across rooms, and the group-key schedule above lives one floor above the
wiring — unchanged.

## Where this could go

| Idea                            | Metaphor                           | Constraint it must respect                |
|---------------------------------|------------------------------------|-------------------------------------------|
| UDP hole punching               | An introduction by a mutual friend | Must never touch plaintext                |
| The teahouse (`--courier`)      | A courier who only introduces      | Design in "Finding each other"            |
| IPv6-first direct dials         | Two people in the same room        | Already how it works                      |
| C-ABI bindings (`.dll` / `.so`) | The same verbs in any language     | No hidden state, no widened trust surface |

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
