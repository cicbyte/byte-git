# byte-git

> A Git power toolkit (CLI + desktop): branch governance, named stashes, multi-repo dev stats, full backups and template management — all in one command.

**中文**: [README.md](README.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-dea584?logo=rust)](https://www.rust-lang.org)
[![Release](https://github.com/cicbyte/byte-git/actions/workflows/release.yml/badge.svg)](https://github.com/cicbyte/byte-git/actions/workflows/release.yml)
[![Platforms](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey)](https://github.com/cicbyte/byte-git/releases)

<!-- screenshot: add a desktop demo screenshot here (GUI main window / logview TUI) -->

## Table of Contents

- [Feature Matrix](#feature-matrix)
- [Install & Build](#install--build)
- [Quick Start](#quick-start)
- [Project Structure](#project-structure)
- [Config & Data](#config--data)
- [Desktop Architecture](#desktop-architecture)
- [License](#license)

## Feature Matrix

| Command | Description |
|---|---|
| `version` | Version info |
| `config` | Global config (YAML) get/set/list |
| `branch` | Clean merged branches, list orphans |
| `stash` | Named stash save/pop/list with search |
| `info` | Repo summary / health check |
| `stats` | Multi-repo commit stats, activity heatmap, streaks, languages |
| `backup` | Full repo backup (git clone --mirror) |
| `license` | 47 OSS license templates, create & detect |
| `ignore` | .gitignore template management (idempotent append) |
| `clone` | Proxy-accelerated clone (Lua rules engine) |
| `download` | GitHub Release asset download |
| `archive` | Repository snapshot archiving |
| `ai` | AI commit message / AI rewrite (gen→run→apply) |
| `rewrite` | Commit history metadata rewriting |
| `review` | AI code review (external OCR integration) |
| `secret` | Secret leak scanning (external gitleaks) |
| `hook` | Git hook management (Lua/scripts) |
| `rebase` | Rebase preview & autosquash |
| `undo` | Safe undo (last commit/rebase/merge, with backup) |
| `logview` | Git log TUI browser (ratatui) |

## Install & Build

**Prebuilt artifacts**: CLI binaries (Linux / Windows / macOS amd64 + macOS arm64) plus the Windows desktop installer (msi / nsis) — built automatically by CI after a `v*` tag is pushed or the [Tag Release](https://github.com/cicbyte/byte-git/actions/workflows/tag.yml) workflow is triggered. Grab yours from [Releases](https://github.com/cicbyte/byte-git/releases).

**From source**: requires Rust >= 1.85 (2024 edition), Git >= 2.0.

```bash
git clone https://github.com/cicbyte/byte-git
cd byte-git
cargo build --release -p byte-git-cli
# Binary: target/release/byte-git
```

**Desktop** (Tauri v2, needs Node >= 20):

```bash
cd apps/desktop
npm --prefix frontend install
cargo tauri build          # or: cargo tauri dev
```

## Quick Start

```bash
byte-git version                     # Show version
byte-git info summary                # Repo overview
byte-git info health                 # Health check
byte-git branch clean --dry-run      # Preview merged branches to clean
byte-git stash save --name wip       # Named save
byte-git stash list --search wip     # Search
byte-git stats --days 30             # Dev stats of last 30 days
byte-git backup create               # Mirror-backup current repo
byte-git license create mit          # Create MIT LICENSE
byte-git ignore add rust node        # Append rust/node gitignore templates
```

## Project Structure

```
byte-git/
├── apps/
│   ├── cli/                 # CLI (clap command tree, binary: byte-git)
│   └── desktop/             # Tauri v2 desktop app
│       ├── frontend/        # Vue 3 + Vite + Pinia (with Wails→Tauri adapter)
│       └── src-tauri/       # Tauri command layer (thin wrapper over crates/core)
├── crates/
│   └── core/                # Domain core: git ops / config / backup / stats / templates
│       └── assets/          # Embedded assets: 47 licenses + gitignore templates + AI prompts
├── docs/                    # Command docs
└── rules/ prompts/          # Lua proxy rules / prompt sources
```

## Config & Data

```
~/.cicbyte/apps/byte-git/   # App root (old ~/.cicbyte/byte-git/ auto-migrated on first run)
  config/config.yaml    # Global config (AI model, GitHub token, feature toggles)
  backups/              # Full repo backups
  prompts/              # AI prompt templates (customizable)
~/.cicbyte/data/        # Cross-project shared data (cicbyte tool family)
.git/byte-git/          # Per-repo data (hook config, rewrite JSON, AI cache)
```

## Desktop Architecture

The frontend calls backend methods via a `window.go.main.ByteGitApp.Method(...)` binding API.
`frontend/src/wailsjs/` ships an adapter layer bridging it to Tauri:

- `go/main/ByteGitApp.ts` — Proxy bridge: camelCase method → snake_case Tauri command, arguments marshalled via a signature table
- `runtime/runtime.ts` — `EventsOn/EventsOff/EventsEmit` → Tauri events

The frontend runs on Tauri unchanged. The Rust command layer exposes **262 commands** organized into
15 domain modules (commit / gitops / backup+log / info+stats /
repo / rewrite / secret / license / system / ignore / github / reference / live / terminal / wallpaper):
covering git operations, AI commit & rewrite, secret scanning, backup & archiving, log browsing, settings
and template management, plus extended domains such as GitHub REST integration, live file watching (notify),
embedded terminal (portable-pty ConPTY) and wallpaper management. The frontend method surface is fully covered.

## License

[MIT](LICENSE) © 2026 cicbyte
