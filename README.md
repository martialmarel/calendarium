# Calendarium

A small macOS menu bar app that shows today's day number and pops up a calendar on click. Written in Rust with [egui](https://github.com/emilk/egui) / [eframe](https://crates.io/crates/eframe) and [tray-icon](https://crates.io/crates/tray-icon).

## Why

[Day-O](https://www.shauninman.com/archive/2009/08/27/day_o_mac_menu_bar_clock) by Shaun Inman was the perfect tool: a simple calendar, sitting in the menu bar, one click away. It is no longer maintained, and its Intel-only binary no longer runs reliably on recent Apple Silicon Macs (macOS has been phasing out Rosetta in some contexts).

Calendarium revives the same minimal use case, rebuilt for Apple Silicon. It was also a chance to write a small native app in Rust and to play with egui.

## Features

- macOS-style calendar icon in the menu bar, showing the current day number.
- The icon follows the menu bar's light/dark appearance (NSImage template mode).
- Click the icon → popup of the current month, anchored under the icon.
- Prev/next month navigation, leading/trailing days from adjacent months shown in grey.
- Translucent, undecorated window that dismisses on outside click or Escape.

## Build & run

Requirements: stable Rust, macOS Apple Silicon.

```sh
# Dev
cargo run

# Release build optimized for size (~2.7 MB)
cargo build --release --no-default-features
```

The binary is at `target/release/calendarium`.

## Logs

Enabled by default. Tune with the standard `env_logger` syntax:
```sh
RUST_LOG=debug cargo run
RUST_LOG=calendarium=info cargo run
```

To produce a release build without logging support:
```sh
cargo build --release --no-default-features
```

## Limitations

- Apple Silicon macOS only (uses font paths under `/System/Library/Fonts/HelveticaNeue.ttc`).
- No localization: month names in English, weekday headers in French (legacy).
- No settings, no context menu.

## License

Licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0).

In short: you are free to use, study, modify and redistribute this software, but any modified version you distribute — including making it available over a network — must also be released under AGPL-3.0, with source code accessible to its users. If you want to use Calendarium under different terms (e.g. embed it in a closed-source product), please open an issue to discuss a commercial license.
