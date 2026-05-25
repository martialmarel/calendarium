use chrono::{Datelike, Local, NaiveDate};
use egui::{Color32, Grid, RichText, Ui};

pub struct CalendarWidget {
    pub displayed_month: NaiveDate, // toujours le 1er du mois
    today: NaiveDate,
}

impl CalendarWidget {
    pub fn new() -> Self {
        let today = Local::now().date_naive();
        Self {
            displayed_month: today.with_day(1).unwrap(),
            today,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // Header navigation
        ui.horizontal(|ui| {
            if ui.small_button("◀").clicked() {
                self.shift_month(-1);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("▶").clicked() {
                    self.shift_month(1);
                }
                ui.centered_and_justified(|ui| {
                    let header = self.displayed_month.format("%B %Y").to_string();
                    ui.label(RichText::new(header).strong().size(14.0));
                });
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(2.0);

        // En-têtes jours (lundi → dimanche)
        let day_names = ["Lu", "Ma", "Me", "Je", "Ve", "Sa", "Di"];
        ui.horizontal(|ui| {
            for (i, name) in day_names.iter().enumerate() {
                let color = if i >= 5 {
                    Color32::from_rgb(180, 80, 80)
                } else {
                    Color32::GRAY
                };
                ui.add_sized(
                    [34.0, 20.0],
                    egui::Label::new(RichText::new(*name).small().color(color)),
                );
            }
        });

        // Grille des jours
        let first_weekday = self.displayed_month.weekday().num_days_from_monday() as usize;
        let days_count = self.days_in_month();
        let prev_days_count = self.days_in_prev_month();

        let accent = Color32::from_rgb(0, 100, 220);
        let muted = Color32::from_rgb(170, 170, 170);
        let muted_weekend = Color32::from_rgb(210, 160, 160);

        Grid::new("cal_grid")
            .num_columns(7)
            .min_col_width(34.0)
            .min_row_height(32.0)
            .spacing([0.0, 2.0])
            .show(ui, |ui| {
                let total = first_weekday + days_count;
                let rows = (total + 6) / 7;

                for row in 0..rows {
                    for col in 0..7 {
                        let cell = row * 7 + col;
                        let is_weekend = col >= 5;

                        let (display_day, in_current_month) = if cell < first_weekday {
                            // Fin du mois précédent
                            let d = prev_days_count - (first_weekday - cell) + 1;
                            (d, false)
                        } else if cell - first_weekday < days_count {
                            (cell - first_weekday + 1, true)
                        } else {
                            // Début du mois suivant
                            (cell - first_weekday - days_count + 1, false)
                        };

                        let is_today = in_current_month && {
                            let d = NaiveDate::from_ymd_opt(
                                self.displayed_month.year(),
                                self.displayed_month.month(),
                                display_day as u32,
                            );
                            d == Some(self.today)
                        };

                        let text = RichText::new(display_day.to_string()).size(13.0);
                        let text = if is_today {
                            text.strong().color(Color32::WHITE).background_color(accent)
                        } else if !in_current_month {
                            text.color(if is_weekend { muted_weekend } else { muted })
                        } else if is_weekend {
                            text.color(Color32::from_rgb(200, 70, 70))
                        } else {
                            text
                        };

                        ui.add_sized([34.0, 32.0], egui::Label::new(text));
                    }
                    ui.end_row();
                }
            });
    }

    fn days_in_prev_month(&self) -> usize {
        let (y, m) = (self.displayed_month.year(), self.displayed_month.month());
        let first_prev = if m == 1 {
            NaiveDate::from_ymd_opt(y - 1, 12, 1)
        } else {
            NaiveDate::from_ymd_opt(y, m - 1, 1)
        }
        .unwrap();
        self.displayed_month
            .signed_duration_since(first_prev)
            .num_days() as usize
    }

    fn shift_month(&mut self, delta: i32) {
        let m = self.displayed_month.month() as i32 + delta;
        let y = self.displayed_month.year();
        let (new_y, new_m) = if m < 1 {
            (y - 1, 12)
        } else if m > 12 {
            (y + 1, 1)
        } else {
            (y, m as u32)
        };
        self.displayed_month = NaiveDate::from_ymd_opt(new_y, new_m as u32, 1).unwrap();
    }

    fn days_in_month(&self) -> usize {
        let (y, m) = (self.displayed_month.year(), self.displayed_month.month());
        let next = if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1)
        };
        next.unwrap()
            .signed_duration_since(self.displayed_month)
            .num_days() as usize
    }
}
