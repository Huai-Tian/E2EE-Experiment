# E2EE-Experiment

> One binary. It listens like a person, speaks like a person, and forgets like a person.

**E2EE-Experiment** takes a single metaphor literally: *the executable is a person.*
No server, no accounts, no database — just a program that can hold up its end of a
private conversation, then remember nothing.

[中文版](README_ZH.md) · [README_AGENT.md](README_AGENT.md) is for machines, not you.

## The model

| A person | This program |
|---|---|
| Ears | `listen` — waits on a TCP port for someone to arrive |
| A mouth | `dial` — reaches out to someone else's port |
| A language only the two of you speak | Noise NN end-to-end encryption |
| Turning away and forgetting | Zero persistence — nothing is ever written to disk |
| Recognizing who you're talking to | **Your job**, after decryption |

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

## Quick start

```bash
cargo build --release

# Terminal 1 — the ear
./target/release/E2EE-Experiment listen 0.0.0.0:7777 -n Alice

# Terminal 2 — the mouth
./target/release/E2EE-Experiment dial 127.0.0.1:7777 -n Bob
```

Type a line, press enter, and it appears on the other side (encrypted in transit).
`/quit` or Ctrl-D hangs up politely; Ctrl-C walks away immediately.

Chat goes to stdout, connection events to stderr — so the conversation itself
stays pipeable.

## Wire protocol

- **Transport**: TCP
- **Framing**: every message (handshake or data) is `u16 big-endian length + payload`
- **Handshake**: Noise `NN` (`Noise_NN_25519_ChaChaPoly_BLAKE2s`), dialer = initiator
    - `-> e` · `<- e, ee`
- **First data frame, both directions**: the self-reported name (UTF-8, unsigned,
  purely cosmetic)
- **Chat frames**: one UTF-8 line each, encrypted as Noise transport messages
- **Hang-up**: an empty plaintext frame (a bare AEAD tag on the wire), or simply
  closing the connection

## Where this could go

| Idea | Metaphor | Constraint it must respect |
|---|---|---|
| UDP hole punching | An introduction by a mutual friend | Must never touch plaintext |
| Ciphertext-only relay | A courier | Forwards opaque bytes, keeps nothing |
| IPv6-first direct dials | Two people in the same room | Already how it works |
| Group conversations | A small gathering | Every pairwise channel still E2EE |

Anything that stores messages, verifies identity, or introduces a privileged node
breaks the model — it belongs in a different project.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
