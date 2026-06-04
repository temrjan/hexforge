# hexforge

Vanity Ethereum address forge — generate `0x…` addresses that contain a chosen
word at the **start**, **anywhere**, or the **end**, through a minimalist native
GUI for Linux / Fedora.

> **Status: early scaffold (WIP).** The engine and GUI are built in phases — see
> the roadmap below.

## What it does

- Searches for ETH addresses matching a hex word (`0-9a-f`), e.g. `0x…deadbeef`.
- CPU engine produces full wallets **with a 12-word BIP-39 mnemonic**.
- *(Planned)* GPU mode via an external OpenCL tool for long words — raw key only.

## Security

This tool generates **private keys**. Treat it accordingly:

- Keys come from the operating-system CSPRNG; secrets are zeroized in memory.
- Nothing is written to disk unless you explicitly export.
- For real funds, run it offline and write the mnemonic / key to paper.
- **Never commit generated wallets** — they are git-ignored by default.

## Build (Fedora)

The release artifact is a single binary, but building the GUI needs desktop
development libraries:

```bash
sudo dnf install gcc libxkbcommon-devel wayland-devel mesa-libGL-devel
cargo build --release
```

## Roadmap

- [x] Scaffold + input-validation primitives
- [ ] **PR1** — core CPU engine (mnemonic, match modes, derivation) + tests
- [ ] **PR2** — egui GUI (search, live progress, results)
- [ ] **PR3** — security / UX (masked secrets, clipboard, export, offline badge)
- [ ] **PR4** — optional GPU engine (external `profanity2` + safe key combine)
- [ ] **PR5** — packaging (`.desktop`, RPM / Flatpak)

## License

[MIT](LICENSE)
