//! SimDisplay — terminal LCD renderer using half-block characters.
//!
//! Converts DynFramebuffer RGB565 pixels into terminal output using
//! Unicode half-block characters (▀/▄/█/space). Each terminal cell
//! represents 2 vertical pixels (top + bottom half-block).

use vk_display::color::Rgb565;
use vk_display::framebuffer::DynFramebuffer;

/// Render a framebuffer to a string of ANSI-colored half-block characters.
///
/// Each terminal character cell shows 2 vertical pixels using the upper
/// half-block character (▀) with foreground=top pixel, background=bottom pixel.
///
/// Returns lines of ANSI-escaped text ready for terminal output.
pub fn framebuffer_to_terminal(fb: &DynFramebuffer) -> Vec<String> {
    let w = fb.width() as usize;
    let h = fb.height() as usize;
    let front = fb.front_buffer();
    let mut lines = Vec::new();

    // Process 2 rows at a time
    let mut y = 0;
    while y < h {
        let mut line = String::with_capacity(w * 20);
        for x in 0..w {
            let top = front[y * w + x];
            let bottom = if y + 1 < h {
                front[(y + 1) * w + x]
            } else {
                Rgb565::BLACK
            };

            let (tr, tg, tb) = rgb565_to_rgb8(top);
            let (br, bg, bb) = rgb565_to_rgb8(bottom);

            if top == Rgb565::BLACK && bottom == Rgb565::BLACK {
                line.push_str("\x1b[0m "); // reset before space to avoid stale bg
            } else {
                // ▀ with fg=top, bg=bottom
                line.push_str(&format!(
                    "\x1b[38;2;{tr};{tg};{tb}m\x1b[48;2;{br};{bg};{bb}m▀"
                ));
            }
        }
        line.push_str("\x1b[0m");
        lines.push(line);
        y += 2;
    }

    lines
}

/// Convert RGB565 to 8-bit RGB components.
fn rgb565_to_rgb8(c: Rgb565) -> (u8, u8, u8) {
    let rgba = c.to_rgba();
    (rgba.r, rgba.g, rgba.b)
}

/// Compute the number of terminal rows needed for a framebuffer.
#[allow(dead_code)]
pub fn terminal_rows_needed(fb_height: u16) -> u16 {
    fb_height.div_ceil(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_framebuffer_renders_spaces() {
        let mut fb = DynFramebuffer::new(4, 4);
        fb.swap(); // front = all black
        let lines = framebuffer_to_terminal(&fb);
        assert_eq!(lines.len(), 2); // 4 rows / 2
        // All spaces (plus reset escape)
        for line in &lines {
            assert!(line.ends_with("\x1b[0m"));
            let content: String = line.chars().filter(|c| *c == ' ').collect();
            assert_eq!(content.len(), 4, "4 space chars for 4 pixel width");
        }
    }

    #[test]
    fn colored_pixel_produces_ansi() {
        let mut fb = DynFramebuffer::new(2, 2);
        fb.draw_pixel(0, 0, Rgb565::RED);
        fb.swap();
        let lines = framebuffer_to_terminal(&fb);
        assert_eq!(lines.len(), 1);
        // Should contain ANSI color escape for red
        assert!(lines[0].contains("\x1b[38;2;255;0;0m"), "should have red fg");
    }

    #[test]
    fn odd_height_handled() {
        let mut fb = DynFramebuffer::new(2, 3);
        fb.swap();
        let lines = framebuffer_to_terminal(&fb);
        assert_eq!(lines.len(), 2); // ceil(3/2) = 2
    }

    #[test]
    fn terminal_rows_needed_calculation() {
        assert_eq!(terminal_rows_needed(412), 206);
        assert_eq!(terminal_rows_needed(1), 1);
        assert_eq!(terminal_rows_needed(0), 0);
        assert_eq!(terminal_rows_needed(3), 2);
    }

    #[test]
    fn rgb565_to_rgb8_black_white() {
        assert_eq!(rgb565_to_rgb8(Rgb565::BLACK), (0, 0, 0));
        assert_eq!(rgb565_to_rgb8(Rgb565::WHITE), (255, 255, 255));
    }

    #[test]
    fn mixed_pixels_produce_half_blocks() {
        let mut fb = DynFramebuffer::new(1, 2);
        fb.draw_pixel(0, 0, Rgb565::RED);   // top
        fb.draw_pixel(0, 1, Rgb565::GREEN); // bottom
        fb.swap();
        let lines = framebuffer_to_terminal(&fb);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains('▀'), "should use half-block character");
    }
}
