# Vendored third-party source

Unmodified copies of published crates, vendored so that line ranges stay
stable and checksum-verified in CI (`vendor/pin.toml`). Do not edit anything
under these directories. All project-authored content lives in
`annotations/`, `glossary/`, `examples/`, `app/`, and `xtask/`.

What each role means is in `vendor/pin.toml`; why a glossary source is quoted
rather than annotated is D9 in `docs/decisions.md`.

## serde_core 1.0.229 (coverage)

- crates.io: https://crates.io/crates/serde_core/1.0.229
- Upstream: https://github.com/serde-rs/serde
- sha256: `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48`
- License: MIT OR Apache-2.0 (see `serde_core-1.0.229/LICENSE-MIT` and `serde_core-1.0.229/LICENSE-APACHE`)
- Copyright: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>

## serde_derive 1.0.229 (narrative)

- crates.io: https://crates.io/crates/serde_derive/1.0.229
- Upstream: https://github.com/serde-rs/serde
- sha256: `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348`
- License: MIT OR Apache-2.0 (see `serde_derive-1.0.229/LICENSE-MIT` and `serde_derive-1.0.229/LICENSE-APACHE`)
- Copyright: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>

## syn 3.0.3 (glossary)

- crates.io: https://crates.io/crates/syn/3.0.3
- Upstream: https://github.com/dtolnay/syn
- sha256: `53e9bae58849f64dfa4f5d5ae372c8341f7305f82a3868709269343628b659a3`
- License: MIT OR Apache-2.0 (see `syn-3.0.3/LICENSE-MIT` and `syn-3.0.3/LICENSE-APACHE`)
- Copyright: David Tolnay <dtolnay@gmail.com>

## quote 1.0.47 (glossary)

- crates.io: https://crates.io/crates/quote/1.0.47
- Upstream: https://github.com/dtolnay/quote
- sha256: `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001`
- License: MIT OR Apache-2.0 (see `quote-1.0.47/LICENSE-MIT` and `quote-1.0.47/LICENSE-APACHE`)
- Copyright: David Tolnay <dtolnay@gmail.com>

## proc-macro2 1.0.107 (glossary)

- crates.io: https://crates.io/crates/proc-macro2/1.0.107
- Upstream: https://github.com/dtolnay/proc-macro2
- sha256: `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9`
- License: MIT OR Apache-2.0 (see `proc-macro2-1.0.107/LICENSE-MIT` and `proc-macro2-1.0.107/LICENSE-APACHE`)
- Copyright: David Tolnay <dtolnay@gmail.com>, Alex Crichton <alex@alexcrichton.com>
