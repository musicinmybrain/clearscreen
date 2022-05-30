[![Crate release version](https://badgen.net/crates/v/clearscreen)](https://crates.io/crates/clearscreen)
[![Crate license: Apache 2.0 or MIT](https://badgen.net/badge/license/Apache%202.0%20or%20MIT)][copyright]
![MSRV: 1.51.0 (breaking)](https://badgen.net/badge/MSRV/1.51.0%20%28breaking%29/green)
[![Bors enabled](https://bors.tech/images/badge_small.svg)](https://app.bors.tech/repositories/45671)
[![CI status on main branch](https://github.com/watchexec/clearscreen/actions/workflows/main.yml/badge.svg)](https://github.com/watchexec/clearscreen/actions/workflows/main.yml)

# ClearScreen

_Cross-platform terminal screen clearing library._

- **[API documentation][docs]**.
- [Dual-licensed][copyright] with Apache 2.0 and MIT.
- Minimum Supported Rust Version: 1.51.0.

[copyright]: ./COPYRIGHT
[docs]: https://docs.rs/clearscreen

Tested with and tweaked for over 80 different terminals, multiplexers, SSH clients.
See my research notes in the [TERMINALS.md](./TERMINALS.md) file.

## Quick start

```toml
[dependencies]
clearscreen = "1.0.9"
```

```rust
clearscreen::clear().unwrap();
```
