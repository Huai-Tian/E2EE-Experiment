# E2EE-Experiment
[简体中文](README_ZH.md) | English | [AI collaboration doc](README_AGENT.md)

> One binary. It listens like a person, speaks like a person, and forgets like
> a person. It can also pull up a table — every rule of group chat is
> translated from the rules of a physical conversation.

**E2EE-Experiment** takes a single metaphor literally: *the executable is a person.*
No server, no accounts, no database — just a program that can hold up its end of a
private conversation, then remember nothing.

> **Using an AI assistant (Copilot / Claude / GPT ...) to develop this project?**
> Read [README_AGENT.md](README_AGENT.md) first — it is written specifically for AI
> and carries the project's design invariants and hard red lines. Skipping it and
> editing code directly is very likely to break the zero-persistence core.

## The idea

| A person                              | This program                                                   |
|---------------------------------------|----------------------------------------------------------------|
| Ears                                  | open from birth — listening from the moment the process starts |
| A mouth                               | `/dial` — reach out to anyone, anytime, no restart needed      |
| A language only the two of you speak  | end-to-end encryption, fresh keys every conversation           |
| A table                               | `/circle` — the roster is the table; sit down and it's a circle |
| Turning away and forgetting           | zero persistence — nothing is ever written to disk             |
| Recognizing who you're talking to     | **your job**, after decryption                                 |

The last row is deliberate. This is the *telephone model* of security: the wire is
private, but the person on the other end introduces themselves however they like.
The system guarantees **secrecy**, never **identity**. Recognizing a voice —
through shared secrets, in-jokes, comparing a short word — is a human act, and it
stays human. (To do that step properly, see
[Secrets & SAS](#secrets--sas-knowing-who-is-on-the-other-end).)

## How it works

Three properties carry all the weight:

1. **Every conversation is a fresh secret.** Each connection performs a new
   key exchange with brand-new keys — an attacker must break **both** a
   classical algorithm (X25519) **and** a post-quantum one (Kyber1024) to read
   anything, so even a recorder who stores today's traffic and waits for a
   quantum computer gets nothing. When the conversation ends, the keys die
   with it: forward secrecy for free.
2. **Nothing is ever stored.** No config, no logs, no keys, no history. The
   process is the lifespan — when it exits, everything it ever knew is gone.
3. **Everyone is equal.** Every copy of the binary is the same; there is no
   server/client split and no privileged node. The optional "teahouse" used
   for NAT traversal only shuffles bytes it cannot read.

**Honest threat model.** *Protects against:* anyone on the wire (ISP, Wi-Fi
snooper, backbone tap) — they see encrypted frames and nothing else.
*Does not protect against (by default):* an active man-in-the-middle who
relays the handshake (the telephone model's accepted cost — a shared secret
blocks it and the SAS word exposes it, see
[Secrets & SAS](#secrets--sas-knowing-who-is-on-the-other-end)), and the
other end itself (they can copy, paste, screenshot).

### Key details worth knowing

- **Traffic shape obfuscation** — handshake and chat frames carry random
  padding, file chunks are cut at randomized sizes: the same sentence sent
  eight times produces eight different frame lengths. A passive observer
  cannot fingerprint message sizes.
- **Files never touch the text lane** — binaries arrive byte-for-byte, no
  text mangling, up to 32 MB per file.
- **No one speaks in another's name at the table** — every member brings a
  one-time voice key: circle speech, farewells, introductions and testimony
  are all signed; each roster row is self-signed by its owner.
- **Every departure or arrival re-keys (a change of breath)** — the leaver
  can no longer read new speech, the newcomer cannot unlock old history:
  the key boundary is the table boundary. There is no roster to leak
  because none is ever stored.
- **The name on your screen comes from the wire, not the plaintext**.
- **Close it and it's forgotten — that IS the coercion defense** — nothing
  is ever written to disk, keys and plaintext are wiped from memory the
  moment they're done, and every conversation's keys are ephemeral: after a
  clean exit, searching the whole machine finds nothing. There is no
  "duress mode", because there is no history to hide. Honest edges: a hard
  kill (`kill -9` / power cut) leaves a seconds-scale RAM residue that
  fades on reboot, and a seized *running* device of course reads what is
  being said — true of any live conversation; closing ends it.

## Getting started

Get the binary either way:

- **Prebuilt** — from [GitHub Releases](https://github.com/Huai-Tian/E2EE-Experiment/releases):
  fully static Linux x86_64 (musl, zero runtime deps), Linux glibc, and Windows.
- **From source** — `cargo build --release` produces `./target/release/E2EE-Experiment`.

Your first conversation, two terminals on one machine:

```bash
# Terminal 1 — Alice is born, listening on 7777
./target/release/E2EE-Experiment -n Alice

# Terminal 2 — Bob is born on his own port, then walks over to Alice
./target/release/E2EE-Experiment -n Bob 127.0.0.1:7778
/dial 127.0.0.1:7777
```

Type a line, press enter, and it appears on the other side — encrypted in
transit. Chat goes to stdout, connection events to stderr, so the conversation
itself stays pipeable. Received files land in `./received/`.

### Startup options

```
E2EE-Experiment [listen-addr] [-n name] [flags]
```

| Flag            | Meaning                                                                                                                                                                               |
|-----------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| *(none)*        | Born listening on `0.0.0.0:7777`. Two people on one machine need different ports.                                                                                                     |
| `-n <name>`     | The name you introduce yourself as (self-reported, nothing behind it).                                                                                                                |
| `--no-chain`    | Don't reserve next-time secrets at farewell, don't accept others' — the right shape for a desk open to strangers.                                                                     |
| `--guard <KEY>` | Stable door lock: every caller must pass the same machine key (32-byte random), never consumed — wrong-key floods can't lock the desk. Pair with `--no-chain` for an armed open desk. |
| `--courier`     | Open a teahouse (see [Behind NAT](#two-people-behind-nat)) — the same binary in its other job.                                                                                        |

### Command reference

| Command                                   | What it does                                                                                           |
|-------------------------------------------|--------------------------------------------------------------------------------------------------------|
| `/dial <addr> [secret]`                   | Reach out to someone; with a secret, both sides must match.                                            |
| `/await <secret>`                         | Arm a one-shot secret: the next arrival must match (verified then burned — port scans can't waste it). |
| `/meet <teahouse-addr> <room> [secret]`   | Meet a pre-agreed person through a teahouse (both pick the same room number).                          |
| `/punch <introducer-addr> [tag] [secret]` | Attempt a *direct* line through a UDP hole punch. Omit the tag and a fresh 128-bit one is generated and shown (Crockford base32 — relay it out-of-band); a short explicit tag draws a loud warning but proceeds (a secret still compensates via PAKE). |
| `/file <path>`                            | Hand a file (≤32 MB) to the focused conversation.                                                      |
| `/talk <id\|name>`                        | Switch attention among parallel conversations.                                                         |
| `/list`                                   | Who's currently connected to you.                                                                      |
| `/card`                                   | Print your name and dial address for every network interface.                                          |
| `/circle [closed\|open] [name1 name2 …]`  | Gather a circle from everyone currently connected; name specific lines instead and only those are invited (a selective circle — old tables stay seated). Re-run inside a circle = roll call (the repair path). Multi-person defaults to a closed table, solo to an open stage. |
| `/gsay <text>`                            | Speak in the circle — signed, encrypted once, everyone receives.                                       |
| `/bring [expected-name]`                  | Introduce someone to the table: hand a seat skeleton to the focused conversation. With a name, an expectation gate opens. |
| `/sit [name]`                             | Take a seat in answer to an offer (the seat signature must be triggered by the person themselves).     |
| `/word`                                   | Compare words: show the current channel's short word (conversation / circle).                          |
| `/bye`                                    | Say goodbye to the focused conversation (a signed circle farewell first, if seated).                   |
| `/quit` or Ctrl-D                         | Say goodbye to everyone and leave. Ctrl-C walks away immediately.                                      |

### Group chat: pulling up a table

1. **Gathering** — anyone runs `/circle` on the people **currently connected
   to them**: a signed invitation roster (what *she* has personally seen, not
   anyone's authoritative register). Each name on the roster brings a
   self-signed random contribution; when **all names have arrived**, the
   group key is cast: nobody dictates it, nobody can predict it. Full mesh
   not required — members just need to be connected to each other (stars
   and chains both welcome); speech travels along the roster.
2. **Speech** — `/gsay <text>`: signed with a one-time voice key, encrypted
   once under the group key, relayed hop by hop along member lines.
   Multi-person circles default to a **closed table** (introductions need
   the table's blessing); a circle of one defaults to an **open stage**.
   Override with `/circle closed|open`.
3. **Comparing words** — `/word`: inside the circle, a group short word
   (12 digits in 4 groups, identical for everyone). Want to confirm nobody
   has tampered with this table? Say it out loud to each other.
4. **Taking a seat** — a seated member `/bring`s (introduces), the candidate
   `/sit`s (self-signed seating): offer → acceptance → rekey → seating
   chart, four steps and the newcomer is in — she can read everything said
   from now on, and cannot unlock anything said before she arrived.
5. **Leaving** — `/bye` is a *signed* farewell: the neighbor holding her
   direct line witnesses and the table rekeys — the leaver can no longer
   read new speech. Vanishing (power cut, unplugged cable) works the same:
   the holder of the dead line testifies. A rekey seals only the future;
   it never pretends to unsay what was said.
6. **Repairing the table** — any inconsistency (two people editing the
   roster at once; undecipherable speech surfacing as a loud "mumble"
   notice) is fixed by one and the same gesture: re-run `/circle` — a
   **roll call**. Fresh circle id, everyone re-gathers, everything settles.
   The protocol never pretends to be always-consistent; it promises:
   **disagreement is always heard, repair is always available.**
7. **Several tables** — one person may sit at several tables at once:
   `/circle name1 name2 …` invites only those lines and keeps every old
   table seated; the new table becomes the current seat. Each table has
   its own key and its own word; one table cannot hear another. Two
   people sharing a single line may belong to different tables. Only a
   no-argument roll call puts the old table down; bare verbs (`/gsay`,
   `/word`, `/bring`) act on the current seat.

### Two people behind NAT

Both of you can dial *out* — a NAT never blocks leaving. The problem is only
who receives. Two answers, both built in:

- **The teahouse** — run the same binary as `--courier` on any machine with a
  public IP. It is not a privileged server: no identity, no storage, no
  plaintext. Two guests who agree on a room number out-of-band get wired
  together. It sees who, when, and how many bytes — never a word.
  ```bash
  # on the public machine
  ./E2EE-Experiment --courier 0.0.0.0:8888
  # both sides, from behind their NATs
  /meet <teahouse-addr> <room> [secret]
  ```
- **The introducer (hole punch)** — the teahouse's UDP side-job. Two people
  who want a *direct* line register the same tag; each learns the other's
  public address and both knock simultaneously — each NAT, believing its own
  person left first, holds the door open. Best effort; if the wall is too
  strict, re-meet via `/meet`. Nothing falls back automatically.

**Room numbers and tags are rendezvous, not secrets** — pick them unguessable
(two pairs colliding on a room number get wired to each other), and agree on
them out-of-band exactly like secrets.

### Secrets & SAS: knowing who is on the other end

The cure for the man-in-the-middle gap is two human acts — a shared **secret**
prevents it up front, the **SAS word** catches it on the spot:

- `/dial <addr> <secret>` — the handshake now includes a cryptographic proof
  that both sides hold the *same* secret. The secret itself never crosses the
  wire in any derivable form; a wrong secret fails loudly; offline guessing
  does not exist.
- `/await <secret>` — arm it on your side before someone arrives.
- **The secret chain** — at farewell, the next secret can be reserved inside
  the already-authenticated channel, so tomorrow's conversation re-authenticates
  automatically. It lives in RAM only; a restart returns you to the out-of-band
  reading. A desk open to strangers opts out with `--no-chain`.
- **The SAS word (`/word`)** — insurance even without a secret: with a focused
  1:1 it shows the **conversation word** (identical at both ends); inside a
  circle, the **group word** (identical for everyone); with both, it shows
  both. Twelve digits in four groups — the decades-proven ritual of reading
  numbers over a phone. A splicer who splits one line into two halves gets
  two keys, hence two different words — one reading aloud and the trick is
  exposed. The word is computed on the spot and never stored; not comparing
  is your freedom — it's a tool for the willing, not an obligation.
- **A weak secret only hurts that one handshake instant** — the secret never
  encrypts the conversation itself; every conversation key is fresh and
  ephemeral.

## For developers

The core is a Rust library (`e2ee`); this CLI is merely its first consumer,
and the same "person" can live inside any program — Rust via the crate, other
languages via the C ABI (`libe2ee.so` / `e2ee.dll`, header `include/e2ee.h`,
frozen add-only surface). `cargo test --features rogue` covers the full
behavior (including the attack-precedent injection matrix — see
[README_AGENT.md](README_AGENT.md)). Design invariants and red lines also
live there — read them before touching anything.

## Known limitations

- **No liveness probes.** A peer that vanishes without closing (or over a
  punched UDP line, which has no EOF) leaves the line and its `/list` entry
  hanging until you `/quit`. A zombie circle member is cleared out by the
  witness rekey of her direct neighbor.
- **The introducer's referral is unauthenticated.** Anyone who knows the tag
  can register as your "peer" — bring a secret if you need to know who's there.
- **The teahouse pairs by room number alone** — pick unguessable ones.
- **The carrier sees the table's physics.** The teahouse/network can observe a
  circle's existence, traffic shape and relay topology among members (the
  circle id is plaintext routing information) — it cannot read a single word.
  "The physical table is inherited by the teahouse" — documented honestly.
- **Group consistency is by roll call.** Disagreement never hides — an
  undecipherable line surfaces loudly as a "mumble"; any member re-runs
  `/circle` to re-gather. When the hub of a star leaves, the rest re-gather
  by roll call. One person may sit at several tables at once — bare verbs
  act on the current seat (see *Several tables* above).
- **Cross-version: chat still works, circles don't.** The size-padded chat
  frame is invisible to older builds (silently skipped, no garbage);
  old→new still reads fine; but 3.0.0's circle frame layouts are
  incompatible with 1.x (that's exactly why this is a breaking release).
  Upgrading both ends is best.

## 🚫 Non-Commercial Statement

This project is initiated by the developer out of personal interest and for
technical research purposes, and is **non-commercial** in nature.

- **Permanently free**:
  This project is completely free, with **no paid features, memberships,
  subscriptions, or in-app purchases**. All features are fully accessible to
  all users.
- **No sponsorship channels**:
  The author has **never opened any sponsorship channels**, does **not accept
  any financial donations**, and permits no third party to collect donations
  in the project's name.
- **Non-profit purpose**:
  This project involves no commercial operations; the author derives no
  direct or indirect financial benefit from it.
- **Research-oriented**:
  This project is consistently positioned for **cryptographic protocol
  research and educational exchange** — a research tool for the community,
  not a commercial product.

**License is AGPL-3.0 only — no commercial exceptions.** This project is
offered under the terms of the GNU AGPL-3.0 (see [LICENSE](LICENSE)), and
**every use must comply with that license in full**. What AGPL requires —
source disclosure for distributed derivatives, and for network services,
complete corresponding source offered to all users — is exactly what it
means to use this project. **Commercial use that cannot accept AGPL terms
does not have the author's authorization**: the author does not offer, and
will not negotiate, dual licensing, commercial exceptions, or proprietary
redistribution. Reselling builds for profit while ignoring AGPL obligations
is copyright infringement.

- **Attribution and statement integrity**:
  Redistribution of unmodified builds is permitted only together with this
  statement and proper attribution. **Removing, altering, or obscuring this
  non-commercial statement when redistributing is prohibited.**
- **Official channels only**:
  Obtain binaries and sources **only** from this repository (GitHub) or its
  official Releases. Builds from any other source are unofficial, unverified,
  and used entirely at the downloader's own risk. The developer assumes no
  responsibility for any issues — including security incidents — arising from
  unofficial sources, and **reserves the right to pursue legal remedies
  against violations of the above terms**.

## ⚠️ Disclaimer

> **This is experimental research code. It has NOT undergone any third-party
> security audit. Do NOT use it for production, mission-critical, or genuinely
> sensitive communications. Using it means you accept everything below.**

- **Purpose limitation**:
  This project is intended for **cryptographic protocol research, software
  testing, and educational purposes only**. Do not use it for any illegal
  purpose (including but not limited to unauthorized interception, harassment,
  or transmitting unlawful content).
- **Legal and regulatory compliance**:
  The use of end-to-end encryption tools **may be regulated or restricted by
  the laws of your jurisdiction**, including encryption regulations and export
  control laws. It is **solely the user's responsibility** to determine and
  comply with all applicable laws before using this project.
- **Known security boundary**:
  This system provides secrecy, not identity authentication (the telephone
  model) — unless both sides use a shared secret or compare the SAS word, an
  active man-in-the-middle can relay the conversation; recognizing the other
  end is the user's own responsibility. Assess the risks before use.
- **No warranty whatsoever**:
  This software is provided under its license terms **without any warranty,
  express or implied**, including but not limited to merchantability, fitness
  for a particular purpose, accuracy, reliability, non-infringement, or
  security. **No guarantee of uninterrupted availability, error-free
  operation, or continued maintenance is given.**
- **No support obligation**:
  The author **has no obligation to provide any user with technical support,
  bug fixes, updates, or assistance of any kind, at any time**.
- **Limitation of liability**:
  To the maximum extent permitted by applicable law, **in no event shall the
  author or contributors be liable for any direct, indirect, incidental,
  special, punitive, or consequential damages arising from the use of or
  inability to use this software**, including but not limited to data loss,
  account bans, legal liability, or security incidents, whether or not
  advised of the possibility.
- **User responsibility**:
  Users assume **all** risks and legal responsibilities arising from using
  this project and from obtaining it through any channel.
- **Severability**:
  If any provision of this disclaimer is found invalid or unenforceable, the
  remaining provisions remain fully in force.
- **Acceptance and changes**:
  Continued use of this project constitutes acceptance of this disclaimer.
  The author reserves the right to update these terms at any time; the
  current version in this repository prevails.
- **Final interpretation**:
  The author reserves the final right of interpretation of this disclaimer.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
