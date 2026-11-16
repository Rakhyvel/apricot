//! Smooth 2D noise for terrain, clouds, and anything else that should look natural

static HASH: [u8; 256] = [
    208, 34, 231, 213, 32, 248, 233, 56, 161, 78, 24, 140, 71, 48, 140, 254, 245, 255, 247, 247,
    40, 185, 248, 251, 245, 28, 124, 204, 204, 76, 36, 1, 107, 28, 234, 163, 202, 224, 245, 128,
    167, 204, 9, 92, 217, 54, 239, 174, 173, 102, 193, 189, 190, 121, 100, 108, 167, 44, 43, 77,
    180, 204, 8, 81, 70, 223, 11, 38, 24, 254, 210, 210, 177, 32, 81, 195, 243, 125, 8, 169, 112,
    32, 97, 53, 195, 13, 203, 9, 47, 104, 125, 117, 114, 124, 165, 203, 181, 235, 193, 206, 70,
    180, 174, 0, 167, 181, 41, 164, 30, 116, 127, 198, 245, 146, 87, 224, 149, 206, 57, 4, 192,
    210, 65, 210, 129, 240, 178, 105, 228, 108, 245, 148, 140, 40, 35, 195, 38, 58, 65, 207, 215,
    253, 65, 85, 208, 76, 62, 3, 237, 55, 89, 232, 50, 217, 64, 244, 157, 199, 121, 252, 90, 17,
    212, 203, 149, 152, 140, 187, 234, 177, 73, 174, 193, 100, 192, 143, 97, 53, 145, 135, 19, 103,
    13, 90, 135, 151, 199, 91, 239, 247, 33, 39, 145, 101, 120, 99, 3, 186, 86, 99, 41, 237, 203,
    111, 79, 220, 135, 158, 42, 30, 154, 120, 67, 87, 167, 135, 176, 183, 191, 253, 115, 184, 21,
    233, 58, 129, 233, 142, 39, 128, 211, 118, 137, 139, 255, 114, 20, 218, 113, 154, 27, 127, 246,
    250, 1, 8, 198, 250, 209, 92, 222, 173, 21, 88, 102, 219,
];

/// Smooth 2D noise at (`x`, `y`), between 0 and 1. Lower `frequency` makes bigger, gentler
/// hills, and each extra octave adds finer detail on top.
pub fn perlin(x: f32, y: f32, frequency: f32, octaves: u32, seed: u64) -> f32 {
    if octaves == 0 {
        return 0.0;
    }
    let mut xa = x * frequency;
    let mut ya = y * frequency;
    let mut amp: f32 = 1.0;
    let mut fin: f32 = 0.0;
    let mut div: f32 = 0.0;

    for _ in 0..octaves {
        fin += octave(xa, ya, seed) * amp;
        div += 256.0 * amp; // the most this octave can add
        xa *= 2.0;
        ya *= 2.0;
        amp *= 0.5;
    }

    fin / div
}

/// One layer of noise, smoothly blended between the hashed values at the 4 surrounding corners
fn octave(x: f32, y: f32, seed: u64) -> f32 {
    // floor, not `as`, so negative coordinates don't fold back on themselves
    let x_int = x.floor() as i32;
    let y_int = y.floor() as i32;
    let x_frac = x - x_int as f32;
    let y_frac = y - y_int as f32;

    let s = corner(x_int, y_int, seed);
    let t = corner(x_int + 1, y_int, seed);
    let u = corner(x_int, y_int + 1, seed);
    let v = corner(x_int + 1, y_int + 1, seed);

    let low = smooth_lerp(s, t, x_frac);
    let high = smooth_lerp(u, v, x_frac);
    smooth_lerp(low, high, y_frac)
}

fn corner(x: i32, y: i32, seed: u64) -> f32 {
    // Only the low byte of the seed matters after the mask, so wrapping is fine
    let tmp = HASH[(y.wrapping_add(seed as i32) & 0xFF) as usize];
    HASH[((tmp as i32 + x) & 0xFF) as usize] as f32
}

/// Blend from `a` to `b`, easing in and out instead of moving at a constant rate
fn smooth_lerp(a: f32, b: f32, t: f32) -> f32 {
    let t = t * t * (3.0 - 2.0 * t);
    a + t * (b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_stays_in_range_and_repeats() {
        for i in -50..50 {
            let (x, y) = (i as f32 * 3.7, i as f32 * -1.3);
            let n = perlin(x, y, 0.05, 6, 7);
            assert!((0.0..=1.0).contains(&n), "perlin({x}, {y}) = {n}");
            assert_eq!(n, perlin(x, y, 0.05, 6, 7));
        }
    }

    #[test]
    fn perlin_uses_most_of_the_range() {
        for octaves in [1, 3] {
            let samples: Vec<f32> = (0..2000)
                .map(|i| perlin(i as f32 * 0.37, i as f32 * 0.11, 1.0, octaves, 2))
                .collect();
            let max = samples.iter().copied().fold(0.0, f32::max);
            let min = samples.iter().copied().fold(1.0, f32::min);
            assert!((0.85..1.0).contains(&max), "{octaves} octaves, max {max}");
            assert!((0.0..0.15).contains(&min), "{octaves} octaves, min {min}");
        }
    }

    #[test]
    fn no_octaves_is_flat() {
        assert_eq!(perlin(3.0, 4.0, 1.0, 0, 1), 0.0);
    }

    #[test]
    fn frequency_scales_the_coordinates() {
        for (x, y) in [(1.3, -2.7), (40.0, 12.5), (-8.2, -0.4)] {
            let scaled = perlin(x * 0.3, y * 0.3, 1.0, 4, 9);
            assert_eq!(perlin(x, y, 0.3, 4, 9), scaled, "at ({x}, {y})");
        }
    }

    #[test]
    fn each_octave_is_twice_as_fine_and_half_as_strong() {
        let (x, y) = (5.3, -1.7);
        let layers = octave(x, y, 4)
            + 0.5 * octave(2.0 * x, 2.0 * y, 4)
            + 0.25 * octave(4.0 * x, 4.0 * y, 4);
        // Each layer's corners go up to 255, so this keeps the total under 1
        let expected = layers / (256.0 + 128.0 + 64.0);
        assert!((perlin(x, y, 1.0, 3, 4) - expected).abs() < 1e-6);
    }

    #[test]
    fn corners_come_from_the_hash_and_the_seed_moves_along_y() {
        assert_eq!(corner(0, 0, 0), 42.0);
        assert_eq!(corner(3, 5, 2), 77.0);
        assert_eq!(corner(-4, 7, 11), 250.0);
        assert_eq!(corner(200, -30, 5), 8.0);
        for (x, y, seed) in [(3, 5, 2), (-4, 7, 11), (17, 0, 100)] {
            assert_eq!(corner(x, y, seed), corner(x, y + seed as i32, 0));
        }
    }

    #[test]
    fn a_seed_can_be_any_u64() {
        for seed in [0, 1, u64::from(u32::MAX), u64::MAX] {
            let n = perlin(3.3, -4.4, 0.1, 3, seed);
            assert!((0.0..=1.0).contains(&n), "seed {seed} gave {n}");
        }
    }

    #[test]
    fn smooth_lerp_eases_in_and_out() {
        assert_eq!(smooth_lerp(10.0, 20.0, 0.0), 10.0);
        assert_eq!(smooth_lerp(10.0, 20.0, 0.25), 11.5625);
        assert_eq!(smooth_lerp(10.0, 20.0, 0.5), 15.0);
        assert_eq!(smooth_lerp(10.0, 20.0, 1.0), 20.0);
    }

    #[test]
    fn perlin_is_smooth_across_zero() {
        // Neighbors on either side of the origin should be close, not mirrored jumps
        let step = 0.01;
        let left = perlin(-step, 0.5, 1.0, 1, 3);
        let right = perlin(step, 0.5, 1.0, 1, 3);
        assert!((left - right).abs() < 0.01, "{left} vs {right}");
    }
}
