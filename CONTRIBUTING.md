# Contributing To LDraw

Thanks for contributing.

## 1) Prerequisites

- Rust toolchain (stable)
- Linux native dependencies (see `README.md`)

Optional but useful:

- `rustup component add rustfmt clippy`

## 2) Setup

```bash
git clone <your-fork-or-repo-url> ldraw
cd ldraw
make check
```

Run the app:

```bash
make run
```

Useful targets:

- `make run`
- `make check`
- `make release`
- `sudo make install`
- `sudo make uninstall`

## 3) Branching

Create a feature/fix branch:

```bash
git checkout -b feat/short-description
```

Examples:

- `feat/minimap-interaction`
- `fix/toolbar-layout`
- `docs/install-guide`

## 4) Coding Guidelines

- Keep changes focused and small.
- Match existing style and naming patterns.
- Prefer clear, maintainable code over clever code.
- Avoid unrelated refactors in the same PR.

## 5) Before Opening A PR

Run these locally:

```bash
make check
```

Recommended:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## 6) Pull Request Checklist

- [ ] Code builds cleanly.
- [ ] Tests pass.
- [ ] Formatting is applied.
- [ ] PR description explains what changed and why.
- [ ] Screenshots/GIFs included for visible UI changes.

## 7) Commit Messages

Use short imperative messages, e.g.:

- `fix minimap collapsed click behavior`
- `add phosphor icons to toolbar`
- `update linux build docs`

## 8) Reporting Bugs / Requesting Features

Please include:

- Linux distro and version
- GPU/driver info (if rendering/input issue)
- Steps to reproduce
- Expected vs actual behavior
- Screenshot or short recording if possible
