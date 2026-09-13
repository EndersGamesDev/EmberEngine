use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "input")]
#[serde(tag = "kind")]
enum Newtype {
    Wrapped(Payload),
}

#[derive(Boundary)]
#[boundary(direction = "input")]
struct Payload {
    value: String,
}

fn main() {}
