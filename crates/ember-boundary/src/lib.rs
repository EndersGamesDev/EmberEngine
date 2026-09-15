//! Compiler-owned descriptions of Rust values crossing a script boundary.

#[allow(unused_extern_crates)]
extern crate self as ember_boundary;

#[cfg(any(feature = "models", test))]
pub mod models;
#[cfg(any(feature = "schema", test))]
mod schema;

#[cfg(feature = "derive")]
pub use ember_boundary_derive::Boundary;
#[cfg(any(feature = "schema", test))]
pub use schema::json_schema;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Which side of a script boundary supplies a value.
pub enum Direction {
    /// A script supplies the value to Rust.
    Input,
    /// Rust supplies the value to a script.
    Output,
    /// The value is read and written on both sides.
    Both,
}

/// The directional view selected while rendering a descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// Fields accepted while deserializing.
    Input,
    /// Fields emitted while serializing or lowering to a script object.
    Output,
}

/// A JSON-compatible value type.
#[derive(Clone, Copy, Debug)]
pub enum TypeRef {
    Bool,
    String,
    Integer {
        signed: bool,
        bits: u8,
        /// The declared script representation for a 64-bit integer.
        wide: Option<Wide>,
    },
    Number {
        bits: u8,
    },
    Nullable(&'static Self),
    Sequence(&'static Self),
    Array(&'static Self, usize),
    Tuple(&'static [Self]),
    Named {
        name: &'static str,
        describe: fn() -> &'static Description,
    },
}

/// How a 64-bit integer is represented at a script boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wide {
    /// Preserve integer identity with a JavaScript `bigint`.
    Exact,
    /// Use a JavaScript `number` within this field-specific precision bound.
    Precise {
        /// The engineering bound that keeps every value exactly representable.
        bound: &'static str,
    },
}

/// One object field and its directional presence rules.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: &'static str,
    pub ty: TypeRef,
    /// Serde accepts this field as absent while deserializing.
    pub input_optional: bool,
    /// The script lowering omits this field instead of emitting null.
    pub output_optional: bool,
}

/// How one enum variant contributes JSON.
#[derive(Clone, Copy, Debug)]
pub enum VariantShape {
    Unit,
    Object(&'static [Field]),
    /// Merge the named payload object with the outer enum tag.
    Intersection(TypeRef),
}

/// One enum variant after Serde casing has been applied.
#[derive(Clone, Copy, Debug)]
pub struct Variant {
    pub name: &'static str,
    pub shape: VariantShape,
}

/// The JSON shape owned by one Rust type.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Object(&'static [Field]),
    Enum {
        tag: Option<&'static str>,
        variants: &'static [Variant],
    },
}

/// Stable metadata emitted beside a boundary type by the derive.
#[derive(Clone, Copy, Debug)]
pub struct Description {
    pub name: &'static str,
    pub direction: Direction,
    /// Whether Serde rejects unknown object keys while deserializing.
    pub deny_unknown_fields: bool,
    pub shape: Shape,
}

/// Implemented only by shapes accepted by the registration derive.
pub trait Boundary: 'static {
    const DESCRIPTION: Description;
    /// Every possible key at this type's outer object level.
    const OBJECT_KEYS: &'static [&'static str];
}

/// Resolves a named reference while retaining a compile-time trait bound.
#[must_use]
pub const fn describe<T: Boundary>() -> &'static Description {
    &T::DESCRIPTION
}

/// Builds a static slice from an explicit list of derived boundary types.
#[macro_export]
macro_rules! boundary_descriptions {
    ($($ty:ty),+ $(,)?) => {{
        const DESCRIPTIONS: &[&$crate::Description] = &[
            $(&<$ty as $crate::Boundary>::DESCRIPTION),+
        ];
        DESCRIPTIONS
    }};
}

/// Whether a derived object can collide with an enclosing enum tag.
#[must_use]
pub const fn has_key(keys: &[&str], needle: &str) -> bool {
    let mut index = 0;
    while index < keys.len() {
        if const_str_eq(keys[index], needle) {
            return true;
        }
        index += 1;
    }
    false
}

/// Proves that a marked newtype can be merged with an enclosing enum tag.
///
/// # Panics
///
/// Const evaluation fails when the payload is not object-like or owns the
/// enclosing tag key.
pub const fn assert_intersection<T: Boundary>(tag: &str) {
    match T::DESCRIPTION.shape {
        Shape::Object(_) | Shape::Enum { tag: Some(_), .. } => {
            assert!(
                !has_key(T::OBJECT_KEYS, tag),
                "map-like newtype intersection collides with its outer tag key"
            );
        }
        Shape::Enum { tag: None, .. } => {
            panic!("boundary(intersection) payload must describe an object or tagged enum");
        }
    }
}

const fn const_str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// A renderer-owned collection populated only through typed registrations.
#[derive(Default)]
pub struct Registry {
    entries: Vec<&'static Description>,
}

impl Registry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Registers the description emitted for `T`.
    pub fn register<T: Boundary>(&mut self) {
        self.entries.push(&T::DESCRIPTION);
    }

    /// Returns the descriptions in registration order.
    #[must_use]
    pub fn entries(&self) -> &[&'static Description] {
        &self.entries
    }
}

impl Description {
    /// Number of wire variants, or zero for a struct.
    #[must_use]
    pub const fn variant_count(&self) -> usize {
        match self.shape {
            Shape::Object(_) => 0,
            Shape::Enum { variants, .. } => variants.len(),
        }
    }
}
