//! Pure comparison arithmetic for the live Mode C versus layer GPU oracle.

use serde::Serialize;

pub(crate) const SAMPLE_COUNT: usize = 8;
pub(crate) const RECORD_STRIDE: usize = 256;
pub(crate) const RECORDS_PER_SAMPLE: usize = 2;
pub(crate) const IMAGE_WIDTH: u32 = 64;
pub(crate) const IMAGE_HEIGHT: u32 = 36;
pub(crate) const IMAGE_BYTES_PER_ROW: u32 = 256;
pub(crate) const IMAGE_BYTES: usize = IMAGE_BYTES_PER_ROW as usize * IMAGE_HEIGHT as usize;
pub(crate) const RECORD_BYTES: usize = SAMPLE_COUNT * RECORDS_PER_SAMPLE * RECORD_STRIDE;
pub(crate) const F32_TOLERANCE: f32 = 4.0e-5;

/// Numeric comparison of two GPU-produced edge-pose sample sets.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct NumericComparison {
    pub(crate) sampled_indices: Vec<u32>,
    pub(crate) compared_records: usize,
    pub(crate) compared_components: usize,
    pub(crate) exact_components: usize,
    pub(crate) mismatched_components: usize,
    pub(crate) tolerance: f32,
    pub(crate) max_abs_error: f32,
    pub(crate) pass: bool,
}

/// Exact byte-level comparison of the two small-rung presentation images.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ImageComparison {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) compared_pixels: u32,
    pub(crate) mismatched_pixels: u32,
    pub(crate) mode_c_checksum: String,
    pub(crate) layer_checksum: String,
    pub(crate) pass: bool,
}

/// Chooses stable samples spanning the beginning, interior, and end of a delivered range.
#[must_use]
pub(crate) fn deterministic_indices(edges: u32) -> Vec<u32> {
    let candidates = [
        0,
        1,
        17,
        599,
        edges / 3,
        edges / 2,
        edges.saturating_sub(2),
        edges.saturating_sub(1),
    ];
    let mut indices = Vec::with_capacity(SAMPLE_COUNT);
    for index in candidates {
        if index < edges && !indices.contains(&index) {
            indices.push(index);
        }
    }
    indices
}

fn component(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("record readback ends before byte {}", offset + 4))?
        .try_into()
        .map_err(|_| "record component is not four bytes".to_string())?;
    Ok(f32::from_ne_bytes(raw))
}

/// Compares the first 16 bytes of every 256-byte-aligned copied record.
pub(crate) fn compare_records(
    mode_c: &[u8],
    layer: &[u8],
    indices: Vec<u32>,
) -> Result<NumericComparison, String> {
    let record_count = indices.len() * RECORDS_PER_SAMPLE;
    let required = record_count.saturating_mul(RECORD_STRIDE);
    if mode_c.len() < required || layer.len() < required {
        return Err(format!(
            "record readback requires {required} bytes, got Mode C {} and layer {}",
            mode_c.len(),
            layer.len()
        ));
    }
    let mut exact_components = 0;
    let mut mismatched_components = 0;
    let mut max_abs_error = 0.0_f32;
    for record in 0..record_count {
        for lane in 0..4 {
            let offset = record * RECORD_STRIDE + lane * 4;
            let left = component(mode_c, offset)?;
            let right = component(layer, offset)?;
            if left.to_bits() == right.to_bits() {
                exact_components += 1;
                continue;
            }
            let error = (left - right).abs();
            if !left.is_finite() || !right.is_finite() || error > F32_TOLERANCE {
                mismatched_components += 1;
            }
            max_abs_error = max_abs_error.max(error);
        }
    }
    Ok(NumericComparison {
        sampled_indices: indices,
        compared_records: record_count,
        compared_components: record_count * 4,
        exact_components,
        mismatched_components,
        tolerance: F32_TOLERANCE,
        max_abs_error,
        pass: mismatched_components == 0,
    })
}

fn checksum(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// Compares two tightly packed 64-by-36 four-byte presentation images.
pub(crate) fn compare_images(mode_c: &[u8], layer: &[u8]) -> Result<ImageComparison, String> {
    if mode_c.len() != IMAGE_BYTES || layer.len() != IMAGE_BYTES {
        return Err(format!(
            "image readback requires {IMAGE_BYTES} bytes, got Mode C {} and layer {}",
            mode_c.len(),
            layer.len()
        ));
    }
    let mismatched_pixels = mode_c
        .chunks_exact(4)
        .zip(layer.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count() as u32;
    Ok(ImageComparison {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
        compared_pixels: IMAGE_WIDTH * IMAGE_HEIGHT,
        mismatched_pixels,
        mode_c_checksum: checksum(mode_c),
        layer_checksum: checksum(layer),
        pass: mismatched_pixels == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        F32_TOLERANCE, IMAGE_BYTES, RECORD_BYTES, RECORD_STRIDE, compare_images, compare_records,
        deterministic_indices,
    };

    fn records(values: &[[f32; 8]]) -> Vec<u8> {
        let mut bytes = vec![0_u8; values.len() * 2 * RECORD_STRIDE];
        for (sample, value) in values.iter().enumerate() {
            for (lane, component) in value.iter().enumerate() {
                let offset = (sample * 2 + lane / 4) * RECORD_STRIDE + lane % 4 * 4;
                bytes[offset..offset + 4].copy_from_slice(&component.to_ne_bytes());
            }
        }
        bytes
    }

    #[test]
    fn deterministic_samples_cover_both_ends_and_interior() {
        assert_eq!(RECORD_BYTES, 8 * 2 * RECORD_STRIDE);
        assert_eq!(
            deterministic_indices(3_000),
            [0, 1, 17, 599, 1_000, 1_500, 2_998, 2_999]
        );
        assert!(deterministic_indices(1).iter().all(|index| *index < 1));
    }

    #[test]
    fn numeric_comparison_distinguishes_tolerance_from_failure() {
        let base = [[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]];
        let mut near = base;
        near[0][6] += F32_TOLERANCE / 2.0;
        let comparison = compare_records(&records(&base), &records(&near), vec![17])
            .expect("complete aligned records compare");
        assert!(comparison.pass);
        assert_eq!(comparison.exact_components, 7);
        near[0][6] += F32_TOLERANCE * 2.0;
        assert!(
            !compare_records(&records(&base), &records(&near), vec![17])
                .expect("complete aligned records compare")
                .pass
        );
    }

    #[test]
    fn image_comparison_reports_exact_checksums_and_pixel_count() {
        let left = vec![7_u8; IMAGE_BYTES];
        let mut right = left.clone();
        let equal = compare_images(&left, &right).expect("fixed images compare");
        assert!(equal.pass);
        assert_eq!(equal.compared_pixels, 64 * 36);
        assert_eq!(equal.mode_c_checksum, equal.layer_checksum);
        right[4] ^= 1;
        let different = compare_images(&left, &right).expect("fixed images compare");
        assert!(!different.pass);
        assert_eq!(different.mismatched_pixels, 1);
        assert_ne!(different.mode_c_checksum, different.layer_checksum);
    }
}
