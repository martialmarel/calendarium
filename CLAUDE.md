# calendarium

Menu bar app macOS en Rust/egui — remplaçant moderne de Day-O.

## Stack
- eframe / egui 0.34 (renderer `glow`, sans `default_fonts` côté icône)
- tray-icon 0.24 (NSStatusBar)
- ab_glyph 0.2 (rendu fonte système Helvetica pour l'icône)
- chrono 0.4
- image 0.25 (resize Lanczos3 pour l'AA de l'icône)

## Build

Dev :
```
cargo build
```

Release optimisée (taille) :
```
cargo build --release --no-default-features
```

Le profil release applique `opt-level="z"`, LTO complet, `codegen-units=1`, `strip` et `panic="abort"`. Feature `logs` (activée par défaut) tirable via `--no-default-features`.

## Architecture

- `src/icon.rs` — génération RGBA de l'icône tray (cadre + anneaux dessinés à la main + chiffre du jour rendu en Helvetica via ab_glyph + downscale Lanczos3). Icône marquée `template` macOS → teintée auto clair/sombre.
- `src/calendar.rs` — widget egui du calendrier (navigation prev/next, jours d'avant/après en gris).
- `src/main.rs` — `eframe::App` :
  - `App::logic` (toujours appelé même viewport invisible) : drainage des tray events, toggle visibilité, focus loss detection.
  - `App::ui` (appelé seulement quand visible) : peint le fond translucide + le widget calendrier.
  - Le tray polling utilise `TrayIconEvent::set_event_handler` (callback sur le main thread macOS) + `mpsc` vers `App::logic`. Aucun thread dédié.

## Notes egui 0.34

- `App::update` est déprécié → split en `App::logic` (toujours) + `App::ui` (seulement si visible).
- `App::ui` ne wrappe plus automatiquement dans un `CentralPanel` → on peint le fond translucide nous-mêmes via `ui.painter().rect_filled`.
- `ui.visuals().panel_fill` ne reflète pas `ctx.set_visuals(...)` côté `App::ui` → on garde la couleur en constante.
- `ViewportCommand::Visible(false)` est OK pour cacher, mais sans `App::logic` indépendant on serait bloqué.

## Contraintes
- macOS Apple Silicon
- Empreinte mémoire minimale (pas de WebView/Tauri)
- LSUIElement = true (pas d'icône Dock)
- Fenêtre translucide : `with_transparent(true)` + `with_has_shadow(false)` + `clear_color = [0,0,0,0]`
