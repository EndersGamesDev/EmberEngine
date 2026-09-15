use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "output")]
struct UndeclaredWideInteger {
    value: u64,
}

fn main() {}
