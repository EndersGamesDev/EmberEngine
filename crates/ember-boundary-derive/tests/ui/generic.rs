use ember_boundary_derive::Boundary;

struct Wrapper<T>(T);

#[derive(Boundary)]
#[boundary(direction = "output")]
struct GenericField {
    value: Wrapper<u8>,
}

fn main() {}
