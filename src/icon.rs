/// Bitmap 3×5 pour les chiffres 0-9 (chaque u8 = ligne de bits)
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

fn draw_digit(pixels: &mut [u8], digit: u8, ox: i32, oy: i32, color: [u8; 3]) {
    let bmp = &DIGITS[digit as usize];
    for (row, &bits) in bmp.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) != 0 {
                let x = ox + col as i32;
                let y = oy + row as i32 * 2; // scale ×2 verticalement
                for dy in 0..2i32 {
                    let px = y + dy;
                    if x >= 0 && x < 32 && px >= 0 && px < 32 {
                        let idx = (px as usize * 32 + x as usize) * 4;
                        pixels[idx] = color[0];
                        pixels[idx + 1] = color[1];
                        pixels[idx + 2] = color[2];
                        pixels[idx + 3] = 255;
                    }
                }
            }
        }
    }
}

pub fn build_icon(day: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; 32 * 32 * 4];

    // Fond blanc arrondi
    for y in 2u32..30 {
        for x in 2u32..30 {
            let idx = (y * 32 + x) as usize * 4;
            pixels[idx] = 248;
            pixels[idx + 1] = 248;
            pixels[idx + 2] = 248;
            pixels[idx + 3] = 255;
        }
    }

    // Barre rouge en haut (style calendrier)
    for y in 2u32..9 {
        for x in 2u32..30 {
            let idx = (y * 32 + x) as usize * 4;
            pixels[idx] = 220;
            pixels[idx + 1] = 50;
            pixels[idx + 2] = 50;
            pixels[idx + 3] = 255;
        }
    }

    // Chiffre(s) du jour centré
    let tens = day / 10;
    let units = day % 10;
    let dark = [40u8, 40, 40];

    if tens > 0 {
        draw_digit(&mut pixels, tens as u8, 10, 14, dark);
        draw_digit(&mut pixels, units as u8, 18, 14, dark);
    } else {
        // Centré pour 1-9
        draw_digit(&mut pixels, units as u8, 15, 14, dark);
    }

    pixels
}
