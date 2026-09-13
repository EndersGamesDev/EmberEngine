use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "input")]
#[serde(untagged)]
enum Untagged {
    A { value: String },
    B { value: u32 },
}

fn main() {}
