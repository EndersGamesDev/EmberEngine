/// One signed or unsigned Rust discriminant rendered as a WGSL integer constant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WgslEnumDiscriminant {
    /// An `i32` discriminant.
    Signed(i32),
    /// A `u32` discriminant.
    Unsigned(u32),
}

/// One named value in a Rust enum's WGSL representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WgslEnumVariant {
    /// Rust variant name.
    pub name: &'static str,
    /// Discriminant read from the Rust variant.
    pub discriminant: WgslEnumDiscriminant,
}

/// Complete constant metadata for one Rust enum.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WgslEnumDescription {
    /// Prefix used to namespace the emitted WGSL constants.
    pub name: &'static str,
    /// Variants in declaration order.
    pub variants: &'static [WgslEnumVariant],
}

/// A Rust enum whose discriminants are available to WGSL templates.
pub trait WgslEnum: Copy + 'static {
    /// Static metadata from which templates emit WGSL constants.
    const DESCRIPTION: WgslEnumDescription;
}

/// Implements [`WgslEnum`](crate::WgslEnum) for an `i32`- or `u32`-represented enum.
///
/// Every listed value is cast directly from the Rust variant, so changing its discriminant changes
/// the shader description in the same compilation.
#[macro_export]
macro_rules! impl_wgsl_enum {
    ($rust_type:ty, $wgsl_name:literal, i32, { $($variant:ident),+ $(,)? }) => {
        impl $crate::WgslEnum for $rust_type {
            const DESCRIPTION: $crate::WgslEnumDescription = $crate::WgslEnumDescription {
                name: $wgsl_name,
                variants: &[
                    $(
                        $crate::WgslEnumVariant {
                            name: stringify!($variant),
                            discriminant: $crate::WgslEnumDiscriminant::Signed(
                                <$rust_type>::$variant as i32,
                            ),
                        },
                    )+
                ],
            };
        }
    };
    ($rust_type:ty, $wgsl_name:literal, u32, { $($variant:ident),+ $(,)? }) => {
        impl $crate::WgslEnum for $rust_type {
            const DESCRIPTION: $crate::WgslEnumDescription = $crate::WgslEnumDescription {
                name: $wgsl_name,
                variants: &[
                    $(
                        $crate::WgslEnumVariant {
                            name: stringify!($variant),
                            discriminant: $crate::WgslEnumDiscriminant::Unsigned(
                                <$rust_type>::$variant as u32,
                            ),
                        },
                    )+
                ],
            };
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{WgslEnum, WgslEnumDiscriminant};

    #[derive(Clone, Copy)]
    #[repr(u32)]
    enum TestMode {
        Preview = 2,
        Final = 9,
    }

    crate::impl_wgsl_enum!(TestMode, "TestMode", u32, { Preview, Final });

    #[test]
    fn enum_macro_reads_the_rust_discriminants() {
        let description = TestMode::DESCRIPTION;
        assert_eq!(description.name, "TestMode");
        assert_eq!(description.variants[0].name, "Preview");
        assert_eq!(
            description.variants[0].discriminant,
            WgslEnumDiscriminant::Unsigned(2),
        );
        assert_eq!(description.variants[1].name, "Final");
        assert_eq!(
            description.variants[1].discriminant,
            WgslEnumDiscriminant::Unsigned(9),
        );
    }
}
