use chrono::{Datelike, Local};
use eframe::egui;
use log::{debug, info, warn};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

mod calendar;
mod icon;

struct CalendarApp {
    _tray: TrayIcon,
    calendar: calendar::CalendarWidget,
    visible: bool,
    first_frame: bool,
}

impl CalendarApp {
    fn new(tray: TrayIcon) -> Self {
        Self {
            _tray: tray,
            calendar: calendar::CalendarWidget::new(),
            visible: false,
            first_frame: true,
        }
    }

    fn refresh_icon(&self) {
        let day = Local::now().day();
        debug!("refresh_icon: jour = {}", day);
        let rgba = icon::build_icon(day);
        match tray_icon::Icon::from_rgba(rgba, 32, 32) {
            Ok(new_icon) => {
                if let Err(e) = self._tray.set_icon(Some(new_icon)) {
                    warn!("set_icon a échoué: {e}");
                }
            }
            Err(e) => warn!("Icon::from_rgba a échoué: {e}"),
        }
    }
}

impl eframe::App for CalendarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            info!("Première frame: masquage initial de la fenêtre");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Drainer tous les évènements tray accumulés, ne toggle qu'une fois par "Up"
        let mut toggle = false;
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            debug!("Évènement tray: {event:?}");
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                toggle = true;
            }
        }
        if toggle {
            self.visible = !self.visible;
            info!("Toggle fenêtre → visible = {}", self.visible);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
            if self.visible {
                self.refresh_icon();
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        // Fermer avec Escape
        if self.visible && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            info!("Escape pressé → masquage");
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // On dessine toujours le panel, peu importe la visibilité
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            self.calendar.show(ui);
            ui.add_space(8.0);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let day = Local::now().day();
    info!("Démarrage de Calendarium (jour {day})");
    let rgba = icon::build_icon(day);

    let tray_icon =
        tray_icon::Icon::from_rgba(rgba, 32, 32).expect("Impossible de créer l'icône tray");

    let tray = TrayIconBuilder::new()
        .with_icon(tray_icon)
        .with_tooltip("Calendrier")
        .build()
        .expect("Impossible de créer le tray icon");
    info!("Tray icon créée");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([270.0, 300.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_visible(false), // caché au démarrage
        ..Default::default()
    };

    eframe::run_native(
        "CalendarBar",
        options,
        Box::new(|_cc| Ok(Box::new(CalendarApp::new(tray)))),
    )
}
