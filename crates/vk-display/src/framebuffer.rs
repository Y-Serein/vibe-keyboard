//! Double-buffered framebuffer for flicker-free rendering.

use crate::color::Rgb565;

pub struct DynFramebuffer {
    width: u16,
    height: u16,
    front: Vec<Rgb565>,
    back: Vec<Rgb565>,
}

impl DynFramebuffer {
    pub fn new(width: u16, height: u16) -> Self {
        let size = width as usize * height as usize;
        Self {
            width,
            height,
            front: vec![Rgb565::BLACK; size],
            back: vec![Rgb565::BLACK; size],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Read a pixel from the back buffer. Returns BLACK if out of bounds.
    pub fn get_pixel(&self, x: u16, y: u16) -> Rgb565 {
        if x < self.width && y < self.height {
            self.back[y as usize * self.width as usize + x as usize]
        } else {
            Rgb565::BLACK
        }
    }

    pub fn draw_pixel(&mut self, x: u16, y: u16, color: Rgb565) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.back[idx] = color;
        }
    }

    pub fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
        let x_start = x as usize;
        let x_end = (x as usize + w as usize).min(self.width as usize);
        let y_end = (y as usize + h as usize).min(self.height as usize);
        if x_start >= x_end {
            return;
        }
        let stride = self.width as usize;
        for row in (y as usize)..y_end {
            self.back[row * stride + x_start..row * stride + x_end].fill(color);
        }
    }

    /// Swap front and back buffers.
    pub fn swap(&mut self) {
        core::mem::swap(&mut self.front, &mut self.back);
    }

    /// Get the front buffer data (for display/transfer).
    pub fn front_buffer(&self) -> &[Rgb565] {
        &self.front
    }

    /// Get the front buffer as raw bytes (zero-copy via bytemuck).
    pub fn front_buffer_as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.front)
    }

    /// Get the front buffer as raw LE bytes (RGB565), zero-copy into pre-allocated dest.
    /// Returns the number of bytes written (width * height * 2).
    pub fn front_buffer_to_bytes(&self, dest: &mut Vec<u8>) {
        let src = self.front_buffer_as_bytes();
        dest.clear();
        if dest.capacity() < src.len() {
            dest.reserve(src.len() - dest.capacity());
        }
        dest.extend_from_slice(src);
    }

    /// Get the back buffer data (for rendering).
    pub fn back_buffer(&self) -> &[Rgb565] {
        &self.back
    }

    /// Get mutable access to the back buffer slice (for direct pixel writes).
    pub fn back_mut(&mut self) -> &mut [Rgb565] {
        &mut self.back
    }

    /// Write a block of pixels into the back buffer (row-major order).
    ///
    /// Pixels outside the framebuffer bounds are silently clipped.
    pub fn write_pixels(&mut self, x: u16, y: u16, w: u16, h: u16, data: &[Rgb565]) {
        for row in 0..h {
            let py = y.saturating_add(row);
            if py >= self.height {
                break;
            }
            for col in 0..w {
                let px = x.saturating_add(col);
                if px >= self.width {
                    break;
                }
                let src_idx = row as usize * w as usize + col as usize;
                if src_idx >= data.len() {
                    return;
                }
                let dst_idx = py as usize * self.width as usize + px as usize;
                self.back[dst_idx] = data[src_idx];
            }
        }
    }

    /// Clear the back buffer to a uniform color.
    pub fn clear(&mut self, color: Rgb565) {
        self.back.fill(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_black_buffers() {
        let fb = DynFramebuffer::new(4, 3);
        assert_eq!(fb.width(), 4);
        assert_eq!(fb.height(), 3);
        assert!(fb.front_buffer().iter().all(|c| *c == Rgb565::BLACK));
        assert!(fb.back_buffer().iter().all(|c| *c == Rgb565::BLACK));
    }

    #[test]
    fn draw_pixel_sets_back_buffer() {
        let mut fb = DynFramebuffer::new(10, 10);
        fb.draw_pixel(3, 2, Rgb565::RED);
        let idx = 2 * 10 + 3;
        assert_eq!(fb.back_buffer()[idx], Rgb565::RED);
        // front unchanged
        assert_eq!(fb.front_buffer()[idx], Rgb565::BLACK);
    }

    #[test]
    fn draw_pixel_out_of_bounds_is_noop() {
        let mut fb = DynFramebuffer::new(5, 5);
        fb.draw_pixel(5, 0, Rgb565::RED);
        fb.draw_pixel(0, 5, Rgb565::RED);
        fb.draw_pixel(100, 100, Rgb565::RED);
        assert!(fb.back_buffer().iter().all(|c| *c == Rgb565::BLACK));
    }

    #[test]
    fn fill_rect_basic() {
        let mut fb = DynFramebuffer::new(10, 10);
        fb.fill_rect(1, 1, 2, 3, Rgb565::BLUE);
        // Check filled area
        for y in 1..4 {
            for x in 1..3 {
                let idx = y * 10 + x;
                assert_eq!(fb.back_buffer()[idx], Rgb565::BLUE, "pixel ({x},{y})");
            }
        }
        // Check unfilled neighbor
        assert_eq!(fb.back_buffer()[0], Rgb565::BLACK);
    }

    #[test]
    fn fill_rect_clips_at_boundary() {
        let mut fb = DynFramebuffer::new(4, 4);
        fb.fill_rect(2, 2, 10, 10, Rgb565::GREEN);
        // Only 2x2 corner should be filled
        assert_eq!(fb.back_buffer()[2 * 4 + 2], Rgb565::GREEN);
        assert_eq!(fb.back_buffer()[2 * 4 + 3], Rgb565::GREEN);
        assert_eq!(fb.back_buffer()[3 * 4 + 2], Rgb565::GREEN);
        assert_eq!(fb.back_buffer()[3 * 4 + 3], Rgb565::GREEN);
        // Outside the rect
        assert_eq!(fb.back_buffer()[1 * 4 + 2], Rgb565::BLACK);
    }

    #[test]
    fn write_pixels_basic() {
        let mut fb = DynFramebuffer::new(4, 4);
        let data = [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE, Rgb565::WHITE];
        fb.write_pixels(1, 1, 2, 2, &data);
        assert_eq!(fb.back_buffer()[1 * 4 + 1], Rgb565::RED);
        assert_eq!(fb.back_buffer()[1 * 4 + 2], Rgb565::GREEN);
        assert_eq!(fb.back_buffer()[2 * 4 + 1], Rgb565::BLUE);
        assert_eq!(fb.back_buffer()[2 * 4 + 2], Rgb565::WHITE);
    }

    #[test]
    fn write_pixels_clips() {
        let mut fb = DynFramebuffer::new(3, 3);
        let data = [Rgb565::RED; 4];
        fb.write_pixels(2, 2, 2, 2, &data);
        // Only (2,2) should be set (clipped at boundary)
        assert_eq!(fb.back_buffer()[2 * 3 + 2], Rgb565::RED);
        assert_eq!(fb.back_buffer()[2 * 3 + 1], Rgb565::BLACK);
    }

    #[test]
    fn write_pixels_short_data() {
        let mut fb = DynFramebuffer::new(4, 4);
        let data = [Rgb565::RED, Rgb565::GREEN]; // only 2 of 4 needed
        fb.write_pixels(0, 0, 2, 2, &data);
        assert_eq!(fb.back_buffer()[0], Rgb565::RED);
        assert_eq!(fb.back_buffer()[1], Rgb565::GREEN);
        // Third pixel index (1*2+0=2) is past data.len(), so stops
        assert_eq!(fb.back_buffer()[4], Rgb565::BLACK);
    }

    #[test]
    fn swap_exchanges_buffers() {
        let mut fb = DynFramebuffer::new(2, 2);
        fb.draw_pixel(0, 0, Rgb565::RED);
        assert_eq!(fb.front_buffer()[0], Rgb565::BLACK);
        fb.swap();
        assert_eq!(fb.front_buffer()[0], Rgb565::RED);
        assert_eq!(fb.back_buffer()[0], Rgb565::BLACK);
    }

    #[test]
    fn fill_rect_extreme_coordinates_no_overflow() {
        let mut fb = DynFramebuffer::new(4, 4);
        // Should not panic due to u16 overflow in debug builds
        fb.fill_rect(u16::MAX, u16::MAX, 1, 1, Rgb565::RED);
        fb.fill_rect(u16::MAX, 0, 10, 10, Rgb565::GREEN);
        fb.fill_rect(0, u16::MAX, 10, 10, Rgb565::BLUE);
        // All pixels should remain black since coordinates are out of bounds
        assert!(fb.back_buffer().iter().all(|c| *c == Rgb565::BLACK));
    }

    #[test]
    fn write_pixels_extreme_coordinates_no_overflow() {
        let mut fb = DynFramebuffer::new(4, 4);
        let data = [Rgb565::RED; 4];
        // Should not panic due to u16 overflow in debug builds
        fb.write_pixels(u16::MAX, u16::MAX, 2, 2, &data);
        fb.write_pixels(u16::MAX, 0, 2, 2, &data);
        fb.write_pixels(0, u16::MAX, 2, 2, &data);
        assert!(fb.back_buffer().iter().all(|c| *c == Rgb565::BLACK));
    }

    #[test]
    fn clear_fills_back_buffer() {
        let mut fb = DynFramebuffer::new(3, 3);
        fb.draw_pixel(0, 0, Rgb565::RED);
        fb.clear(Rgb565::WHITE);
        assert!(fb.back_buffer().iter().all(|c| *c == Rgb565::WHITE));
    }
}
