# Ogema — Upstream Guarantees & Invariants

Three crates: `ogema` (the searcher), `geyser-watcher` (standalone gRPC
listener, Raydium vault prices), `sentinel` (standalone gRPC listener,
pre-funded wallet detection). This document states what each part assumes
about its upstream, and what it guarantees about its own behavior. Every
claim here should be checkable against the code directly.

## 1. Why three crates, not one

`yellowstone-grpc-client` pulls `solana-transaction` v2.x internally (Agave's
newer split crates). `solana-client`/`solana-sdk` 1.18 (monolithic, used for
Jito bundle construction and RPC calls everywhere else in `ogema`) cannot
coexist with that in one dependency tree: `ed25519-dalek 1.0.1` (pinned via
the v2 chain) requires `zeroize ^1`, while `rustls 0.23` (pinned via `tonic`,
pulled in by yellowstone) requires `zeroize ^1.7`. No single version of
either package satisfies both. Confirmed via `cargo check`, not assumed.

**Invariant:** `ogema`'s own `Cargo.toml` never depends on
`yellowstone-grpc-client`, `yellowstone-grpc-proto`, or `tonic`, directly or
transitively. If a future change reintroduces that dependency, the build
will fail to resolve, not silently misbehave.

## 2. IPC contract between crates

`geyser-watcher` and `sentinel` each own a Unix socket:
- `geyser-watcher` writes `ARBSIGNAL <pool_price> <pyth_price> <deviation> <direction> <reserve_in> <reserve_out>` to `/tmp/geyser_arb.sock`
- `sentinel` writes `SIGNAL <pubkey> <lamports> <slot>` to `/tmp/sentinel.sock`

`ogema`'s `arb_receiver.rs` and `sentinel_receiver.rs` parse these lines.

**Invariant:** the wire format is plain whitespace-separated text, not a
binary protocol, so either side can be inspected directly by reading raw
socket output, no decoder or special tooling required. Field order is
fixed; adding a field requires updating both the writer and the parser in
the same commit, they are not independently versioned.

**Assumption:** the socket exists and `ogema` is running before
`geyser-watcher`/`sentinel` attempt to connect. If not, both retry silently
(`eprintln!` + reconnect loop) rather than crash. This means a signal sent
while `ogema` is down is simply lost, not queued.

## 3. Bundle construction guarantees

`bundle.rs`'s `build()` always constructs exactly three transactions in
this order: burnCu lock, the actual instruction, tip transfer. `send()`
fires all four Jito regional endpoints in parallel and returns on the
first that responds with a bundle ID, cancelling nothing, the other three
requests complete or fail independently.

**Invariant:** `build()` is a pure function of its inputs, same keypair,
same instruction, same tip, always produces the same three transactions
(modulo blockhash, which is fetched fresh each call). No hidden state.

**Assumption:** `rpc.get_latest_blockhash()` returns a blockhash valid long
enough for the bundle to land. This is a real timing assumption not
verified anywhere in code, if RPC latency is high, the blockhash can expire
before the bundle reaches a leader.

## 4. Pump.fun instruction correctness

`pump_buy.rs` and `pump_sell.rs` are generated from the live on-chain
Anchor IDL, fetched directly from its PDA
(`createWithSeed(base, "anchor:idl", programId)`, zlib-decompressed), a more
improved version from the one used in triggger2nest.

**Verified, not assumed:**
- Both discriminators checked against the IDL's own discriminator field
- The `global` PDA derivation confirmed against a live on-chain account
  (exists, owned by the pump program)
- The bonding-curve reserve layout confirmed against a real curve reading
  80.11 SOL / 402M tokens, correct order of magnitude near graduation
- `fee_recipient`'s offset in the Global account (41 bytes in) confirmed
  by decoding the real Global state account and checking the resulting
  pubkey holds real SOL as a System Program wallet, not just that it
  decodes without erroring

**Invariant:** account ordering in both instruction builders is IDL-exact
and load-bearing. Do not reorder `AccountMeta`s to "clean up" the code,
Solana instructions are positional, not named, at the runtime level.

## 5. Exit monitor guarantees

`exit_monitor.rs`'s `watch()` polls the bonding curve's virtual reserves
directly, no oracle, no external price feed, computing
`price = v_sol_reserves / v_token_reserves` from bytes read straight off
the account. Four exit conditions checked in a fixed priority order every
poll: take-profit, stop-loss, trailing-stop, timeout, first one to
evaluate true wins that poll cycle.

**Invariant:** once any exit condition fires, `watch()` returns immediately
with a built sell instruction, it does not wait for the sell to land
before returning. The caller (`snipe.rs`) is responsible for actually
submitting it.

**Assumption:** the bonding curve account layout (`v_token@8, v_quote@16,
complete@48, creator@49`) is stable. Confirmed against one live curve at
time of writing, not guaranteed to hold if pump.fun changes the account
layout in a future program upgrade, there is no version check on this.

## 6. What snipe.rs does NOT guarantee (open gaps)

- **Token mint resolution.** `sentinel`'s signal carries a funded wallet
  pubkey, not a token mint. `snipe::execute()` takes a `mint: Pubkey`
  parameter directly, something upstream of it must resolve wallet to mint
  before calling this, that resolution does not exist yet.
- **No classifier gate.** `snipe::execute()` will attempt to buy any mint
  it is given, provided the curve has not migrated. There is no filter for
  rug risk, holder concentration, or any other signal, that decision layer
  is not built.
- **Token amount after buy is an estimate**, not a read of the actual ATA
  balance. `exit_monitor::watch()` and the eventual sell instruction use
  `est_tokens` computed from the pre-trade price, not the confirmed
  post-trade balance. Slippage means the real balance may differ.

## 7. How to check this document against the code

Every invariant above traces to a specific file:
- §1 → `Cargo.toml` (root), compare against `geyser-watcher/Cargo.toml`
- §2 → `arb_receiver.rs`, `sentinel_receiver.rs`, and the corresponding
  `signal::fire()` functions in `geyser-watcher`/`sentinel`
- §3 → `bundle.rs`
- §4 → `pump_buy.rs`, `pump_sell.rs`
- §5 → `exit_monitor.rs`
- §6 → `snipe.rs`, read the function signature and what it does not check

Section 6 gaps are listed here specifically so review time goes to
verifying what exists, not rediscovering what does not.
