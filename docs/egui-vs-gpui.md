# egui vs GPUI — journal de migration

Document de travail comparant la stack actuelle (egui 0.34 + eframe) avec
une exploration de GPUI (Zed Industries) + gpui-component (Longbridge).
Mis à jour au fil de l'expérimentation sur la branche `feat/try-gpui`.

---

## 1. Réflexions a priori (avant tout code)

### Nature des deux frameworks

| | **egui** | **GPUI** |
|---|---|---|
| Modèle | Immediate mode | Retained, déclaratif (entities + observers) |
| Renderer | OpenGL (`glow`), wgpu en option | Metal natif (macOS), Vulkan/DX en cours |
| Layout | Manuel + helpers | Flexbox via `taffy` (API à la Tailwind) |
| Texte | `ab_glyph` / `epaint` | CoreText + sous-pixel AA sur macOS |
| Async | Pas intégré | Executor natif (`cx.spawn`, background pool) |
| Distribution | crates.io stable | **Git-only**, API qui dérive |
| Maturité macOS menu bar | Bonne (notre app marche) | Inconnue — Zed n'a pas ce pattern |

### gpui-component (Longbridge)

- Apache-2.0, ~11k★, maintenu activement.
- Inspiré shadcn/ui, theming clair/sombre cohérent.
- 4 familles : Basic (24), Form (8), Layout (9), Advanced (9).
- Pour calendarium, les trois composants utiles sont **Calendar**, **DatePicker**, **Popover**.

### Points d'attention identifiés a priori

1. **Dépendance git** → pinner sur un commit pour éviter les régressions silencieuses.
2. **Empreinte mémoire** → menu bar app, donc à mesurer (RSS au repos, binaire).
3. **Tray icon** → GPUI ne gère pas NSStatusBar. On garde forcément `tray-icon`
   et on pilote la fenêtre GPUI depuis son callback (main thread macOS).
4. **Fenêtre translucide + sans ombre + click-outside-to-hide** → API
   `WindowOptions` de GPUI inconnue, à valider.

### Hypothèse initiale

> Pour un menu bar app minimaliste, egui reste plus pragmatique.
> GPUI brillerait s'il y avait des animations riches, du texte custom,
> ou du contenu lourd à intégrer.

À confirmer ou infirmer par mesures concrètes.

---

## 2. Première mise en place — scaffolding

### Stratégie

- **Ne pas casser l'app egui**. Ajout d'un second binaire `gpui_demo` derrière
  une feature `gpui-experiment` (opt-in).
- Étape 1 : Hello-World GPUI qui ouvre une fenêtre — valider la chaîne de build.
- Étape 2 (à venir) : intégrer le Calendar de gpui-component.
- Étape 3 (à venir) : brancher sur `tray-icon`, translucidité, click-outside.

### Dépendances ajoutées

```toml
gpui = { git = "https://github.com/zed-industries/zed", optional = true }
gpui_platform = { git = "https://github.com/zed-industries/zed",
                  features = ["font-kit"], optional = true }
gpui-component = { git = "https://github.com/longbridge/gpui-component",
                   optional = true }

[patch.crates-io]
psm = { git = "https://github.com/rust-lang/stacker", branch = "master" }
```

Le patch `psm` est requis par Zed sur Apple Silicon (stacker version récente).

### Frictions rencontrées

| Friction | Cause | Résolution |
|---|---|---|
| Conflit `core-foundation` 0.10.0 vs 0.10.1 | `gpui` épingle `=0.10.0`, lockfile à 0.10.1 (via tray-icon) | `cargo update` global |
| Conflit `toml` (cbindgen vs gtk-sys) | `tray-icon` tire toute la chaîne Linux `libappindicator → gtk-sys → system-deps → toml 0.8.2`, incompatible avec `cbindgen 0.28` qui veut `^0.8.8` | Idem, `cargo update` global |
| `xcrun: unable to find utility "metal"` | `xcode-select` pointait vers Command Line Tools, pas Xcode complet | `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer` |
| `cannot execute tool 'metal' due to missing Metal Toolchain` | Xcode 26 : Metal Toolchain devient un composant téléchargeable séparé | `xcodebuild -downloadComponent MetalToolchain` (~1-2 Go) |
| `cannot satisfy _: Into<AnyView>` | API drift : `Root::new` accepte directement `Entity<V>`, le `.into()` du README est obsolète | Retirer le `.into()` |

### Coût d'entrée objectif

- **Première compile à froid** : plusieurs minutes (téléchargement du repo Zed
  entier + compilation des shaders Metal + ~500 crates transitives).
- **Pré-requis machine** : Xcode complet (~10 Go) + Metal Toolchain.
  Les Command Line Tools seules ne suffisent pas.

### Code minimal qui fonctionne

`src/bin/gpui_demo.rs` (~50 lignes) : `gpui_platform::application().run(...)`
qui ouvre une fenêtre via `cx.open_window`, wrappe une view `HelloWorld`
dans `Root::new` (composant racine requis par gpui-component pour son theming).

---

## 3. Mesures à la première compilation réussie

### Binaire — comparaison release (`--release --no-default-features`)

| Binaire | Contenu | Taille | Temps build (chaud) |
|---|---|---|---|
| `calendarium` (egui) | Calendar complet + tray + icône custom + chiffre dynamique | **3.9 Mo** | 2 min 09 |
| `gpui_demo` (GPUI) | Hello-World, pas de Calendar, pas de tray | **3.1 Mo** | 1 min 40 |

Profil release : `opt-level = "z"`, `lto = true`, `codegen-units = 1`,
`strip = "symbols"`, `panic = "abort"`.

### Lecture honnête

- Comparaison **pas équitable** : `gpui_demo` n'embarque pas encore Calendar,
  tray-icon, fenêtre translucide, etc. La taille va monter avec l'étape 2.
- LTO + `opt-level=z` font un élagage agressif : tout ce qui n'est pas
  utilisé de gpui-component disparaît.
- Le préjugé "GPUI = trop lourd pour menu bar" **ne tient pas** sur cette
  première mesure release. Le coût réel est ailleurs (build time à froid,
  taille du `target/`, RAM runtime à mesurer).

### Binaire debug (à titre indicatif)

`gpui_demo` debug = **62 Mo** (avec symboles).

### Runtime — premier lancement de fenêtre

- Fenêtre s'ouvre, rendu Metal fonctionne, gpui-component s'initialise.
- ⚠️ Pas de gestion de fermeture par défaut : `WindowOptions::default()`
  n'enregistre aucun handler, donc clic croix rouge inactif, Cmd-Q inactif
  (pas de menu). À câbler manuellement avec `cx.quit()` sur un keystroke
  ou via le menu.

---

## 4. À mesurer / décider ensuite

- [ ] Étape 2 — intégrer Calendar gpui-component, mesurer impact taille
- [ ] Étape 3 — câblage `tray-icon` → fenêtre GPUI (main thread macOS)
- [ ] Translucidité fenêtre (équivalent `with_transparent(true)` + `with_has_shadow(false)`)
- [ ] Click-outside-to-hide (équivalent de notre focus loss detection egui)
- [ ] RSS au repos (process backgroundé, fenêtre cachée)
- [ ] Empreinte disque `target/` complet
- [ ] Cmd-Q / menu / handler de fermeture
- [ ] LSUIElement (pas d'icône Dock) — vérifier comportement GPUI
