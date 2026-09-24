# Contract WASM Size Audit

Audit of the compiled contract sizes, where the bytes go, and what was done to
reduce them (#344).

Why this matters: on Soroban, uploading contract code is charged per byte
written, and the code is a ledger entry whose rent also scales with its size.
Every byte cut from a contract reduces the install fee and its ongoing rent,
and gives more room under the network's maximum contract size
(`contract_max_size_bytes`).

## How to reproduce

```bash
./scripts/measure-size.sh
```

This builds the release WASM, then runs `scripts/optimize-wasm.sh` with and
without `--strip-docs` and prints all three sizes.

Measurement setup for the numbers below: `soroban-sdk` 21.7.7, rustc 1.94.1,
target `wasm32-unknown-unknown`, the existing release profile in `Cargo.toml`
(`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`,
`strip = "symbols"`), and `stellar contract optimize` from stellar-cli 25.1.
Exact byte counts shift slightly with the rustc version.

## Results

| Contract | Baseline (raw) | After this change (raw) | Optimized (deployed by default) | Optimized + `STRIP_SPEC_DOCS=1` |
|---|---:|---:|---:|---:|
| learn-token | 103,196 B | 103,196 B | 82,082 B (−20.5%) | 60,945 B (−40.9%) |
| credential-nft | 50,541 B | 49,476 B | 38,608 B (−23.6%) | 28,500 B (−43.6%) |
| progress-tracker | 88,319 B | 88,319 B | 72,694 B (−17.7%) | 46,281 B (−47.6%) |
| **Total** | **242,056 B** | **240,991 B** | **193,384 B (−20.1%)** | **135,726 B (−43.9%)** |

Percentages are measured against the baseline raw build.

## Where the bytes go

Section breakdown of each baseline binary after the wasm-opt pass:

| Section | learn-token | credential-nft | progress-tracker |
|---|---:|---:|---:|
| `code` (function bodies) | 43,730 | 20,835 | 32,020 |
| `contractspecv0` (interface spec) | 29,343 | 13,407 | 33,051 |
|  ↳ of which doc comments | 21,369 | 10,243 | 26,857 |
| `data` (constants and strings) | 6,782 | 3,780 | 5,866 |
| `export` (entry-point names) | 1,255 | 608 | 1,004 |
| everything else | 972 | 650 | 753 |

The contract spec takes up 34–45% of each binary. `#[contractimpl]` and
`#[contracttype]` copy every `///` doc comment on public functions, arguments,
structs, fields and enum variants into this section verbatim (up to 1 KB per
item). For progress-tracker, doc comments alone make up 37% of the deployed
bytes.

### Largest functions (raw build, symbols kept)

| Contract | Function | Bytes |
|---|---|---:|
| learn-token | SDK `FromVal` conversion (monomorphized for contract types) | 3,199 |
| learn-token | `claim_vested` | 1,116 |
| learn-token | `compiler_builtins::u128_div_rem` (i128 math) | 1,084 |
| learn-token | `claim_reward` / `vote` | 869 / 853 |
| progress-tracker | SDK `FromVal` conversion | 1,628 |
| progress-tracker | `create_course` | 1,100 |
| progress-tracker | `complete_module_in_place` / `enroll_checked` | 938 / 938 |
| credential-nft | SDK `FromVal` conversion | 1,451 |
| credential-nft | `mint_credential` | 1,401 |
| credential-nft | `revoke_credential_with_reason` / `revoke_credential` | 1,170 / 1,128 |

## Optimizations applied

### 1. wasm-opt pass on every deploy and upgrade

`scripts/optimize-wasm.sh` runs `contract optimize` (binaryen wasm-opt) on
each contract and writes `<name>.optimized.wasm`. `deploy.sh`, `upgrade.sh`
and `verify.sh` now build, optimize, and upload or compare the optimized
artifact. This removes 17–24% with no change in behavior. Before this change
the scripts uploaded the unoptimized `cargo build` output.

### 2. Deduplicated credential revocation (credential-nft)

`revoke_credential` and `revoke_credential_with_reason` contained the same
~60 lines (auth, revoked-flag write, learner- and course-index pruning), which
compiled into two ~1.1 KB copies. Both now call one shared
`revoke_and_prune` helper. Index pruning also uses the host-side
`Vec::first_index_of` instead of a guest-side loop that fetched and compared
each element. That is smaller, and it costs fewer host calls per revocation.
Savings: 1,065 B raw and 672 B after wasm-opt. Events, storage writes and
panic messages are unchanged, and all credential-nft tests pass.

### 3. Removed unused code (learn-token)

`storage::check_allowance_expired_readonly` had no callers and was hidden
behind `#[allow(dead_code)]`. The linker was already dropping it, so it did
not affect binary size. It was removed as source cleanup.

### 4. Optional: strip doc comments from the on-chain spec

`STRIP_SPEC_DOCS=1 ./scripts/deploy.sh testnet` (or
`./scripts/optimize-wasm.sh --strip-docs`) blanks every doc string in
`contractspecv0`. The spec is decoded and re-encoded with the CLI's own XDR
codec (`stellar xdr decode/encode --type ScSpecEntry`). Function names,
argument names and types, and return types are kept, so clients, `contract
invoke` and `contract bindings` keep working. This saves another 10–26 KB per
contract.

**Trade-off:** explorers and bindings generated from the *on-chain* WASM stop
showing the doc text. The docs stay in the source and in this repository's
README. The option is off by default so maintainers can decide per network.
One suggested policy is to strip docs on mainnet and keep them on testnet.
`verify.sh` must run with the same setting that was used for the deployment.

Verification: the optimized and doc-stripped binaries were loaded into the
Soroban test host (`Env::register_contract_wasm`) and invoked successfully,
and `stellar contract info interface` decodes all spec entries (87, 33 and 60).

## Options evaluated and not taken

| Change | Result |
|---|---|
| `opt-level = "s"` instead of `"z"` | credential-nft −1.4 KB, learn-token +0.3 KB, progress-tracker +1.1 KB. No clear gain, so `"z"` stays. |
| `wasm32v1-none` target instead of `wasm32-unknown-unknown` | Within ±1 KB. Worth adopting for toolchain compatibility later, but it is not a size lever. |

The release profile (`Cargo.toml:122-130`) was already set up for size. None
of its settings needed to change.

## Remaining opportunities (follow-up work)

1. **`core::fmt` machinery kept alive by string panics (about 4.5 KB per
   contract).** `panic!("...")`, `panic!("{} ...", x)` and
   `Option::expect("...")` put `core::fmt` function pointers into the WASM
   function table. That keeps `fmt::write`, `Formatter::pad`,
   `pad_integral`, integer `Display` and `Error as Debug` in the binary, even
   though the SDK panic handler never prints anything. Replacing them with
   `#[contracterror]` enums and `panic_with_error!` would remove this code and
   give clients typed error codes. The 152 `#[should_panic(expected = "...")]`
   tests match on the message text, so this needs a coordinated test
   migration and should be its own PR.
2. **i128 arithmetic builtins in learn-token (about 1.8 KB).** 128-bit
   multiplication and division (`u128_div_rem`, `__muloti4`) come from
   reward, vesting and governance math. Where the operands fit in 64 bits,
   doing that math in `u64`/`i64` would drop these helpers.
3. **Large contract types.** The monomorphized `FromVal` conversions are the
   largest single functions (1.4–3.2 KB). Splitting rarely-read fields out of
   large structs such as `Course` and `ProgressInfo` into separate storage
   entries would shrink both the conversion code and the per-read storage
   cost.
4. **Entry-point count.** learn-token exports 70 functions. Some are
   off-chain helpers, such as `estimate_claim_gas` and `get_storage_size`,
   that could move to the indexer or API, saving their code, export and spec
   bytes.
5. **Shorter doc comments.** If stripping docs on-chain is not wanted, trimming
   the longest function docs (for example `transfer` in credential-nft, which
   has several paragraphs) to one-line summaries, and moving long explanations
   into module-level docs, keeps most of the benefit.
