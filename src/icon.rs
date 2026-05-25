//! Génération de l'icône tray (bitmap RGBA).
//!
//! Cadre arrondi + anneaux de reliure dessinés à la main, et chiffre(s) du
//! jour rendus via la fonte système macOS (Helvetica). On dessine à
//! SUPER_SCALE× puis on downscale en Lanczos3 pour antialiaser proprement
//! les courbes du cadre.

use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
use image::{imageops::FilterType, ImageBuffer, Rgba};
use log::warn;
use std::sync::OnceLock;

pub const ICON_SIZE: u32 = 44;
const SUPER_SCALE: u32 = 4;
const RENDER_SIZE: u32 = ICON_SIZE * SUPER_SCALE;

/// Chemins (face, index_dans_TTC) testés dans l'ordre.
const FONT_CANDIDATES: &[(&str, u32)] = &[
    ("/System/Library/Fonts/HelveticaNeue.ttc", 1), // Bold
    ("/System/Library/Fonts/Helvetica.ttc", 1),     // Bold
    ("/System/Library/Fonts/HelveticaNeue.ttc", 0), // Regular
    ("/System/Library/Fonts/Helvetica.ttc", 0),     // Regular
];

fn load_font() -> Option<FontVec> {
    for (path, idx) in FONT_CANDIDATES {
        match std::fs::read(path) {
            Ok(data) => match FontVec::try_from_vec_and_index(data, *idx) {
                Ok(font) => {
                    log::info!("Fonte chargée: {path} (index {idx})");
                    return Some(font);
                }
                Err(e) => warn!("Échec parse {path} idx={idx}: {e}"),
            },
            Err(e) => warn!("Lecture {path} impossible: {e}"),
        }
    }
    None
}

fn font() -> Option<&'static FontVec> {
    static FONT: OnceLock<Option<FontVec>> = OnceLock::new();
    FONT.get_or_init(load_font).as_ref()
}

fn blend_px(pixels: &mut [u8], x: i32, y: i32, color: [u8; 3], coverage: f32) {
    let s = RENDER_SIZE as i32;
    if x < 0 || y < 0 || x >= s || y >= s {
        return;
    }
    let idx = (y as usize * RENDER_SIZE as usize + x as usize) * 4;
    let new_alpha = (coverage.clamp(0.0, 1.0) * 255.0) as u8;
    // Si un pixel a déjà été posé (cadre), on garde le maximum d'opacité.
    if new_alpha > pixels[idx + 3] {
        pixels[idx] = color[0];
        pixels[idx + 1] = color[1];
        pixels[idx + 2] = color[2];
        pixels[idx + 3] = new_alpha;
    }
}

fn set_px(pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    let s = RENDER_SIZE as i32;
    if x < 0 || y < 0 || x >= s || y >= s {
        return;
    }
    let idx = (y as usize * RENDER_SIZE as usize + x as usize) * 4;
    pixels[idx] = color[0];
    pixels[idx + 1] = color[1];
    pixels[idx + 2] = color[2];
    pixels[idx + 3] = color[3];
}

/// Capsule verticale centrée en (cx, cy), demi-largeur half_w, demi-hauteur half_h.
/// = rectangle central + deux demi-disques aux extrémités.
fn fill_capsule_v(
    pixels: &mut [u8],
    cx: i32,
    cy: i32,
    half_w: i32,
    half_h: i32,
    color: [u8; 4],
) {
    let body_h = half_h - half_w;
    if body_h > 0 {
        fill_rect(pixels, cx - half_w, cy - body_h, cx + half_w, cy + body_h, color);
    }
    fill_disc(pixels, cx, cy - body_h.max(0), half_w, color);
    fill_disc(pixels, cx, cy + body_h.max(0), half_w, color);
}

fn fill_disc(pixels: &mut [u8], cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r * r {
                set_px(pixels, x, y, color);
            }
        }
    }
}

fn fill_rect(pixels: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            set_px(pixels, x, y, color);
        }
    }
}

fn draw_frame(pixels: &mut [u8], color: [u8; 4]) {
    let margin = 1 * SUPER_SCALE as i32; // 1 px de marge externe
    let top_offset = 2 * SUPER_SCALE as i32;
    let radius = 5 * SUPER_SCALE as i32;
    let thickness = 2 * SUPER_SCALE as i32; // 2 px après downscale

    let x0 = margin;
    let y0 = margin + top_offset;
    let x1 = RENDER_SIZE as i32 - margin - 1;
    let y1 = RENDER_SIZE as i32 - margin - 1;

    fill_rect(pixels, x0 + radius, y0, x1 - radius, y0 + thickness - 1, color);
    fill_rect(pixels, x0 + radius, y1 - thickness + 1, x1 - radius, y1, color);
    fill_rect(pixels, x0, y0 + radius, x0 + thickness - 1, y1 - radius, color);
    fill_rect(pixels, x1 - thickness + 1, y0 + radius, x1, y1 - radius, color);

    let corners = [
        (x0 + radius, y0 + radius, -1, -1),
        (x1 - radius, y0 + radius, 1, -1),
        (x0 + radius, y1 - radius, -1, 1),
        (x1 - radius, y1 - radius, 1, 1),
    ];
    let r_outer_sq = radius * radius;
    let r_inner = radius - thickness;
    let r_inner_sq = r_inner * r_inner;
    for &(cx, cy, sx, sy) in &corners {
        for y in 0..=radius {
            for x in 0..=radius {
                let d2 = x * x + y * y;
                if d2 <= r_outer_sq && d2 > r_inner_sq {
                    set_px(pixels, cx + sx * x, cy + sy * y, color);
                }
            }
        }
    }
}

/// Dessine le texte centré dans la zone (cx_min..cx_max, cy_min..cy_max).
fn draw_text_centered(
    pixels: &mut [u8],
    font: &FontVec,
    text: &str,
    cx_min: i32,
    cy_min: i32,
    cx_max: i32,
    cy_max: i32,
    color: [u8; 3],
) {
    let target_h = (cy_max - cy_min) as f32;
    let scale = PxScale::from(target_h * 1.15);
    let scaled = font.as_scaled(scale);

    // Mesure : largeur totale + bornes verticales (sommet/baseline).
    let mut total_w = 0.0f32;
    let mut last_glyph_id = None;
    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(prev) = last_glyph_id {
            total_w += scaled.kern(prev, gid);
        }
        total_w += scaled.h_advance(gid);
        last_glyph_id = Some(gid);
    }

    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let text_h = ascent - descent;

    let zone_w = (cx_max - cx_min) as f32;
    let zone_h = (cy_max - cy_min) as f32;
    let start_x = cx_min as f32 + (zone_w - total_w) / 2.0;
    // Baseline pour centrage vertical optique
    let baseline_y = cy_min as f32 + (zone_h + text_h) / 2.0 - descent.abs();

    let mut pen_x = start_x;
    let mut last_glyph_id = None;
    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(prev) = last_glyph_id {
            pen_x += scaled.kern(prev, gid);
        }
        let glyph = gid.with_scale_and_position(scale, point(pen_x, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                blend_px(pixels, px, py, color, coverage);
            });
        }
        pen_x += scaled.h_advance(gid);
        last_glyph_id = Some(gid);
    }
}

pub fn build_icon(day: u32) -> Vec<u8> {
    let size = RENDER_SIZE as usize;
    let mut pixels = vec![0u8; size * size * 4];

    let fg = [40u8, 40, 40, 255];
    let fg_rgb = [fg[0], fg[1], fg[2]];

    // Cadre arrondi
    draw_frame(&mut pixels, fg);

    // Anneaux de reliure : capsules verticales (petits traits arrondis épais)
    let ring_cy = 3 * SUPER_SCALE as i32;
    let ring_half_h = 2 * SUPER_SCALE as i32;
    let ring_half_w = SUPER_SCALE as i32 + SUPER_SCALE as i32 / 2; // 6 → ~1.5 px effectif
    fill_capsule_v(&mut pixels, 14 * SUPER_SCALE as i32, ring_cy, ring_half_w, ring_half_h, fg);
    fill_capsule_v(&mut pixels, 30 * SUPER_SCALE as i32, ring_cy, ring_half_w, ring_half_h, fg);

    // Chiffres : centrés dans la zone sous les anneaux, avec une marge intérieure
    // confortable par rapport au cadre.
    let text = day.to_string();
    let zone_y0 = 11 * SUPER_SCALE as i32;
    let zone_y1 = (ICON_SIZE as i32 - 5) * SUPER_SCALE as i32;
    let zone_x0 = 6 * SUPER_SCALE as i32;
    let zone_x1 = (ICON_SIZE as i32 - 6) * SUPER_SCALE as i32;

    match font() {
        Some(f) => {
            draw_text_centered(&mut pixels, f, &text, zone_x0, zone_y0, zone_x1, zone_y1, fg_rgb);
        }
        None => {
            warn!("Aucune fonte système chargée — icône sans chiffre");
        }
    }

    let big: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(RENDER_SIZE, RENDER_SIZE, pixels).expect("buffer size invariant");
    let small = image::imageops::resize(&big, ICON_SIZE, ICON_SIZE, FilterType::Lanczos3);
    small.into_raw()
}
