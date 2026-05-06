//! Widget system — self-drawing components for the 960×412 LCD.
//!
//! Each widget knows how to render itself to a framebuffer region.
//! Widgets are simple value types with no internal state beyond their data.

use vk_display::color::Rgb565;
use vk_display::framebuffer::DynFramebuffer;

/// A rectangular region on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
}

/// Text alignment within a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Bitmap font: 24×40 pixel fixed-width glyphs (scaled from 6×10 base, 4x).
/// Fills 960×412 LCD.
pub const FONT_W: u16 = 24;
pub const FONT_H: u16 = 40;

/// Base glyph dimensions (before scaling).
const BASE_W: u16 = 6;
const BASE_H: u16 = 10;
const SCALE_X: u16 = 4; // horizontal scale
const SCALE_Y: u16 = 4; // vertical scale (10*4=40)

/// Large font constants for titles (6x scale).
pub const FONT_W_LG: u16 = 36;
pub const FONT_H_LG: u16 = 60;
const SCALE_X_LG: u16 = 6;
const SCALE_Y_LG: u16 = 6;

/// Render a single ASCII character at custom scale.
pub fn draw_char_scaled(fb: &mut DynFramebuffer, x: u16, y: u16, ch: u8, color: Rgb565, sx: u16, sy: u16) {
    let glyph_w = BASE_W as u16 * sx;
    let glyph_h = BASE_H as u16 * sy;
    let fb_w = fb.width();
    let fb_h = fb.height();

    // Pre-clip: entire glyph is off-screen
    if x >= fb_w || y >= fb_h {
        return;
    }

    let glyph = font_glyph(ch);

    // Fast path: entire glyph is fully within bounds — use direct index access
    if x + glyph_w <= fb_w && y + glyph_h <= fb_h {
        let stride = fb_w as usize;
        for row in 0..BASE_H {
            let bits = glyph[row as usize];
            for col in 0..BASE_W {
                if bits & (0x80 >> col) != 0 {
                    let px = (x + col * sx) as usize;
                    let py = (y + row * sy) as usize;
                    let back = fb.back_mut();
                    for dy in 0..sy as usize {
                        let row_start = (py + dy) * stride + px;
                        for dx in 0..sx as usize {
                            back[row_start + dx] = color;
                        }
                    }
                }
            }
        }
    } else {
        // Slow path: partial clip needed
        for row in 0..BASE_H {
            let bits = glyph[row as usize];
            for col in 0..BASE_W {
                if bits & (0x80 >> col) != 0 {
                    let px = x + col * sx;
                    let py = y + row * sy;
                    for dy in 0..sy {
                        for dx in 0..sx {
                            if px + dx < fb_w && py + dy < fb_h {
                                fb.draw_pixel(px + dx, py + dy, color);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Render large text (4x scale, for titles).
pub fn draw_text_large(fb: &mut DynFramebuffer, x: u16, y: u16, text: &str, color: Rgb565) {
    let mut cx = x;
    for ch in text.chars() {
        if cx + FONT_W_LG > fb.width() { break; }
        if ch.is_ascii() {
            draw_char_scaled(fb, cx, y, ch as u8, color, SCALE_X_LG, SCALE_Y_LG);
            cx += FONT_W_LG;
        } else {
            // CJK via unifont at 2.5x scale (40px height)
            if let Some(g) = unifont::get_glyph(ch) {
                let gw = if g.get_width() == 16 { 16u16 } else { 8u16 };
                let scale = 3u16; // unifont 16px * 3 = 48px (close to 40)
                let dw = gw * scale;
                if cx + dw > fb.width() { break; }
                for row in 0..16u16 {
                    for col in 0..gw {
                        if g.get_pixel(col as usize, row as usize) {
                            let px = cx + col * scale;
                            let py = y + row * scale;
                            for dy in 0..scale { for dx in 0..scale {
                                if py + dy < fb.height() { fb.draw_pixel(px + dx, py + dy, color); }
                            }}
                        }
                    }
                }
                cx += dw;
            } else {
                draw_char_scaled(fb, cx, y, b'?', color, SCALE_X_LG, SCALE_Y_LG);
                cx += FONT_W_LG;
            }
        }
    }
}

/// Render a single ASCII character glyph at (x, y) into the framebuffer (scaled).
/// P0-3 fix: delegates to draw_char_scaled for fast-path rendering.
pub fn draw_char(fb: &mut DynFramebuffer, x: u16, y: u16, ch: u8, color: Rgb565) {
    draw_char_scaled(fb, x, y, ch, color, SCALE_X, SCALE_Y);
}

/// Scale factor for unifont rendering (unifont is 8x16 halfwidth / 16x16 fullwidth).
const UNI_SCALE: u16 = 3; // 3x scale: halfwidth=24x48, fullwidth=48x48 (matches 4x ASCII)

/// Render a string of text using unifont (supports full Unicode including CJK).
/// ASCII uses custom bitmap for consistency; non-ASCII uses unifont.
pub fn draw_text(fb: &mut DynFramebuffer, x: u16, y: u16, text: &str, color: Rgb565) {
    let mut cx = x;
    for ch in text.chars() {
        if cx + FONT_W > fb.width() {
            break;
        }
        if ch.is_ascii() {
            draw_char(fb, cx, y, ch as u8, color);
            cx += FONT_W;
        } else {
            // Use unifont for non-ASCII (CJK, emoji, etc.)
            let glyph = unifont::get_glyph(ch);
            match glyph {
                Some(g) => {
                    let is_wide = g.get_width() == 16;
                    let glyph_w = if is_wide { 16u16 } else { 8u16 };
                    let draw_w = glyph_w * UNI_SCALE;
                    if cx + draw_w > fb.width() { break; }
                    for row in 0..16u16 {
                        for col in 0..glyph_w {
                            if g.get_pixel(col as usize, row as usize) {
                                let px = cx + col * UNI_SCALE;
                                let py = y + row * UNI_SCALE;
                                for dy in 0..UNI_SCALE {
                                    for dx in 0..UNI_SCALE {
                                        if py + dy < fb.height() {
                                            fb.draw_pixel(px + dx, py + dy, color);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    cx += draw_w;
                }
                None => {
                    // Fallback: draw '?' for unknown chars
                    draw_char(fb, cx, y, b'?', color);
                    cx += FONT_W;
                }
            }
        }
    }
}

/// Medium font (3x scale: 18×30). For emphasized rows like the active session.
pub const FONT_W_MD: u16 = 18;
pub const FONT_H_MD: u16 = 30;
const SCALE_MD: u16 = 3;
const UNI_SCALE_MD: u16 = 2;

/// Small font (2x scale: 12×20). For dense data rows on the high-DPI panel.
pub const FONT_W_SM: u16 = 12;
pub const FONT_H_SM: u16 = 20;
const SCALE_SM: u16 = 2;
const UNI_SCALE_SM: u16 = 2;

/// Tiny font (1x scale: 6×10). For caption-like labels (table headers).
pub const FONT_W_TINY: u16 = 6;
pub const FONT_H_TINY: u16 = 10;
const SCALE_TINY: u16 = 1;
const UNI_SCALE_TINY: u16 = 1;

/// Internal: render text at arbitrary scale (ASCII via bitmap, CJK via unifont).
fn draw_text_at(
    fb: &mut DynFramebuffer,
    x: u16,
    y: u16,
    text: &str,
    color: Rgb565,
    sx: u16,
    sy: u16,
    uni_scale: u16,
) {
    let glyph_w = BASE_W * sx;
    let mut cx = x;
    for ch in text.chars() {
        if cx + glyph_w > fb.width() {
            break;
        }
        if ch.is_ascii() {
            draw_char_scaled(fb, cx, y, ch as u8, color, sx, sy);
            cx += glyph_w;
        } else if let Some(g) = unifont::get_glyph(ch) {
            let gw = if g.get_width() == 16 { 16u16 } else { 8u16 };
            let dw = gw * uni_scale;
            if cx + dw > fb.width() {
                break;
            }
            for row in 0..16u16 {
                for col in 0..gw {
                    if g.get_pixel(col as usize, row as usize) {
                        let px = cx + col * uni_scale;
                        let py = y + row * uni_scale;
                        for dy in 0..uni_scale {
                            for dx in 0..uni_scale {
                                if py + dy < fb.height() {
                                    fb.draw_pixel(px + dx, py + dy, color);
                                }
                            }
                        }
                    }
                }
            }
            cx += dw;
        } else {
            draw_char_scaled(fb, cx, y, b'?', color, sx, sy);
            cx += glyph_w;
        }
    }
}

/// Render text at medium scale (18×30) — for emphasized/active rows.
pub fn draw_text_md(fb: &mut DynFramebuffer, x: u16, y: u16, text: &str, color: Rgb565) {
    draw_text_at(fb, x, y, text, color, SCALE_MD, SCALE_MD, UNI_SCALE_MD);
}

/// Render text at small scale (12×20).
pub fn draw_text_sm(fb: &mut DynFramebuffer, x: u16, y: u16, text: &str, color: Rgb565) {
    draw_text_at(fb, x, y, text, color, SCALE_SM, SCALE_SM, UNI_SCALE_SM);
}

/// Render text at tiny scale (6×10) — for caption rows like table headers.
pub fn draw_text_tiny(fb: &mut DynFramebuffer, x: u16, y: u16, text: &str, color: Rgb565) {
    draw_text_at(fb, x, y, text, color, SCALE_TINY, SCALE_TINY, UNI_SCALE_TINY);
}

/// A simple text label widget.
#[derive(Debug, Clone)]
pub struct Label {
    pub text: String,
    pub color: Rgb565,
    pub align: Align,
}

impl Label {
    pub fn new(text: impl Into<String>, color: Rgb565) -> Self {
        Self {
            text: text.into(),
            color,
            align: Align::Left,
        }
    }

    pub fn aligned(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn render(&self, fb: &mut DynFramebuffer, bounds: Rect) {
        let text_w = self.text.len() as u16 * FONT_W;
        let x = match self.align {
            Align::Left => bounds.x,
            Align::Center => bounds.x + bounds.w.saturating_sub(text_w) / 2,
            Align::Right => bounds.x + bounds.w.saturating_sub(text_w),
        };
        let y = bounds.y + bounds.h.saturating_sub(FONT_H) / 2;
        draw_text(fb, x, y, &self.text, self.color);
    }
}

/// A horizontal status bar showing a key-value pair.
#[derive(Debug, Clone)]
pub struct StatusBar {
    pub key: String,
    pub value: String,
    pub key_color: Rgb565,
    pub value_color: Rgb565,
}

impl StatusBar {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            key_color: Rgb565::WHITE,
            value_color: Rgb565::GREEN,
        }
    }

    pub fn render(&self, fb: &mut DynFramebuffer, bounds: Rect) {
        let y = bounds.y + bounds.h.saturating_sub(FONT_H) / 2;
        draw_text(fb, bounds.x, y, &self.key, self.key_color);
        let val_x = bounds.x + (self.key.len() as u16 + 1) * FONT_W;
        draw_text(fb, val_x, y, &self.value, self.value_color);
    }
}

/// A horizontal divider line.
pub fn draw_divider(fb: &mut DynFramebuffer, y: u16, width: u16, color: Rgb565) {
    for x in 0..width {
        fb.draw_pixel(x, y, color);
    }
}

// ── Minimal bitmap font (6×10, ASCII 0x20..0x7F) ──
// This is a simplified font. Characters not in range render as blank.

fn font_glyph(ch: u8) -> [u8; 10] {
    if !(0x20..=0x7E).contains(&ch) {
        return [0; 10];
    }
    FONT_DATA[(ch - 0x20) as usize]
}

/// 6×10 bitmap font data for printable ASCII.
/// Each entry is 10 rows; each row is a byte with the 6 MSBs as pixel columns.
#[rustfmt::skip]
const FONT_DATA: [[u8; 10]; 95] = [
    // 0x20 ' '
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x21 '!'
    [0x00, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x20, 0x00, 0x00],
    // 0x22 '"'
    [0x00, 0x50, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x23 '#'
    [0x00, 0x50, 0xF8, 0x50, 0x50, 0xF8, 0x50, 0x00, 0x00, 0x00],
    // 0x24 '$'
    [0x00, 0x20, 0x70, 0xA0, 0x70, 0x28, 0x70, 0x20, 0x00, 0x00],
    // 0x25 '%'
    [0x00, 0x48, 0xA8, 0x50, 0x20, 0x50, 0xA8, 0x90, 0x00, 0x00],
    // 0x26 '&'
    [0x00, 0x20, 0x50, 0x20, 0x68, 0x90, 0x68, 0x00, 0x00, 0x00],
    // 0x27 '\''
    [0x00, 0x20, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x28 '('
    [0x00, 0x10, 0x20, 0x20, 0x20, 0x20, 0x20, 0x10, 0x00, 0x00],
    // 0x29 ')'
    [0x00, 0x40, 0x20, 0x20, 0x20, 0x20, 0x20, 0x40, 0x00, 0x00],
    // 0x2A '*'
    [0x00, 0x00, 0x20, 0x70, 0x20, 0x50, 0x00, 0x00, 0x00, 0x00],
    // 0x2B '+'
    [0x00, 0x00, 0x20, 0x20, 0xF8, 0x20, 0x20, 0x00, 0x00, 0x00],
    // 0x2C ','
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x20, 0x40, 0x00],
    // 0x2D '-'
    [0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x2E '.'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00],
    // 0x2F '/'
    [0x00, 0x08, 0x08, 0x10, 0x20, 0x40, 0x80, 0x80, 0x00, 0x00],
    // 0x30 '0'
    [0x00, 0x70, 0x88, 0x98, 0xA8, 0xC8, 0x88, 0x70, 0x00, 0x00],
    // 0x31 '1'
    [0x00, 0x20, 0x60, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00, 0x00],
    // 0x32 '2'
    [0x00, 0x70, 0x88, 0x08, 0x10, 0x20, 0x40, 0xF8, 0x00, 0x00],
    // 0x33 '3'
    [0x00, 0x70, 0x88, 0x08, 0x30, 0x08, 0x88, 0x70, 0x00, 0x00],
    // 0x34 '4'
    [0x00, 0x10, 0x30, 0x50, 0x90, 0xF8, 0x10, 0x10, 0x00, 0x00],
    // 0x35 '5'
    [0x00, 0xF8, 0x80, 0xF0, 0x08, 0x08, 0x88, 0x70, 0x00, 0x00],
    // 0x36 '6'
    [0x00, 0x30, 0x40, 0x80, 0xF0, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x37 '7'
    [0x00, 0xF8, 0x08, 0x10, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00],
    // 0x38 '8'
    [0x00, 0x70, 0x88, 0x88, 0x70, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x39 '9'
    [0x00, 0x70, 0x88, 0x88, 0x78, 0x08, 0x10, 0x60, 0x00, 0x00],
    // 0x3A ':'
    [0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00],
    // 0x3B ';'
    [0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x20, 0x20, 0x40, 0x00],
    // 0x3C '<'
    [0x00, 0x08, 0x10, 0x20, 0x40, 0x20, 0x10, 0x08, 0x00, 0x00],
    // 0x3D '='
    [0x00, 0x00, 0x00, 0xF8, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00],
    // 0x3E '>'
    [0x00, 0x40, 0x20, 0x10, 0x08, 0x10, 0x20, 0x40, 0x00, 0x00],
    // 0x3F '?'
    [0x00, 0x70, 0x88, 0x08, 0x10, 0x20, 0x00, 0x20, 0x00, 0x00],
    // 0x40 '@'
    [0x00, 0x70, 0x88, 0xB8, 0xA8, 0xB8, 0x80, 0x70, 0x00, 0x00],
    // 0x41 'A'
    [0x00, 0x20, 0x50, 0x88, 0x88, 0xF8, 0x88, 0x88, 0x00, 0x00],
    // 0x42 'B'
    [0x00, 0xF0, 0x88, 0x88, 0xF0, 0x88, 0x88, 0xF0, 0x00, 0x00],
    // 0x43 'C'
    [0x00, 0x70, 0x88, 0x80, 0x80, 0x80, 0x88, 0x70, 0x00, 0x00],
    // 0x44 'D'
    [0x00, 0xF0, 0x88, 0x88, 0x88, 0x88, 0x88, 0xF0, 0x00, 0x00],
    // 0x45 'E'
    [0x00, 0xF8, 0x80, 0x80, 0xF0, 0x80, 0x80, 0xF8, 0x00, 0x00],
    // 0x46 'F'
    [0x00, 0xF8, 0x80, 0x80, 0xF0, 0x80, 0x80, 0x80, 0x00, 0x00],
    // 0x47 'G'
    [0x00, 0x70, 0x88, 0x80, 0xB8, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x48 'H'
    [0x00, 0x88, 0x88, 0x88, 0xF8, 0x88, 0x88, 0x88, 0x00, 0x00],
    // 0x49 'I'
    [0x00, 0x70, 0x20, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00, 0x00],
    // 0x4A 'J'
    [0x00, 0x38, 0x10, 0x10, 0x10, 0x10, 0x90, 0x60, 0x00, 0x00],
    // 0x4B 'K'
    [0x00, 0x88, 0x90, 0xA0, 0xC0, 0xA0, 0x90, 0x88, 0x00, 0x00],
    // 0x4C 'L'
    [0x00, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0xF8, 0x00, 0x00],
    // 0x4D 'M'
    [0x00, 0x88, 0xD8, 0xA8, 0x88, 0x88, 0x88, 0x88, 0x00, 0x00],
    // 0x4E 'N'
    [0x00, 0x88, 0xC8, 0xA8, 0x98, 0x88, 0x88, 0x88, 0x00, 0x00],
    // 0x4F 'O'
    [0x00, 0x70, 0x88, 0x88, 0x88, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x50 'P'
    [0x00, 0xF0, 0x88, 0x88, 0xF0, 0x80, 0x80, 0x80, 0x00, 0x00],
    // 0x51 'Q'
    [0x00, 0x70, 0x88, 0x88, 0x88, 0xA8, 0x90, 0x68, 0x00, 0x00],
    // 0x52 'R'
    [0x00, 0xF0, 0x88, 0x88, 0xF0, 0xA0, 0x90, 0x88, 0x00, 0x00],
    // 0x53 'S'
    [0x00, 0x70, 0x88, 0x80, 0x70, 0x08, 0x88, 0x70, 0x00, 0x00],
    // 0x54 'T'
    [0x00, 0xF8, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00],
    // 0x55 'U'
    [0x00, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x56 'V'
    [0x00, 0x88, 0x88, 0x88, 0x88, 0x50, 0x50, 0x20, 0x00, 0x00],
    // 0x57 'W'
    [0x00, 0x88, 0x88, 0x88, 0xA8, 0xA8, 0xD8, 0x88, 0x00, 0x00],
    // 0x58 'X'
    [0x00, 0x88, 0x88, 0x50, 0x20, 0x50, 0x88, 0x88, 0x00, 0x00],
    // 0x59 'Y'
    [0x00, 0x88, 0x88, 0x50, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00],
    // 0x5A 'Z'
    [0x00, 0xF8, 0x08, 0x10, 0x20, 0x40, 0x80, 0xF8, 0x00, 0x00],
    // 0x5B '['
    [0x00, 0x70, 0x40, 0x40, 0x40, 0x40, 0x40, 0x70, 0x00, 0x00],
    // 0x5C '\'
    [0x00, 0x80, 0x80, 0x40, 0x20, 0x10, 0x08, 0x08, 0x00, 0x00],
    // 0x5D ']'
    [0x00, 0x70, 0x10, 0x10, 0x10, 0x10, 0x10, 0x70, 0x00, 0x00],
    // 0x5E '^'
    [0x00, 0x20, 0x50, 0x88, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x5F '_'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF8, 0x00, 0x00],
    // 0x60 '`'
    [0x00, 0x40, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x61 'a'
    [0x00, 0x00, 0x00, 0x70, 0x08, 0x78, 0x88, 0x78, 0x00, 0x00],
    // 0x62 'b'
    [0x00, 0x80, 0x80, 0xF0, 0x88, 0x88, 0x88, 0xF0, 0x00, 0x00],
    // 0x63 'c'
    [0x00, 0x00, 0x00, 0x70, 0x88, 0x80, 0x88, 0x70, 0x00, 0x00],
    // 0x64 'd'
    [0x00, 0x08, 0x08, 0x78, 0x88, 0x88, 0x88, 0x78, 0x00, 0x00],
    // 0x65 'e'
    [0x00, 0x00, 0x00, 0x70, 0x88, 0xF8, 0x80, 0x70, 0x00, 0x00],
    // 0x66 'f'
    [0x00, 0x30, 0x48, 0x40, 0xF0, 0x40, 0x40, 0x40, 0x00, 0x00],
    // 0x67 'g'
    [0x00, 0x00, 0x00, 0x78, 0x88, 0x88, 0x78, 0x08, 0x70, 0x00],
    // 0x68 'h'
    [0x00, 0x80, 0x80, 0xF0, 0x88, 0x88, 0x88, 0x88, 0x00, 0x00],
    // 0x69 'i'
    [0x00, 0x20, 0x00, 0x60, 0x20, 0x20, 0x20, 0x70, 0x00, 0x00],
    // 0x6A 'j'
    [0x00, 0x10, 0x00, 0x30, 0x10, 0x10, 0x10, 0x90, 0x60, 0x00],
    // 0x6B 'k'
    [0x00, 0x80, 0x80, 0x90, 0xA0, 0xC0, 0xA0, 0x90, 0x00, 0x00],
    // 0x6C 'l'
    [0x00, 0x60, 0x20, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00, 0x00],
    // 0x6D 'm'
    [0x00, 0x00, 0x00, 0xD0, 0xA8, 0xA8, 0xA8, 0x88, 0x00, 0x00],
    // 0x6E 'n'
    [0x00, 0x00, 0x00, 0xF0, 0x88, 0x88, 0x88, 0x88, 0x00, 0x00],
    // 0x6F 'o'
    [0x00, 0x00, 0x00, 0x70, 0x88, 0x88, 0x88, 0x70, 0x00, 0x00],
    // 0x70 'p'
    [0x00, 0x00, 0x00, 0xF0, 0x88, 0x88, 0xF0, 0x80, 0x80, 0x00],
    // 0x71 'q'
    [0x00, 0x00, 0x00, 0x78, 0x88, 0x88, 0x78, 0x08, 0x08, 0x00],
    // 0x72 'r'
    [0x00, 0x00, 0x00, 0xB0, 0xC8, 0x80, 0x80, 0x80, 0x00, 0x00],
    // 0x73 's'
    [0x00, 0x00, 0x00, 0x70, 0x80, 0x70, 0x08, 0xF0, 0x00, 0x00],
    // 0x74 't'
    [0x00, 0x40, 0x40, 0xF0, 0x40, 0x40, 0x48, 0x30, 0x00, 0x00],
    // 0x75 'u'
    [0x00, 0x00, 0x00, 0x88, 0x88, 0x88, 0x88, 0x78, 0x00, 0x00],
    // 0x76 'v'
    [0x00, 0x00, 0x00, 0x88, 0x88, 0x50, 0x50, 0x20, 0x00, 0x00],
    // 0x77 'w'
    [0x00, 0x00, 0x00, 0x88, 0xA8, 0xA8, 0xA8, 0x50, 0x00, 0x00],
    // 0x78 'x'
    [0x00, 0x00, 0x00, 0x88, 0x50, 0x20, 0x50, 0x88, 0x00, 0x00],
    // 0x79 'y'
    [0x00, 0x00, 0x00, 0x88, 0x88, 0x78, 0x08, 0x88, 0x70, 0x00],
    // 0x7A 'z'
    [0x00, 0x00, 0x00, 0xF8, 0x10, 0x20, 0x40, 0xF8, 0x00, 0x00],
    // 0x7B '{'
    [0x00, 0x18, 0x20, 0x20, 0xC0, 0x20, 0x20, 0x18, 0x00, 0x00],
    // 0x7C '|'
    [0x00, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00],
    // 0x7D '}'
    [0x00, 0xC0, 0x20, 0x20, 0x18, 0x20, 0x20, 0xC0, 0x00, 0x00],
    // 0x7E '~'
    [0x00, 0x00, 0x00, 0x60, 0x92, 0x0C, 0x00, 0x00, 0x00, 0x00],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_render_left_aligned() {
        let mut fb = DynFramebuffer::new(60, 20);
        let label = Label::new("Hi", Rgb565::WHITE);
        label.render(&mut fb, Rect::new(0, 0, 60, 20));
        // Should have some non-black pixels from the 'H' glyph
        let has_pixels = fb.back_buffer().iter().any(|c| *c != Rgb565::BLACK);
        assert!(has_pixels, "label should render pixels");
    }

    #[test]
    fn label_render_center_aligned() {
        let mut fb = DynFramebuffer::new(60, 20);
        let label = Label::new("A", Rgb565::WHITE).aligned(Align::Center);
        label.render(&mut fb, Rect::new(0, 0, 60, 20));
        // 'A' is 6px wide, center in 60px → starts at ~27
        // Check that pixel at col 0 row 5 is black (text not at left edge)
        assert_eq!(fb.back_buffer()[5 * 60], Rgb565::BLACK);
    }

    #[test]
    fn label_render_right_aligned() {
        let mut fb = DynFramebuffer::new(60, 20);
        let label = Label::new("X", Rgb565::WHITE).aligned(Align::Right);
        label.render(&mut fb, Rect::new(0, 0, 60, 20));
        // Text should be near right edge
        let has_pixels = fb.back_buffer().iter().any(|c| *c != Rgb565::BLACK);
        assert!(has_pixels, "right-aligned label should render pixels");
    }

    #[test]
    fn status_bar_render() {
        let mut fb = DynFramebuffer::new(120, 20);
        let bar = StatusBar::new("Mode:", "PLAN");
        bar.render(&mut fb, Rect::new(0, 0, 120, 20));
        let has_pixels = fb.back_buffer().iter().any(|c| *c != Rgb565::BLACK);
        assert!(has_pixels, "status bar should render pixels");
    }

    #[test]
    fn draw_char_renders_pixels() {
        let mut fb = DynFramebuffer::new(10, 12);
        draw_char(&mut fb, 0, 0, b'A', Rgb565::WHITE);
        let has_pixels = fb.back_buffer().iter().any(|c| *c != Rgb565::BLACK);
        assert!(has_pixels, "draw_char('A') should produce non-black pixels");
    }

    #[test]
    fn draw_text_clips_at_boundary() {
        let mut fb = DynFramebuffer::new(12, 12); // only room for 2 chars
        draw_text(&mut fb, 0, 0, "ABCDEF", Rgb565::WHITE);
        // Should not panic, and last columns should be blank
        let last_col_row = 5 * 12 + 11;
        assert_eq!(fb.back_buffer()[last_col_row], Rgb565::BLACK);
    }

    #[test]
    fn draw_divider_fills_row() {
        let mut fb = DynFramebuffer::new(20, 5);
        draw_divider(&mut fb, 2, 20, Rgb565::WHITE);
        for x in 0..20u16 {
            let idx = 2 * 20 + x as usize;
            assert_eq!(fb.back_buffer()[idx], Rgb565::WHITE, "divider pixel at x={x}");
        }
        // Row above should be black
        assert_eq!(fb.back_buffer()[1 * 20], Rgb565::BLACK);
    }

    #[test]
    fn font_glyph_space_is_blank() {
        let g = font_glyph(b' ');
        assert!(g.iter().all(|b| *b == 0));
    }

    #[test]
    fn font_glyph_out_of_range_is_blank() {
        let g = font_glyph(0x00);
        assert!(g.iter().all(|b| *b == 0));
        let g2 = font_glyph(0xFF);
        assert!(g2.iter().all(|b| *b == 0));
    }
}
