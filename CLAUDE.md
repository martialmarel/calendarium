# calendarium

Menu bar app macOS en Rust/egui.

## Stack
- eframe / egui 0.29
- tray-icon 0.19 (NSStatusBar)
- chrono 0.4

## Build
cargo build --release

## Objectif
Icône calendrier dans la barre macOS avec numéro du jour.
Clic = popup calendrier du mois avec navigation prev/next.

## Contraintes
- Empreinte mémoire minimale (pas de WebView/Tauri)
- LSUIElement = true (pas d'icône Dock)