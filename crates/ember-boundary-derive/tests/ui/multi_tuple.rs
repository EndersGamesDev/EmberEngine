use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "input")]
#[serde(tag = "kind")]
enum MultiTuple {
    Pair(u8, u8),
}

fn main() {}
