//! Color types supporting both embedded (RGB565) and desktop (RGBA) rendering.

/// RGB565 color for embedded LCD displays (16-bit, 65K colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgb565(pub u16);

// SAFETY: Rgb565 is #[repr(transparent)] over u16, which is Pod+Zeroable.
unsafe impl bytemuck::Zeroable for Rgb565 {}
unsafe impl bytemuck::Pod for Rgb565 {}

impl Rgb565 {
    pub const BLACK: Self = Self(0x0000);
    pub const WHITE: Self = Self(0xFFFF);
    pub const RED: Self = Self(0xF800);
    pub const GREEN: Self = Self(0x07E0);
    pub const BLUE: Self = Self(0x001F);

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        let r5 = (r >> 3) as u16;
        let g6 = (g >> 2) as u16;
        let b5 = (b >> 3) as u16;
        Self((r5 << 11) | (g6 << 5) | b5)
    }

    pub fn to_rgba(self) -> Rgba {
        let r = ((self.0 >> 11) & 0x1F) as u8;
        let g = ((self.0 >> 5) & 0x3F) as u8;
        let b = (self.0 & 0x1F) as u8;
        Rgba {
            r: (r << 3) | (r >> 2),
            g: (g << 2) | (g >> 4),
            b: (b << 3) | (b >> 2),
            a: 255,
        }
    }
}

/// RGBA color for desktop rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    pub fn to_rgb565(self) -> Rgb565 {
        Rgb565::new(self.r, self.g, self.b)
    }

    pub fn to_u32(self) -> u32 {
        (self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_new_pure_colors() {
        // Pure red: r=0xFF -> r5=0x1F, g=0, b=0 -> 0x1F << 11 = 0xF800
        assert_eq!(Rgb565::new(255, 0, 0), Rgb565::RED);
        // Pure green: g=0xFF -> g6=0x3F -> 0x3F << 5 = 0x07E0
        assert_eq!(Rgb565::new(0, 255, 0), Rgb565::GREEN);
        // Pure blue: b=0xFF -> b5=0x1F = 0x001F
        assert_eq!(Rgb565::new(0, 0, 255), Rgb565::BLUE);
    }

    #[test]
    fn rgb565_new_black_white() {
        assert_eq!(Rgb565::new(0, 0, 0), Rgb565::BLACK);
        assert_eq!(Rgb565::new(255, 255, 255), Rgb565::WHITE);
    }

    #[test]
    fn rgb565_to_rgba_black_white() {
        let black = Rgb565::BLACK.to_rgba();
        assert_eq!(black.r, 0);
        assert_eq!(black.g, 0);
        assert_eq!(black.b, 0);
        assert_eq!(black.a, 255);

        let white = Rgb565::WHITE.to_rgba();
        assert_eq!(white.r, 255);
        assert_eq!(white.g, 255);
        assert_eq!(white.b, 255);
    }

    #[test]
    fn rgba_to_rgb565_roundtrip() {
        // Exact roundtrip works for black and white
        let black_rt = Rgba::BLACK.to_rgb565().to_rgba();
        assert_eq!(black_rt, Rgba::BLACK);

        let white_rt = Rgba::WHITE.to_rgb565().to_rgba();
        assert_eq!(white_rt, Rgba::WHITE);
    }

    #[test]
    fn rgba_to_rgb565_lossy() {
        // Non-boundary values lose precision due to 5/6-bit quantization
        let rgba = Rgba { r: 100, g: 100, b: 100, a: 255 };
        let rgb565 = rgba.to_rgb565();
        let back = rgb565.to_rgba();
        // Should be close but not necessarily exact
        assert!((back.r as i16 - rgba.r as i16).unsigned_abs() <= 4);
        assert!((back.g as i16 - rgba.g as i16).unsigned_abs() <= 2);
        assert!((back.b as i16 - rgba.b as i16).unsigned_abs() <= 4);
    }

    #[test]
    fn rgba_to_u32() {
        let c = Rgba {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0xFF,
        };
        assert_eq!(c.to_u32(), 0xFF123456);
    }

    #[test]
    fn rgb565_to_rgba_pure_red() {
        let rgba = Rgb565::RED.to_rgba();
        assert_eq!(rgba.r, 255);
        assert_eq!(rgba.g, 0);
        assert_eq!(rgba.b, 0);
    }

    #[test]
    fn rgba_constants() {
        assert_eq!(Rgba::BLACK.to_u32(), 0xFF000000);
        assert_eq!(Rgba::WHITE.to_u32(), 0xFFFFFFFF);
    }
}
