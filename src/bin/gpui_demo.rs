// GPUI experiment. Build with:
//   cargo run --bin gpui_demo --features gpui-experiment
//
// Étape 2: afficher le composant Calendar de gpui-component.
// Pas encore de tray, pas encore de translucidité.

#[cfg(not(feature = "gpui-experiment"))]
fn main() {
    eprintln!("Rebuild with --features gpui-experiment");
}

#[cfg(feature = "gpui-experiment")]
mod app {
    use gpui::{
        prelude::*, px, size, App, AppContext, Bounds, Context, Entity, IntoElement, Render,
        Window, WindowBounds, WindowOptions,
    };
    use gpui_component::{
        calendar::{Calendar, CalendarState},
        v_flex, Root,
    };

    pub struct CalendarApp {
        calendar: Entity<CalendarState>,
    }

    impl CalendarApp {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                calendar: cx.new(|cx| CalendarState::new(window, cx)),
            }
        }
    }

    impl Render for CalendarApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .size_full()
                .p_3()
                .child(Calendar::new(&self.calendar))
        }
    }

    pub fn run() {
        gpui_platform::application()
            .with_assets(gpui_component_assets::Assets)
            .run(move |cx: &mut App| {
            gpui_component::init(cx);

            let bounds = Bounds::centered(None, size(px(340.), px(340.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            cx.spawn(async move |cx| {
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| CalendarApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
    }
}

#[cfg(feature = "gpui-experiment")]
fn main() {
    app::run();
}
