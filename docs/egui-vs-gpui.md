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

## 6. Étape 4 — click-outside-to-hide

### API utilisée

GPUI n'expose pas d'observer public pour les changements d'activation
de fenêtre (`activation_observers` est `pub(crate)`). Mais
`Window::is_window_active()` est public et lisible depuis n'importe quel
`update()`. On greffe la vérification sur la boucle de polling 50 ms
existante.

### Algorithme

```text
chaque 50 ms:
  drain events tray → traiter (toggle ouvert/fermé)
  si fenêtre ouverte ET ouverte depuis > 300 ms ET !is_window_active():
    fermer la fenêtre
```

Deux détails de robustesse :
1. **Délai de grâce de 300 ms** après ouverture : sans ça, on ferme
   instantanément car macOS n'a pas encore activé la fenêtre fraîchement
   créée.
2. **Debounce de 200 ms** sur le re-clic tray : quand l'utilisateur
   clique sur l'icône tray alors que la fenêtre est ouverte, le clic
   retire d'abord le focus de la fenêtre (→ on ferme via focus loss),
   *puis* l'event tray arrive (→ on ouvrirait une nouvelle fenêtre).
   Sans debounce, on verrait close-puis-réouvre instantané.

### Bilan

Avec ces deux ajouts, le comportement est identique à l'app egui actuelle
côté UX (l'app egui utilise une heuristique similaire :
`frames_since_open > 20 && unfocused_frames > 3`).

Coût : ~30 lignes de plus, aucune dépendance supplémentaire.

---

## 7. Étape 5 — Escape & vibrancy

### Escape

Système d'actions GPUI : `actions!(scope, [Close])` génère un type, on lie
la touche via `cx.bind_keys([KeyBinding::new("escape", Close, None)])`,
et la view réagit avec `.on_action(|_: &Close, window, _cx| window.remove_window())`.

Détail crucial : la view doit avoir un `FocusHandle` actif et appeler
`window.focus(&handle, cx)` à son `new()` — sinon la fenêtre n'a aucun
élément focus et le dispatcher de keys ne route rien.

Le check d'activité du polling a été corrigé : `.unwrap_or(true)` (au lieu
de `false`) pour qu'on nettoie bien le `slot` quand la fenêtre est détruite
en dehors du flux (par Escape par exemple).

### Vibrancy macOS

Trois conditions à réunir :

1. `WindowOptions { window_background: WindowBackgroundAppearance::Blurred, .. }` côté fenêtre — pose un `NSVisualEffectView` derrière le contenu.
2. **Ne PAS** peindre de fond opaque par-dessus dans la view racine.
3. **Override du theme global** — `gpui-component`'s `Root` peint inconditionnellement `.bg(cx.theme().background)` (ligne 504 de `root.rs`). Sans toucher au theme, on bouche tout. Solution : `Theme::global_mut(cx).background = rgba(0xf5f7fa55).into()` juste après `gpui_component::init(cx)`.

Premier essai sans override theme : aucun effet (le fond opaque du Root
masquait le NSVisualEffectView). Le piège est subtil parce que rien n'est
en erreur — c'est juste invisible.

Note : l'override theme côté light affecte aussi d'autres composants qui
utilisent `theme.background`. Pour un bundle complet on voudra définir
un theme custom plutôt que muter le theme par défaut, surtout pour aussi
gérer le mode sombre.

Rendu final : flou natif macOS derrière la fenêtre, texture comparable à
NotificationCenter / Spotlight. Largement supérieur visuellement à notre
fond solide en egui (qui peint un `Color32` translucide sans vrai blur).

---

## 8. Mesures finales

Après avoir câblé l'équivalent fonctionnel de l'app egui (tray, Calendar,
toggle, click-outside, Escape, vibrancy), comparaison équitable.

### Binaire release (`--release --no-default-features`)

| Binaire | Taille |
|---|---|
| `calendarium` (egui — tray + calendar custom + icône dynamique) | **3.9 Mo** |
| `gpui_demo` (GPUI — tray + Calendar gpui-component + theming + vibrancy) | **5.5 Mo** |

Surcoût GPUI : **+1.6 Mo (+40%)**. Loin du blowup redouté à plusieurs
dizaines de Mo. LTO + `opt-level=z` + `strip` font un travail féroce
d'élagage.

### RSS au repos (fenêtre fermée, juste tray vivant)

| Binaire | RSS |
|---|---|
| `calendarium` (egui) | **108 Mo** |
| `gpui_demo` (GPUI) | **44.7 Mo** |

GPUI utilise **2.4× moins de RAM** au repos. Hypothèse : `eframe` initialise
un contexte OpenGL + viewport dès le démarrage, même fenêtre cachée. Notre
code GPUI n'ouvre la fenêtre qu'au clic tray, donc pas de pipeline graphique
alloué tant qu'on n'a pas cliqué.

Mesure non faite : RSS avec fenêtre ouverte. Probablement plus proche.

### Empreinte disque `target/`

- Sans `gpui-experiment` : ~600 Mo
- Avec `gpui-experiment` : **9.9 Go**

C'est le **vrai prix** de GPUI : tirer tout Zed via git, ses dépendances
transitives Wayland/X11/Windows pour cross-platform, plus rust-embed avec
les assets bundlés. Sur disque, c'est ~16× plus volumineux.

### Temps de build

- Cold (cargo fetch + tout compiler) : ~10-20 min
- Chaud (changement de notre code uniquement) : 2-3 s pour `check`, ~50 s
  pour release build complet (LTO sur tout Zed).

### Synthèse honnête

| Critère | Gagnant | Note |
|---|---|---|
| Taille binaire | egui (légèrement) | GPUI à 40% en plus, mais sub-6 Mo |
| RAM au repos | **GPUI** | Surprise positive |
| Disque (`target/`) | **egui** (très clairement) | 16× plus volumineux côté GPUI |
| Temps de compile cold | egui | GPUI = 10+ min la première fois |
| Temps de compile chaud | match nul | GPUI un peu plus lent à cause de LTO |
| Qualité de rendu (texte) | **GPUI** | CoreText sub-pixel vs `ab_glyph` |
| Vibrancy / blur natif | **GPUI** | Vrai NSVisualEffectView vs Color32 |
| API stabilité | **egui** | crates.io vs git-dep qui dérive |
| Maturité macOS menu bar | **egui** | Pattern éprouvé, GPUI inédit |
| Friction de mise en route | egui | GPUI = Xcode + Metal Toolchain requis |

### Pour l'app calendarium

Verdict perso après cette exploration : pour le **scope actuel** (un
calendrier menu bar minimaliste), egui reste suffisant et bien plus
léger à maintenir. **Mais** si l'app évolue vers du contenu plus riche
(animations, événements de calendrier multi-jours, intégration iCal,
themes système clair/sombre auto, search rapide façon Spotlight),
GPUI deviendrait nettement plus intéressant — surtout vu la RAM plus
basse et la vibrancy native.

À garder en réserve, pas à appliquer aveuglément.

---

## 9. Étape 6 — finalisation et optimisations

Quatre optimisations passées en une refonte du `run()` :

### A. Boucle évènementielle (fini le polling 50ms)

`std::sync::mpsc` + `cx.background_executor().timer(50ms)` remplacés par
`flume::unbounded` + `futures::select_biased!` sur trois branches :

```rust
let tray_recv     = rx.recv_async().fuse();
let focus_timer   = cx.background_executor().timer(Duration::from_millis(150)).fuse();
let midnight_timer = cx.background_executor().timer(until_next_midnight()).fuse();
futures::pin_mut!(tray_recv, focus_timer, midnight_timer);
futures::select_biased! {
    evt = tray_recv     => handle_tray_click(...),
    _   = focus_timer   => check_focus_loss(...),
    _   = midnight_timer => refresh_tray_icon(...),
}
```

Effet : zéro wakeup CPU tant qu'il ne se passe rien. Le clic tray
réveille la future immédiatement (latence imperceptible vs 50ms avant).
Le focus check tourne à 150ms (largement assez réactif perçu) parce qu'on
ne peut pas s'abonner à l'event d'activation (API `pub(crate)`).

### B. LSUIElement runtime

Sans bundle `.app`, on appelle directement
`[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory]`
via le crate `objc` (déjà transitif). 10 lignes, l'app disparaît du
Dock et de Cmd-Tab instantanément au démarrage.

### C. Refresh icône à minuit

Branche du `select_biased!` : un timer dont la durée est calculée
dynamiquement (`until_next_midnight()`). À chaque tour de boucle on
recalcule, donc même si la machine sort de veille un changement de jour
sera capté dans la minute.

### D. Dark mode

`window.appearance()` lu à l'ouverture de chaque fenêtre, override de
`Theme.background` avec une teinte adaptée :
- Light : `rgba(0xf5f7fa55)` (presque transparent + tint clair)
- Dark : `rgba(0x1f212455)` (presque transparent + tint sombre)

Comme on re-applique à chaque ouverture, basculer l'apparence système
puis ré-ouvrir donne le bon thème. Pas d'observer continu pendant que la
fenêtre est ouverte (suffisant pour un menu bar dropdown).

### Coût des optimisations

| Métrique | Avant (étape 5) | Après (étape 6) |
|---|---|---|
| Binaire release | 5.5 Mo | **5.5 Mo** |
| RSS au repos | 44.7 Mo | **44.9 Mo** |

Aucune. Les trois nouvelles deps (`flume`, `futures`, `objc`) étaient
déjà toutes transitivement présentes (via gpui-component / Zed / tray-icon),
donc les déclarer comme directes n'ajoute pas une seule ligne de code à
compiler. LTO élague le reste.

### Bilan final du binaire `gpui_demo`

~290 lignes pour reproduire et **dépasser** l'app egui actuelle :
- ✅ tray icon avec chiffre du jour (template macOS auto-tinté)
- ✅ refresh à minuit
- ✅ toggle ouvrir/fermer au clic
- ✅ click-outside-to-hide (avec debounce du re-clic)
- ✅ Escape pour fermer
- ✅ vibrancy macOS native (NSVisualEffectView)
- ✅ dark mode auto
- ✅ pas d'icône Dock (LSUIElement runtime, sans bundle)
- ✅ boucle 100 % évènementielle (zéro wakeup au repos)

À comparer à l'app egui actuelle qui n'a ni vibrancy, ni dark mode auto,
et qui poll 200ms quand visible.

---

## 10. TODO restant (vraiment optionnel)

- [ ] Backing scale factor réel (écrans externes non-2x — Apple Silicon
      a toujours du 2.0 sur l'écran intégré, c'est juste pour les setups
      avec écran externe 1x)
- [ ] Bundle `.app` propre pour distribution (Info.plist, signature)
- [ ] Theme custom au lieu de muter le theme par défaut

---

## 11. Recommandation (opinion)

> Section d'avis perso après l'expérimentation. À pondérer selon les
> ambitions futures de l'app.

**TL;DR : rester sur egui aujourd'hui, garder la branche `feat/try-gpui`
parquée comme option future. La valeur la plus durable de cette
exploration est ce journal.**

### Pourquoi ne pas migrer maintenant

**Le gain visuel est réel mais marginal pour ce scope.** Vibrancy, dark
mode auto, rendu texte CoreText : objectivement supérieurs. Mais pour
un picker de date 270×300 affiché 5 secondes par jour, l'utilisateur
n'en parle plus après une semaine. Calendarium n'est pas Linear ou Sketch.

**Les coûts sont permanents et structurels :**

- 9.9 Go de `target/` vs ~600 Mo (×16). Multiplié par chaque clone, chaque
  CI, chaque contributeur potentiel.
- Cold build 10–20 min, barrière sérieuse à l'onboarding.
- Git-dep sur Zed sans semver : à chaque bump c'est la roulette API. On
  l'a vu en 3 endroits dans cette session (`Root::new` signature,
  `window.focus` arity, theme override).
- Xcode complet + Metal Toolchain comme pré-requis machine (~15 Go).
- L'override `Theme.background` est un workaround fragile : il dépend d'un
  champ qui peut changer de structure à n'importe quelle release de
  gpui-component, sans warning compile.

**Le RSS plus bas (44 vs 108 Mo) est séduisant** mais 60 Mo de RAM sur
un Mac moderne, personne ne le voit en pratique. Pas un critère décisif
pour un menu bar app.

### Quand GPUI deviendrait décisif

- **Scope qui s'élargit** : events iCal multi-jours, scrolling, animations,
  search façon Spotlight → retained mode + Metal payent vraiment.
- **Publication grand public** : l'effet vibrancy fait la différence en
  screenshots et donne une impression "natif macOS" qu'egui ne peut pas
  atteindre.
- **Volonté de suivre l'écosystème Zed** par affinité technique.

### Plan recommandé

1. **Aujourd'hui** : ne merger ni `feat/try-gpui` ni rien, garder la
   branche vivante. Le journal est l'artefact à conserver.
2. **Dans 3-6 mois**, si l'app évolue, re-questionner — la branche est
   à 80% de parité fonctionnelle, prête à reprendre.
3. **Avant toute annonce publique** : reconsidérer sérieusement GPUI,
   pour les screenshots.

### Ce qui reste vrai indépendamment

L'exploration a confirmé que **GPUI est viable techniquement** pour un
menu bar app macOS, malgré son orientation "éditeur Zed". Les 6 pièges
documentés (init order ObjC, lifetime NSStatusItem, coords physiques,
assets, theme override, focus dispatch) sont réels mais surmontables
en quelques heures. Si la décision de migrer arrive un jour, on sait
exactement par où passer.
