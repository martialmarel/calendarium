// GPUI experiment. Build with:
//   cargo run --bin gpui_demo --features gpui-experiment
//
// Étape 3: tray-icon (avec chiffre du jour) → toggle fenêtre GPUI à
// chaque clic gauche. Fenêtre style popup, sans titlebar, positionnée
// sous l'icône tray. Pas encore de close-on-focus-loss ni translucidité.

#[cfg(not(feature = "gpui-experiment"))]
fn main() {
    eprintln!("Rebuild with --features gpui-experiment");
}

#[cfg(feature = "gpui-experiment")]
#[path = "../icon.rs"]
mod icon;

#[cfg(feature = "gpui-experiment")]
mod app {
    use crate::icon;
    use chrono::{Datelike, Local};
    use gpui::{
        point, prelude::*, px, size, App, AppContext, Bounds, Context, Entity, IntoElement, Point,
        Render, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    };
    use gpui_component::{
        calendar::{Calendar, CalendarState},
        v_flex, Root,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct CalendarView {
        calendar: Entity<CalendarState>,
    }

    impl CalendarView {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                calendar: cx.new(|cx| CalendarState::new(window, cx)),
            }
        }
    }

    impl Render for CalendarView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .size_full()
                .p_3()
                .child(Calendar::new(&self.calendar))
        }
    }

    fn build_tray() -> TrayIcon {
        let day = Local::now().day();
        let rgba = icon::build_icon(day);
        let img = tray_icon::Icon::from_rgba(rgba, icon::ICON_SIZE, icon::ICON_SIZE)
            .expect("Impossible de créer l'icône tray");
        TrayIconBuilder::new()
            .with_icon(img)
            .with_icon_as_template(true)
            .with_tooltip("Calendrier (GPUI)")
            .build()
            .expect("Impossible de créer le tray icon")
    }

    fn open_window(cx: &mut App, anchor: Point<f32>) -> WindowHandle<Root> {
        // Position: largeur 340, centrée sous l'icône.
        let win_w = 340.0_f32;
        let win_h = 340.0_f32;
        let x = (anchor.x - win_w / 2.0).max(4.0);
        let y = anchor.y + 4.0;
        let bounds = Bounds {
            origin: point(px(x), px(y)),
            size: size(px(win_w), px(win_h)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                // Repousser les feux tricolores hors écran (menu bar dropdown style).
                traffic_light_position: Some(point(px(-100.), px(-100.))),
            }),
            kind: WindowKind::PopUp,
            is_resizable: false,
            is_movable: false,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| CalendarView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("Failed to open window")
    }

    pub fn run() {
        gpui_platform::application()
            .with_assets(gpui_component_assets::Assets)
            .run(move |cx: &mut App| {
                gpui_component::init(cx);

                // Tray créé APRÈS l'init GPUI : sinon `tray-icon` initialise
                // NSApplication d'une manière qui entre en conflit avec
                // l'ivar `platform` posé par GPUI (panic objc au démarrage).
                let tray = build_tray();

                let (tx, rx) = mpsc::channel::<TrayIconEvent>();
                TrayIconEvent::set_event_handler(Some(move |evt| {
                    let _ = tx.send(evt);
                }));

                struct ActiveWin {
                    handle: WindowHandle<Root>,
                    opened_at: Instant,
                }

                // État partagé entre la tâche de polling et les updates main thread.
                let active: Rc<RefCell<Option<ActiveWin>>> = Rc::new(RefCell::new(None));
                // Dernier instant de fermeture, pour ignorer le re-click tray qui
                // suit immédiatement une fermeture par perte de focus.
                let last_closed: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));

                // Tâche async qui poll le channel tray toutes les 50 ms ET
                // vérifie la perte de focus de la fenêtre.
                // On *déplace* le tray dans la future pour le garder vivant —
                // sinon il serait droppé à la fin du callback et le NSStatusItem
                // disparaîtrait avant même d'apparaître.
                cx.spawn(async move |cx| {
                    let _tray = tray;
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                        // Drain non-bloquant des évènements tray.
                        let mut events = Vec::new();
                        while let Ok(evt) = rx.try_recv() {
                            events.push(evt);
                        }
                        let active = active.clone();
                        let last_closed = last_closed.clone();
                        let _ = cx.update(move |cx| {
                            // 1. Traiter les clics tray.
                            for evt in events {
                                if let TrayIconEvent::Click {
                                    button: tray_icon::MouseButton::Left,
                                    button_state: tray_icon::MouseButtonState::Up,
                                    rect,
                                    ..
                                } = evt
                                {
                                    let mut slot = active.borrow_mut();
                                    if let Some(a) = slot.take() {
                                        let _ = a.handle.update(cx, |_, window, _| {
                                            window.remove_window();
                                        });
                                        *last_closed.borrow_mut() = Some(Instant::now());
                                    } else {
                                        // Debounce : si on vient de fermer (perte de focus
                                        // ou tray click), un nouveau clic tray est
                                        // probablement la suite du même geste — on l'ignore.
                                        if last_closed
                                            .borrow()
                                            .map_or(false, |t| t.elapsed() < Duration::from_millis(200))
                                        {
                                            continue;
                                        }
                                        // tray-icon retourne des pixels PHYSIQUES sur macOS,
                                        // GPUI veut des pixels logiques. Sur Apple Silicon
                                        // le scale est toujours 2.0 (Retina). TODO: lire le
                                        // vrai backingScaleFactor pour les écrans externes.
                                        let scale = 2.0_f32;
                                        let anchor = Point {
                                            x: (rect.position.x as f32
                                                + rect.size.width as f32 / 2.0)
                                                / scale,
                                            y: (rect.position.y as f32 + rect.size.height as f32)
                                                / scale,
                                        };
                                        *slot = Some(ActiveWin {
                                            handle: open_window(cx, anchor),
                                            opened_at: Instant::now(),
                                        });
                                    }
                                }
                            }

                            // 2. Détecter la perte de focus → fermer.
                            // Délai de grâce de 300 ms après ouverture sinon
                            // on fermerait avant même que macOS ait activé la fenêtre.
                            let mut slot = active.borrow_mut();
                            let should_close = slot
                                .as_ref()
                                .filter(|a| a.opened_at.elapsed() > Duration::from_millis(300))
                                .map(|a| {
                                    a.handle
                                        .update(cx, |_, window, _| window.is_window_active())
                                        .map(|active| !active)
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false);
                            if should_close {
                                if let Some(a) = slot.take() {
                                    let _ = a.handle.update(cx, |_, window, _| {
                                        window.remove_window();
                                    });
                                    *last_closed.borrow_mut() = Some(Instant::now());
                                }
                            }
                        });
                    }
                })
                .detach();
            });
    }
}

#[cfg(feature = "gpui-experiment")]
fn main() {
    app::run();
}
