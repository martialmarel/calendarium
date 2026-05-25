use chrono::{Datelike, Local};
use eframe::egui;
use log::{debug, info, warn};
use std::sync::mpsc;
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

mod calendar;
mod icon;

const WIN_W: f32 = 270.0;
const WIN_H: f32 = 300.0;
/// Couleur de fond translucide du popup.
const PANEL_BG: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(245, 247, 250, 240);
/// Rayon des coins arrondis du popup.
const PANEL_RADIUS: f32 = 12.0;

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
    /// Réception des évènements tray ré-émis par le thread de pompage.
    tray_rx: mpsc::Receiver<TrayIconEvent>,
}

impl CalendarApp {
    fn new(tray: TrayIcon, tray_rx: mpsc::Receiver<TrayIconEvent>) -> Self {
        Self {
            _tray: tray,
            calendar: calendar::CalendarWidget::new(),
            visible: false,
            first_frame: true,
            unfocused_frames: 0,
            frames_since_open: 0,
            tray_rx,
        }
    }

    fn refresh_icon(&self) {
        let day = Local::now().day();
        debug!("refresh_icon: jour = {}", day);
        let rgba = icon::build_icon(day);
        match tray_icon::Icon::from_rgba(rgba, icon::ICON_SIZE, icon::ICON_SIZE) {
            Ok(new_icon) => {
                if let Err(e) = self._tray.set_icon_with_as_template(Some(new_icon), true) {
                    warn!("set_icon a échoué: {e}");
                }
            }
            Err(e) => warn!("Icon::from_rgba a échoué: {e}"),
        }
    }
}

impl eframe::App for CalendarApp {
    /// Fond de framebuffer transparent : la translucidité du panel se compose
    /// alors directement sur le bureau.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Logique non-UI : toujours appelée par eframe 0.34, même quand la fenêtre
    /// est `Visible(false)`. C'est ici qu'on traite les évènements tray pour
    /// pouvoir rouvrir la fenêtre après fermeture.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            info!("Première frame");
            // Masque la fenêtre au démarrage proprement maintenant que `logic`
            // est appelé indépendamment de la visibilité.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Drainer les évènements tray re-émis par le thread de pompage.
        let mut toggle_to: Option<egui::Pos2> = None;
        let ppp = ctx.pixels_per_point().max(1.0);
        while let Ok(event) = self.tray_rx.try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                rect,
                ..
            } = event
            {
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
            if self.visible {
                self.refresh_icon();
                self.unfocused_frames = 0;
                self.frames_since_open = 0;
                info!("Position cible: ({:.0}, {:.0})", pos.x, pos.y);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WIN_W, WIN_H)));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Fermer avec Escape
        if self.visible && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            info!("Escape pressé → masquage");
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Fermer si la fenêtre perd le focus (clic en dehors).
        if self.visible {
            self.frames_since_open = self.frames_since_open.saturating_add(1);
            let focused = ctx.input(|i| i.focused);
            if focused {
                self.unfocused_frames = 0;
            } else {
                self.unfocused_frames = self.unfocused_frames.saturating_add(1);
            }
            if self.frames_since_open > 20 && self.unfocused_frames > 3 {
                info!("Perte du focus stable → masquage");
                self.visible = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    /// Rendu UI — eframe 0.34 ne l'appelle que quand la fenêtre est visible.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Peint le fond translucide nous-mêmes : en 0.34 le framework ne wrappe
        // plus `App::ui` dans un CentralPanel utilisant `panel_fill`.
        ui.painter()
            .rect_filled(ui.max_rect(), PANEL_RADIUS, PANEL_BG);

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                self.calendar.show(ui);
            });
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(feature = "logs")]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let day = Local::now().day();
    info!("Démarrage de Calendarium (jour {day})");
    let rgba = icon::build_icon(day);

    let tray_icon = tray_icon::Icon::from_rgba(rgba, icon::ICON_SIZE, icon::ICON_SIZE)
        .expect("Impossible de créer l'icône tray");

    let tray = TrayIconBuilder::new()
        .with_icon(tray_icon)
        .with_icon_as_template(true) // macOS : laisse le système teinter clair/sombre
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
            .with_transparent(true)
            // macOS : pas d'ombre NSWindow par-dessus le contenu translucide.
            .with_has_shadow(false)
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native(
        "CalendarBar",
        options,
        Box::new(|cc| {
            // Pompage des évènements tray dans un thread dédié : on les ré-émet
            // dans notre channel et on réveille le runloop egui après chaque
            // évènement (essentiel pour rouvrir la fenêtre après Visible(false)).
            let (tx, rx) = mpsc::channel::<TrayIconEvent>();
            let ctx_clone = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let tray_rx = TrayIconEvent::receiver();
                loop {
                    match tray_rx.recv() {
                        Ok(event) => {
                            debug!("[tray-thread] event: {event:?}");
                            if tx.send(event).is_err() {
                                break;
                            }
                            ctx_clone.request_repaint();
                        }
                        Err(_) => break,
                    }
                }
            });

            Ok(Box::new(CalendarApp::new(tray, rx)))
        }),
    )
}
