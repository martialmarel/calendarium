// GPUI experiment. Build with:
//   cargo run --bin gpui_demo --features gpui-experiment
//
// Version finalisée — toutes les features de l'app egui + bonus :
//   - tray-icon avec chiffre du jour, refresh à minuit
//   - popup vibrancy macOS, theming light/dark
//   - toggle / click-outside / Escape pour fermer
//   - LSUIElement runtime (pas d'icône Dock)
//   - boucle évènementielle (pas de polling)

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
    use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveTime};
    use futures::FutureExt;
    use gpui::{
        actions, point, prelude::*, px, rgba, size, App, AppContext, Bounds, Context, Entity,
        FocusHandle, Focusable, IntoElement, KeyBinding, Point, Render, TitlebarOptions, Window,
        WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
        WindowOptions,
    };
    use gpui_component::{
        calendar::{Calendar, CalendarState},
        v_flex, Root, Theme,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

    actions!(calendarium, [Close]);

    pub struct CalendarView {
        calendar: Entity<CalendarState>,
        focus_handle: FocusHandle,
    }

    impl CalendarView {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            // Applique le theme adapté à l'apparence actuelle de la fenêtre.
            apply_theme_for(cx, window.appearance());
            Self {
                calendar: cx.new(|cx| CalendarState::new(window, cx)),
                focus_handle,
            }
        }
    }

    impl Focusable for CalendarView {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for CalendarView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .track_focus(&self.focus_handle)
                .key_context("CalendarView")
                .on_action(|_: &Close, window, _cx| {
                    window.remove_window();
                })
                .size_full()
                .p_3()
                .child(Calendar::new(&self.calendar))
        }
    }

    /// Pose le fond translucide adapté à l'apparence (light/dark) — la teinte
    /// doit rester très transparente pour ne pas masquer le NSVisualEffectView.
    fn apply_theme_for(cx: &mut App, appearance: WindowAppearance) {
        let theme = Theme::global_mut(cx);
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => {
                theme.background = rgba(0xf5f7fa55).into();
            }
            WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                theme.background = rgba(0x1f212455).into();
            }
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

    fn refresh_tray_icon(tray: &TrayIcon) {
        let day = Local::now().day();
        let rgba = icon::build_icon(day);
        if let Ok(img) = tray_icon::Icon::from_rgba(rgba, icon::ICON_SIZE, icon::ICON_SIZE) {
            let _ = tray.set_icon_with_as_template(Some(img), true);
        }
    }

    /// Durée jusqu'à minuit local (avec un fallback à 1h en cas d'erreur).
    fn until_next_midnight() -> Duration {
        let now = Local::now();
        let tomorrow = (now.date_naive() + ChronoDuration::days(1)).and_time(NaiveTime::MIN);
        (tomorrow - now.naive_local())
            .to_std()
            .unwrap_or(Duration::from_secs(3600))
    }

    /// `[NSApplication setActivationPolicy: NSApplicationActivationPolicyAccessory]`.
    /// Équivalent runtime du `LSUIElement = true` dans Info.plist : pas d'icône
    /// dans le Dock, l'app disparaît de Cmd-Tab.
    fn set_accessory_activation_policy() {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};
        unsafe {
            if let Some(cls) = Class::get("NSApplication") {
                let app: *mut Object = msg_send![cls, sharedApplication];
                if !app.is_null() {
                    // NSApplicationActivationPolicyAccessory = 1
                    let _: () = msg_send![app, setActivationPolicy: 1i64];
                }
            }
        }
    }

    fn open_window(cx: &mut App, anchor: Point<f32>) -> WindowHandle<Root> {
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
                traffic_light_position: Some(point(px(-100.), px(-100.))),
            }),
            kind: WindowKind::PopUp,
            is_resizable: false,
            is_movable: false,
            window_background: WindowBackgroundAppearance::Blurred,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| CalendarView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("Failed to open window")
    }

    struct ActiveWin {
        handle: WindowHandle<Root>,
        opened_at: Instant,
    }

    fn handle_tray_click(
        rect: tray_icon::Rect,
        active: &Rc<RefCell<Option<ActiveWin>>>,
        last_closed: &Rc<RefCell<Option<Instant>>>,
        cx: &mut App,
    ) {
        let mut slot = active.borrow_mut();
        if let Some(a) = slot.take() {
            let _ = a.handle.update(cx, |_, window, _| window.remove_window());
            *last_closed.borrow_mut() = Some(Instant::now());
            return;
        }
        // Debounce : si on vient de fermer (perte de focus), le clic tray
        // qui a *causé* la perte de focus n'est qu'un artefact du même geste.
        if last_closed
            .borrow()
            .map_or(false, |t| t.elapsed() < Duration::from_millis(200))
        {
            return;
        }
        // tray-icon retourne des pixels PHYSIQUES sur macOS, GPUI des logiques.
        // Apple Silicon = Retina 2.0. TODO : backingScaleFactor pour écrans externes.
        let scale = 2.0_f32;
        let anchor = Point {
            x: (rect.position.x as f32 + rect.size.width as f32 / 2.0) / scale,
            y: (rect.position.y as f32 + rect.size.height as f32) / scale,
        };
        *slot = Some(ActiveWin {
            handle: open_window(cx, anchor),
            opened_at: Instant::now(),
        });
    }

    fn check_focus_loss(
        active: &Rc<RefCell<Option<ActiveWin>>>,
        last_closed: &Rc<RefCell<Option<Instant>>>,
        cx: &mut App,
    ) {
        let mut slot = active.borrow_mut();
        let should_close = slot
            .as_ref()
            // Délai de grâce : macOS active la fenêtre ~quelques frames après l'open.
            .filter(|a| a.opened_at.elapsed() > Duration::from_millis(300))
            .map(|a| {
                a.handle
                    .update(cx, |_, window, _| window.is_window_active())
                    .map(|active| !active)
                    // Si la fenêtre a été détruite ailleurs (Escape, etc.),
                    // l'update échoue → on traite comme inactive pour nettoyer le slot.
                    .unwrap_or(true)
            })
            .unwrap_or(false);
        if should_close {
            if let Some(a) = slot.take() {
                let _ = a.handle.update(cx, |_, window, _| window.remove_window());
                *last_closed.borrow_mut() = Some(Instant::now());
            }
        }
    }

    pub fn run() {
        gpui_platform::application()
            .with_assets(gpui_component_assets::Assets)
            .run(move |cx: &mut App| {
                gpui_component::init(cx);
                cx.bind_keys([KeyBinding::new("escape", Close, None)]);
                set_accessory_activation_policy();

                let tray = Rc::new(build_tray());

                // Channel async + sync : send synchrone depuis le callback OS,
                // recv async dans la future GPUI. Plus de polling 50ms.
                let (tx, rx) = flume::unbounded::<TrayIconEvent>();
                TrayIconEvent::set_event_handler(Some(move |evt| {
                    let _ = tx.send(evt);
                }));

                let active: Rc<RefCell<Option<ActiveWin>>> = Rc::new(RefCell::new(None));
                let last_closed: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));

                // Boucle évènementielle : on attend (sans CPU wakeup) le premier
                // des trois events suivants — tray click, tick de focus check,
                // ou minuit pour rafraîchir l'icône.
                cx.spawn(async move |cx| {
                    let tray = tray; // ownership : keeps NSStatusItem alive.
                    loop {
                        let tray_recv = rx.recv_async().fuse();
                        let focus_timer = cx
                            .background_executor()
                            .timer(Duration::from_millis(150))
                            .fuse();
                        let midnight_timer = cx
                            .background_executor()
                            .timer(until_next_midnight())
                            .fuse();
                        futures::pin_mut!(tray_recv, focus_timer, midnight_timer);

                        futures::select_biased! {
                            evt = tray_recv => {
                                if let Ok(TrayIconEvent::Click {
                                    button: tray_icon::MouseButton::Left,
                                    button_state: tray_icon::MouseButtonState::Up,
                                    rect, ..
                                }) = evt {
                                    let active = active.clone();
                                    let last_closed = last_closed.clone();
                                    let _ = cx.update(move |cx| {
                                        handle_tray_click(rect, &active, &last_closed, cx);
                                    });
                                }
                            }
                            _ = focus_timer => {
                                let active = active.clone();
                                let last_closed = last_closed.clone();
                                let _ = cx.update(move |cx| {
                                    check_focus_loss(&active, &last_closed, cx);
                                });
                            }
                            _ = midnight_timer => {
                                refresh_tray_icon(&tray);
                            }
                        }
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
