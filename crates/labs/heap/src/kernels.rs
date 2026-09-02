//! Fixed benchmark workloads and their CPU address/color oracle.

// The shader oracle intentionally mirrors fixed-width integer and f32 substrate conversions.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

#[cfg(test)]
use crate::Descriptor;

pub(super) const FETCH_WIDTH: u32 = 512;
pub(super) const FETCH_HEIGHT: u32 = 512;
pub(super) const PAYLOAD_SIDE: u32 = 64;
pub(super) const FETCHES_PER_FRAGMENT: u32 = 16;
pub(super) const DRAW_STEPS: [u32; 5] = [16, 64, 256, 1_024, 4_096];

pub(super) const DIRECT_FETCH_SHADER: &str = include_str!("fetch-direct.wgsl");
pub(super) const HEAP_FETCH_SHADER_TEMPLATE: &str = include_str!("fetch-heap.wgsl");
pub(super) const HEAP_DRAW_SHADER_TEMPLATE: &str = include_str!("draw-heap.wgsl");
pub(super) const TRADITIONAL_DRAW_SHADER: &str = include_str!("draw-traditional.wgsl");

pub(super) fn heap_shader(template: &str, capacity: u32) -> String {
    template.replace("__DESCRIPTOR_CAPACITY__", &capacity.to_string())
}

pub(super) fn payload_texel(x: u32, y: u32) -> [f32; 4] {
    let seed = x
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(y.wrapping_mul(0x85eb_ca6b));
    [
        (seed & 255) as f32 / 255.0,
        ((seed >> 8) & 255) as f32 / 255.0,
        ((seed >> 16) & 255) as f32 / 255.0,
        1.0,
    ]
}

#[cfg(test)]
const fn fetch_coordinate(pixel_x: u32, pixel_y: u32, fetch: u32) -> (u32, u32) {
    let seed = pixel_x
        .wrapping_mul(1_664_525)
        .wrapping_add(pixel_y.wrapping_mul(1_013_904_223))
        .wrapping_add(fetch.wrapping_mul(747_796_405));
    (
        (seed ^ (seed >> 16)) & 63,
        ((seed >> 6) ^ (seed >> 19)) & 63,
    )
}

#[cfg(test)]
fn direct_reference(pixel_x: u32, pixel_y: u32) -> [f32; 4] {
    let mut sum = [0.0_f32; 4];
    for fetch in 0..FETCHES_PER_FRAGMENT {
        let (x, y) = fetch_coordinate(pixel_x, pixel_y, fetch);
        let value = payload_texel(x, y);
        for (output, component) in sum.iter_mut().zip(value) {
            *output += component;
        }
    }
    sum.map(|value| (value * 0.031_25).fract().abs())
}

#[cfg(test)]
fn heap_reference(pixel_x: u32, pixel_y: u32, descriptor: Descriptor) -> [f32; 4] {
    let mut sum = [0.0_f32; 4];
    for fetch in 0..FETCHES_PER_FRAGMENT {
        let (logical_x, logical_y) = fetch_coordinate(pixel_x, pixel_y, fetch);
        let x = logical_x % u32::from(descriptor.width);
        let y = logical_y % u32::from(descriptor.height);
        let value = payload_texel(x, y);
        for (output, component) in sum.iter_mut().zip(value) {
            *output += component;
        }
    }
    sum.map(|value| (value * 0.031_25).fract().abs())
}

pub(super) const fn material_color(index: u32) -> [u8; 4] {
    let value = index.wrapping_mul(0x9e37_79b9).rotate_left(11);
    [
        48 + (value & 0xbf) as u8,
        48 + ((value >> 8) & 0xbf) as u8,
        48 + ((value >> 16) & 0xbf) as u8,
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        DIRECT_FETCH_SHADER, DRAW_STEPS, FETCH_HEIGHT, FETCH_WIDTH, FETCHES_PER_FRAGMENT,
        HEAP_DRAW_SHADER_TEMPLATE, HEAP_FETCH_SHADER_TEMPLATE, PAYLOAD_SIDE,
        TRADITIONAL_DRAW_SHADER, direct_reference, heap_reference, heap_shader, material_color,
    };
    use crate::{Descriptor, HeapKind};

    fn validate_shader(source: &str) {
        let module = naga::front::wgsl::parse_str(source).expect("fixed WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("fixed WGSL validates");
    }

    #[test]
    fn benchmark_shaders_parse_and_validate() {
        validate_shader(DIRECT_FETCH_SHADER);
        validate_shader(&heap_shader(HEAP_FETCH_SHADER_TEMPLATE, 1_024));
        validate_shader(&heap_shader(HEAP_DRAW_SHADER_TEMPLATE, 1_024));
        validate_shader(TRADITIONAL_DRAW_SHADER);
    }

    #[test]
    fn direct_and_descriptor_addressing_match_cpu_reference() {
        let descriptor = Descriptor {
            layer: 3,
            x: 17,
            y: 29,
            width: 64,
            height: 64,
            kind: HeapKind::Data,
        };
        for (x, y) in [(0, 0), (1, 511), (255, 257), (511, 511)] {
            assert_eq!(direct_reference(x, y), heap_reference(x, y, descriptor));
        }
        assert_eq!(
            u64::from(FETCH_WIDTH) * u64::from(FETCH_HEIGHT) * u64::from(FETCHES_PER_FRAGMENT),
            4_194_304
        );
        assert_eq!(PAYLOAD_SIDE, 64);
        assert_eq!(DRAW_STEPS, [16, 64, 256, 1_024, 4_096]);
        assert_ne!(material_color(1), material_color(2));
    }
}
