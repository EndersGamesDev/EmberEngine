use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "output")]
struct CustomSerializer {
    #[serde(serialize_with = "as_text")]
    value: u32,
}

fn as_text() {}
fn main() {}
