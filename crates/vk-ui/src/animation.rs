//! Simple animation primitives driven by frame counter.
//!
//! No timers or floating-point — purely frame-count driven.

/// Blink animation: returns true for `on_frames`, false for `off_frames`, repeating.
pub fn blink(frame: u32, on_frames: u32, off_frames: u32) -> bool {
    let cycle = on_frames + off_frames;
    if cycle == 0 {
        return true;
    }
    (frame % cycle) < on_frames
}

/// Pulse animation: returns a brightness 0..=max that rises then falls over `period` frames.
pub fn pulse(frame: u32, period: u32, max: u8) -> u8 {
    if period == 0 {
        return max;
    }
    let half = period / 2;
    let pos = frame % period;
    if pos < half {
        // Rising
        (pos * max as u32 / half.max(1)) as u8
    } else {
        // Falling
        ((period - pos) * max as u32 / half.max(1)) as u8
    }
}

/// Slide offset: linearly interpolates from `start` to `end` over `duration` frames.
/// Returns `end` if frame >= duration.
pub fn slide(frame: u32, duration: u32, start: i16, end: i16) -> i16 {
    if duration == 0 || frame >= duration {
        return end;
    }
    let delta = end as i32 - start as i32;
    let offset = (delta * frame as i32) / duration as i32;
    (start as i32 + offset) as i16
}

/// Highlight toggle: alternates between two values every `interval` frames.
pub fn alternate<T: Copy>(frame: u32, interval: u32, a: T, b: T) -> T {
    if interval == 0 {
        return a;
    }
    if (frame / interval).is_multiple_of(2) {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_basic_cycle() {
        // 10 on, 10 off
        assert!(blink(0, 10, 10));
        assert!(blink(9, 10, 10));
        assert!(!blink(10, 10, 10));
        assert!(!blink(19, 10, 10));
        assert!(blink(20, 10, 10)); // wraps
    }

    #[test]
    fn blink_zero_cycle() {
        assert!(blink(0, 0, 0));
        assert!(blink(100, 0, 0));
    }

    #[test]
    fn blink_always_on() {
        assert!(blink(0, 10, 0));
        assert!(blink(5, 10, 0));
    }

    #[test]
    fn pulse_rises_and_falls() {
        // period=20, max=100
        assert_eq!(pulse(0, 20, 100), 0);
        assert_eq!(pulse(5, 20, 100), 50);
        assert_eq!(pulse(10, 20, 100), 100);
        assert_eq!(pulse(15, 20, 100), 50);
    }

    #[test]
    fn pulse_zero_period() {
        assert_eq!(pulse(0, 0, 255), 255);
    }

    #[test]
    fn slide_basic() {
        assert_eq!(slide(0, 10, 0, 100), 0);
        assert_eq!(slide(5, 10, 0, 100), 50);
        assert_eq!(slide(10, 10, 0, 100), 100);
        assert_eq!(slide(20, 10, 0, 100), 100); // past duration
    }

    #[test]
    fn slide_zero_duration() {
        assert_eq!(slide(0, 0, 0, 100), 100);
    }

    #[test]
    fn slide_negative_direction() {
        assert_eq!(slide(0, 10, 100, 0), 100);
        assert_eq!(slide(5, 10, 100, 0), 50);
        assert_eq!(slide(10, 10, 100, 0), 0);
    }

    #[test]
    fn alternate_basic() {
        assert_eq!(alternate(0, 5, "A", "B"), "A");
        assert_eq!(alternate(4, 5, "A", "B"), "A");
        assert_eq!(alternate(5, 5, "A", "B"), "B");
        assert_eq!(alternate(9, 5, "A", "B"), "B");
        assert_eq!(alternate(10, 5, "A", "B"), "A");
    }

    #[test]
    fn alternate_zero_interval() {
        assert_eq!(alternate(0, 0, 1, 2), 1);
        assert_eq!(alternate(100, 0, 1, 2), 1);
    }
}
