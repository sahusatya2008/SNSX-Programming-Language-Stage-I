# SNSP: Smart Neural Secure Protocol

SNSP is the language-native networking substrate for SNSX.

## Properties

- Transport-agnostic abstraction
- Default encrypted channels
- Stream and RPC frames
- Identity-first handshakes
- Low-latency datagram-friendly framing

## Frame layout

```text
+----------+---------+-----------+-------------+-----------------+
| version  | flags   | stream_id | message_len | message payload |
+----------+---------+-----------+-------------+-----------------+
```

Flags:

- `OPEN`
- `DATA`
- `CLOSE`
- `RPC`
- `ACK`

## Handshake

1. Client sends signed hello with ephemeral X25519 public key.
2. Server verifies identity and replies with signed accept payload.
3. Both peers derive an AES-256 session key.
4. All subsequent frames are encrypted and authenticated.

