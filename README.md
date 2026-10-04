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

## The idea

| A person                             | This program                                                   |
|--------------------------------------|----------------------------------------------------------------|
| Ears                                 | open from birth — listening from the moment the process starts |
| A mouth                              | `/dial` — reach out to anyone, anytime, no restart needed      |
| A language only the two of you speak | end-to-end encryption, fresh keys every conversation           |
| Turning away and forgetting          | zero persistence — nothing is ever written to disk             |
| Recognizing who you're talking to    | **your job**, after decryption                                 |

The last row is deliberate. This is the *telephone model* of security: the wire is
private, but the person on the other end introduces themselves however they like.
The system guarantees **secrecy**, never **identity**. Recognizing a voice —
through shared secrets, in-jokes, a familiar tone — is a human act, and it stays
human. (If you do need certainty about who is on the other end, agree on a secret
out-of-band and use it when dialing — see [Secrets](#secrets-knowing-who-is-on-the-other-end).)

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
relays the handshake (the telephone model's accepted cost — mitigated by
secrets), and the other end itself (they can copy, paste, screenshot).

### Key details worth knowing

- **Traffic shape obfuscation** — handshake and chat frames carry random
  padding, file chunks are cut at randomized sizes: the same sentence sent
  eight times produces eight different frame lengths. A passive observer
  cannot fingerprint message sizes.
- **Files never touch the text lane** — binaries arrive byte-for-byte, no
  text mangling, up to 32 MB per file.
- **Group chat rekeys on every join** — the new member triggers a fresh key;
  there is no roster to leak because none is ever stored.
- **The name on your screen comes from the wire, not the plaintext** — no
  one can put words in another's mouth inside a conversation or a group.

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
| `--hidden`      | Never answer LAN broadcast probes (`/shout` cannot detect you); exact-address dials still work.                                                                                       |
| `--no-chain`    | Don't reserve next-time secrets at farewell, don't accept others' — the right shape for a desk open to strangers.                                                                     |
| `--guard <KEY>` | Stable door lock: every caller must pass the same machine key (32-byte random), never consumed — wrong-key floods can't lock the desk. Pair with `--no-chain` for an armed open desk. |
| `--courier`     | Open a teahouse (see [Behind NAT](#two-people-behind-nat)) — the same binary in its other job.                                                                                        |

### Command reference

| Command                                   | What it does                                                                                           |
|-------------------------------------------|--------------------------------------------------------------------------------------------------------|
| `/dial <addr> [secret]`                   | Reach out to someone; with a secret, both sides must match.                                            |
| `/await <secret>`                         | Arm a one-shot secret: the next arrival must match (verified then burned — port scans can't waste it). |
| `/meet <teahouse-addr> <room> [secret]`   | Meet a pre-agreed person through a teahouse (both pick the same room number).                          |
| `/punch <introducer-addr> <tag> [secret]` | Attempt a *direct* line through a UDP hole punch (both register the same tag).                         |
| `/file <path>`                            | Hand a file (≤32 MB) to the focused conversation.                                                      |
| `/talk <id\|name>`                        | Switch attention among parallel conversations.                                                         |
| `/list`                                   | Who's currently connected to you.                                                                      |
| `/card`                                   | Print your name and dial address for every network interface.                                          |
| `/shout`                                  | Broadcast "anyone there?" on the LAN; listeners answer with name + address.                            |
| `/circle`                                 | Form a group key with everyone currently connected (all must have dialed each other).                  |
| `/gsay <text>`                            | Speak in the circle — encrypted once, everyone receives.                                               |
| `/bye`                                    | Say goodbye to the focused conversation.                                                               |
| `/quit` or Ctrl-D                         | Say goodbye to everyone and leave. Ctrl-C walks away immediately.                                      |

### Group chat

1. Everyone dials everyone (full mesh — that's what makes it listener-free).
2. Anyone runs `/circle` — each member contributes randomness, the group key
   is derived from all of it. Nobody dictates it, nobody can predict it.
3. `/gsay <text>` — one ciphertext, fanned out to every link.
4. Someone new joins? They dial everyone, then anyone re-runs `/circle` —
   that *is* the rekey rule. Practical ceiling: ~8 people (an N-person circle
   needs N² links; bigger gatherings need a stage, which is a different project).

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

### Secrets: knowing who is on the other end

The cure for the man-in-the-middle gap is one human act: agree on a secret
out-of-band (in person, by phone — any channel but this one), then use it:

- `/dial <addr> <secret>` — the handshake now includes a cryptographic proof
  that both sides hold the *same* secret. The secret itself never crosses the
  wire in any derivable form; a wrong secret fails loudly; offline guessing
  does not exist.
- `/await <secret>` — arm it on your side before someone arrives.
- **The secret chain** — at farewell, the next secret can be reserved inside
  the already-authenticated channel, so tomorrow's conversation re-authenticates
  automatically. It lives in RAM only; a restart returns you to the out-of-band
  reading. A desk open to strangers opts out with `--no-chain`.
- **A weak secret only hurts that one handshake instant** — the secret never
  encrypts the conversation itself; every conversation key is fresh and
  ephemeral.

## For developers

The core is a Rust library (`e2ee`); this CLI is merely its first consumer,
and the same "person" can live inside any program — Rust via the crate, other
languages via the C ABI (`libe2ee.so` / `e2ee.dll`, header `include/e2ee.h`,
frozen add-only surface). `cargo test` covers the full behavior. Design
invariants and red lines live in [README_AGENT.md](README_AGENT.md) — read it
before touching anything.

## Known limitations

- **No liveness probes.** A peer that vanishes without closing (or over a
  punched UDP line, which has no EOF) leaves the line and its `/list` entry
  hanging until you `/quit`.
- **The introducer's referral is unauthenticated.** Anyone who knows the tag
  can register as your "peer" — bring a secret if you need to know who's there.
- **The teahouse pairs by room number alone** — pick unguessable ones.
- **Group consistency is by convention** — a botched simultaneous re-`/circle`
  surfaces as "mumble" notices; re-run `/circle` to converge. One circle per
  process.
- **`--hidden` is not invisibility.** It silences the broadcast reflex only;
  a port scanner can still find the open port (nothing tells it what it is).
  True "nobody gets in" adds `/await <secret>`.
- **Cross-version chat is one-way.** The size-padded chat frame is invisible
  to older builds (silently skipped, no garbage); old→new still reads fine.
  Upgrade both ends together.

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
- **Known security boundaries**:
  This system provides secrecy, not identity verification (the telephone
  model) — an active man-in-the-middle can relay conversations unless both
  sides use a shared secret; recognizing the other end is the user's own
  responsibility. Assess the risks before use.
- **No warranty of any kind**:
  This software is provided under its license terms **without any express or
  implied warranty**, including but not limited to merchantability, fitness
  for a particular purpose, accuracy, reliability, non-infringement, or
  security. There is **no guarantee of uninterrupted availability, error-free
  operation, or continued maintenance**.
- **No support obligation**:
  The author is **not obligated to provide technical support, bug fixes,
  updates, or any assistance**, to any user, at any time.
- **Limitation of liability**:
  To the maximum extent permitted by applicable law, **in no event shall the
  authors or contributors be liable for any direct, indirect, incidental,
  special, exemplary, or consequential damages arising from, or in any way
  related to, the use or inability to use this software**, whether advised of
  the possibility. This includes but is not limited to data loss, account
  bans, legal liability, or security incidents.
- **User responsibility**:
  Users bear **all** risks and legal responsibilities arising from their use
  of this project, and from obtaining it from any channel.
- **Severability**:
  If any provision of this disclaimer is held invalid or unenforceable, the
  remaining provisions remain in full force and effect.
- **Acceptance and changes**:
  Continued use of this project constitutes acceptance of this disclaimer.
  The author reserves the right to update these terms at any time; the
  current version in this repository governs.
- **Final interpretation**:
  The author of this project reserves the right of final interpretation of
  this disclaimer.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
