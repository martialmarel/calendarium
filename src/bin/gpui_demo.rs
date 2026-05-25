// GPUI hello-world experiment. Build with:
//   cargo run --bin gpui_demo --features gpui-experiment
//
// Étape 1: valider que le toolchain GPUI + gpui-component compile et qu'une
// fenêtre s'ouvre. Pas encore de tray, pas encore de Calendar.

#[cfg(not(feature = "gpui-experiment"))]
fn main() {
    eprintln!("Rebuild with --features gpui-experiment");
}

#[cfg(feature = "gpui-experiment")]
mod app {
    use gpui::{
        div, prelude::*, App, AppContext, Context, IntoElement, Render, Window, WindowOptions,
    };
    use gpui_component::Root;

    pub struct HelloWorld;

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child("Calendarium · GPUI experiment")
                .child("Hello, World!")
        }
    }

    pub fn run() {
        gpui_platform::application().run(move |cx: &mut App| {
            gpui_component::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(WindowOptions::default(), |window, cx| {
                    let view = cx.new(|_| HelloWorld);
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
