use crate::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    pub(crate) fn from_color(color: Color) -> Self {
        rgb_to_hsv(color)
    }

    pub(crate) fn to_color(self, alpha: u8) -> Color {
        hsv_to_rgb(self.h, self.s, self.v, alpha)
    }
}

pub(crate) fn rgb_to_hsv(color: Color) -> Hsv {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let mut h = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        ((b - r) / delta) + 2.0
    } else {
        ((r - g) / delta) + 4.0
    } / 6.0;

    if h >= 1.0 {
        h = 0.0;
    }

    let s = if max <= f32::EPSILON { 0.0 } else { delta / max };

    Hsv { h, s, v: max }
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32, alpha: u8) -> Color {
    let h = h.rem_euclid(1.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);

    let scaled = h * 6.0;
    let sector = scaled.floor() as i32;
    let fraction = scaled - sector as f32;

    let p = v * (1.0 - s);
    let q = v * (1.0 - fraction * s);
    let t = v * (1.0 - (1.0 - fraction) * s);

    let (r, g, b) = match sector.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    Color::rgba(to_byte(r), to_byte(g), to_byte(b), alpha)
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_hues_round_trip() {
        for color in [
            Color::rgb(255, 0, 0),
            Color::rgb(0, 255, 0),
            Color::rgb(0, 0, 255),
            Color::rgb(255, 255, 255),
            Color::rgb(0, 0, 0),
            Color::rgb(127, 63, 231),
        ] {
            let hsv = rgb_to_hsv(color);
            let result = hsv.to_color(color.a);
            assert!((result.r as i16 - color.r as i16).abs() <= 1);
            assert!((result.g as i16 - color.g as i16).abs() <= 1);
            assert!((result.b as i16 - color.b as i16).abs() <= 1);
        }
    }

    #[test]
    fn alpha_is_preserved_by_conversion() {
        let color = hsv_to_rgb(0.5, 0.8, 0.6, 77);
        assert_eq!(color.a, 77);
    }
}
