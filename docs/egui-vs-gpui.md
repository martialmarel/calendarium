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

## 4. Étape 2 — intégration du composant Calendar

### Code

```rust
let calendar = cx.new(|cx| CalendarState::new(window, cx));
// ...
v_flex().size_full().p_3().child(Calendar::new(&self.calendar))
```

API à deux niveaux : `CalendarState` (entity stateful, possédée par la view)
et `Calendar::new(&state)` (élément éphémère reconstruit à chaque render).
Pattern classique GPUI : on n'instancie pas le widget, on instancie son état.

### Frictions

| Friction | Cause | Résolution |
|---|---|---|
| Colonne samedi tronquée, week 30 coupée | Fenêtre 270×300 trop étroite, les cellules ont un min-width qui pousse au-delà | Fenêtre 340×340 |
| Boutons prev/next **invisibles mais cliquables** | Icônes SVG non chargées : `gpui_platform::application()` n'enregistre aucun asset provider par défaut | Ajouter `gpui-component-assets` et `.with_assets(Assets)` |
| `Bounds::centered` veut `&App`, pas `AsyncApp` | Calculé avant `cx.spawn(...)`, pas dedans | Hoister hors du spawn |

### Le piège des assets

C'est un des points où GPUI/gpui-component est **moins forgiving** qu'egui :
- egui embarque ses glyphes et icônes dans `epaint` → marche out-of-the-box.
- gpui-component charge ses SVG via un `AssetSource` que **l'app doit fournir**.
  Le crate `gpui-component-assets` les bundle via `rust-embed`, mais il faut
  l'ajouter explicitement et appeler `.with_assets(Assets)`.

Symptôme à retenir : **boutons fonctionnels mais invisibles** = assets manquants.

### Comparaison visuelle vs widget egui actuel

| | egui calendar | gpui-component Calendar |
|---|---|---|
| Largeur minimale lisible | ~250 px | ~330 px (plus aéré) |
| Typo | bitmap via `ab_glyph` | CoreText (sous-pixel AA) |
| Apparence | Custom-maison | Design shadcn-like, theming intégré |
| Navigation année | Non | Oui (clic sur année → vue année) |

### Coût après ajout des assets

- Build chaud : +1 crate (`rust-embed` + `gpui-component-assets`), ~23 s.
- Pas re-mesuré en release, à faire après l'étape 3.

---

## 5. Étape 3 — câblage tray-icon → fenêtre GPUI

### Modèle adopté

Pas de show/hide d'une fenêtre persistante (GPUI n'expose pas vraiment cette
API). À la place : **open/close** d'une fenêtre éphémère à chaque toggle.

- Tray créé **dans** le callback `run()`, pas avant (sinon panic objc :
  `Ivar platform not found on class NSApplication` car `tray-icon` initialise
  NSApp avant que GPUI ne pose son ivar).
- `mpsc::channel` pour les events tray, drainé par une `cx.spawn(...)` avec
  `background_executor().timer(50ms)`.
- Sur clic gauche : si une fenêtre existe → `window.remove_window()`, sinon
  on l'ouvre avec `WindowKind::PopUp`, titlebar transparente, traffic lights
  repoussés hors écran.

### Frictions rencontrées

| Friction | Cause | Résolution |
|---|---|---|
| Panic `Ivar platform not found on class NSApplication` au démarrage | Tray créé AVANT GPUI : `tray-icon` initialise NSApp, GPUI veut ensuite poser un ivar `platform` sur la classe → conflit | Créer le tray DANS le callback `run()` |
| Icône tray apparaît puis disparaît instantanément | `let _tray = build_tray()` est une locale du callback → droppé à la fin du callback → NSStatusItem retiré | Déplacer le tray dans la future `cx.spawn(async move)` qui vit jusqu'à la fin de l'app |
| Fenêtre s'ouvre très à droite de l'écran | `tray-icon` retourne le rect en **pixels physiques**, GPUI veut des pixels logiques | Diviser par 2 (Retina). TODO : lire le vrai `backingScaleFactor` pour écrans externes non-2x |

### Pièges fondamentaux (à graver)

1. **Ordre d'initialisation des frameworks ObjC** : sur macOS, GPUI et
   `tray-icon` se battent pour le contrôle de `NSApplication`. GPUI doit
   gagner. Tray-icon doit être créé *après* l'init GPUI.
2. **Durée de vie du `NSStatusItem`** : il disparaît dès que le wrapper
   Rust est droppé. Tout ce qui interagit avec AppKit a une lifetime
   visuelle — pas seulement mémoire.
3. **Coordonnées hétérogènes** : `tray-icon` (physique) ↔ GPUI (logique).
   Faux ami : tout est en `f64`/`Pixels`, mais pas la même unité.

### API GPUI utile découverte

- `WindowKind::PopUp` — fenêtre toujours au-dessus, sans dans la liste Cmd-Tab.
- `TitlebarOptions { appears_transparent: true, traffic_light_position: Some(off-screen) }` — cache la titlebar.
- `Window::remove_window()` — détruit la fenêtre (vs hide).
- `cx.open_window(opts, |window, cx| Root::new(...))` — open async, retourne `WindowHandle`.
- `WindowHandle::update(cx, |root, window, cx| ...)` — opérations sur la fenêtre depuis ailleurs.
- `cx.spawn(async move |cx| ...)` + `background_executor().timer(d).await` — boucle de polling périodique propre.

### Bilan code

`src/bin/gpui_demo.rs` ~180 lignes pour reproduire la base de l'app egui :
- Tray + icône dynamique (réutilise `icon::build_icon` via `#[path]`)
- Toggle visibilité fenêtre
- Position relative au tray

Reste à porter : translucidité, click-outside, Escape, LSUIElement, refresh
icône à minuit, performance hidden-state.

---

## 6. À mesurer / décider ensuite

- [ ] Click-outside-to-hide (sur `WindowKind::PopUp`, voir `on_focus_out` /
      `window.observe_blur` ou détection au niveau NSWindow)
- [ ] Translucidité fenêtre (`WindowBackgroundAppearance::Transparent` côté
      macOS — vérifier si suffisant pour effet vibrancy / blur)
- [ ] Escape pour fermer
- [ ] RSS au repos vs egui (mesure runtime)
- [ ] Empreinte disque `target/` complet
- [ ] Backing scale factor réel (écrans externes non-2x)
- [ ] LSUIElement (pas d'icône Dock) — `bundle` ou Info.plist requis
- [ ] Re-mesurer taille binaire release **avec** toutes les features câblées
