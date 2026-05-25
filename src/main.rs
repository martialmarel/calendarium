use chrono::{Datelike, Local};
use eframe::egui;
use log::{debug, info, warn};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

mod calendar;
mod icon;

const WIN_W: f32 = 270.0;
const WIN_H: f32 = 300.0;

struct CalendarApp {
    _tray: TrayIcon,
    calendar: calendar::CalendarWidget,
    visible: bool,
    first_frame: bool,
    /// Frames consécutives sans focus depuis la dernière ouverture.
    /// On masque quand on dépasse un seuil — laisse à la fenêtre le temps
    /// d'acquérir le focus, et tolère un flicker macOS.
    unfocused_frames: u32,
    /// Frames écoulées depuis la dernière ouverture (grâce period).
    frames_since_open: u32,
}

impl CalendarApp {
    fn new(tray: TrayIcon) -> Self {
        Self {
            _tray: tray,
            calendar: calendar::CalendarWidget::new(),
            visible: false,
            first_frame: true,
            unfocused_frames: 0,
            frames_since_open: 0,
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
        let mut toggle_to: Option<egui::Pos2> = None;
        let ppp = ctx.pixels_per_point().max(1.0);
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click { .. } | TrayIconEvent::Enter { .. } | TrayIconEvent::Leave { .. }
            ) {
                debug!("Évènement tray: {event:?}");
            }
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                // rect est en pixels physiques → convertir en points logiques pour egui
                let icon_center_x_phys = rect.position.x as f32 + rect.size.width as f32 / 2.0;
                let icon_bottom_y_phys = rect.position.y as f32 + rect.size.height as f32;
                let icon_center_x = icon_center_x_phys / ppp;
                let icon_bottom_y = icon_bottom_y_phys / ppp;
                let x = (icon_center_x - WIN_W / 2.0).max(4.0);
                let y = icon_bottom_y + 4.0;
                toggle_to = Some(egui::pos2(x, y));
            }
        }
        if let Some(pos) = toggle_to {
            self.visible = !self.visible;
            info!("Toggle fenêtre → visible = {}", self.visible);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
            if self.visible {
                self.refresh_icon();
                self.unfocused_frames = 0;
                self.frames_since_open = 0;
                info!("Position cible: ({:.0}, {:.0})", pos.x, pos.y);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        // Fermer avec Escape
        if self.visible && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            info!("Escape pressé → masquage");
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Fermer si la fenêtre perd le focus (clic en dehors).
        // - Grâce de 20 frames après l'ouverture (le temps que macOS attribue le focus).
        // - Il faut rester non-focus pendant 3 frames consécutives pour tolérer
        //   les micro-pertes de focus liées au tray.
        if self.visible {
            self.frames_since_open = self.frames_since_open.saturating_add(1);
            let focused = ctx.input(|i| i.focused);
            if focused {
                self.unfocused_frames = 0;
            } else {
                self.unfocused_frames = self.unfocused_frames.saturating_add(1);
            }
            debug!(
                "focus={} unfocused_frames={} frames_since_open={}",
                focused, self.unfocused_frames, self.frames_since_open
            );
            if self.frames_since_open > 20 && self.unfocused_frames > 3 {
                info!("Perte du focus stable → masquage");
                self.visible = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
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
            .with_inner_size([WIN_W, WIN_H])
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
