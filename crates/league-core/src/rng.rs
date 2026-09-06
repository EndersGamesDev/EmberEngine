//! Deterministic randomness: stateless hashes, never a stored generator.
//!
//! The repo rule is that the only randomness must be tick-indexed so replays
//! and rollback stay possible; league has no RNG state at all. Every roll is
//! `roll(seed, tick, who, salt)` — same game, same tick, same answer.

/// SplitMix-style 64-bit mixer.
const fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The one hash: every random-looking decision in the sim comes from it.
#[must_use]
pub const fn hash(seed: u64, tick: u64, who: u64, salt: u64) -> u64 {
    mix64(seed
        .wrapping_mul(0x517c_c1b7_2722_0a95)
        ^ tick.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ who.wrapping_mul(0xd6e8_feb8_6659_fd93)
        ^ salt.wrapping_mul(0x85eb_ca77_f3ac_7cf9))
}

/// A uniform f32 in `0..1`.
#[must_use]
pub fn unit(seed: u64, tick: u64, who: u64, salt: u64) -> f32 {
    // 24 bits of mantissa is all an f32 can distinguish anyway.
    (hash(seed, tick, who, salt) >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_inputs_stable_output() {
        assert_eq!(hash(1, 2, 3, 4), hash(1, 2, 3, 4));
        assert_ne!(hash(1, 2, 3, 4), hash(1, 2, 3, 5));
        assert_ne!(hash(1, 2, 3, 4), hash(2, 2, 3, 4));
    }

    #[test]
    fn unit_stays_in_range_and_moves() {
        let a = unit(7, 100, 3, 1);
        let b = unit(7, 101, 3, 1);
        assert!((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b));
        assert!((a - b).abs() > 1e-6);
    }

    #[test]
    fn rolls_are_not_suspiciously_lumpy() {
        let mut under = 0;
        for t in 0..4000 {
            if unit(31, t, 1, 0) < 0.25 {
                under += 1;
            }
        }
        assert!((800..1200).contains(&under), "quarter-band count {under}");
    }
}
