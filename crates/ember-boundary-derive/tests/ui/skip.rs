use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "output")]
struct Skipped {
    #[serde(skip)]
    hidden: String,
}

fn main() {}
