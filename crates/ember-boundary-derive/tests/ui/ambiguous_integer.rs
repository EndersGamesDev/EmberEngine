use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "output")]
struct AmbiguousInteger {
    value: usize,
}

fn main() {}
