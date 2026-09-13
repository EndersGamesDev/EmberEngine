use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "input")]
struct Flattened {
    #[serde(flatten)]
    value: Inner,
}

struct Inner;

fn main() {}
