use ember_boundary_derive::Boundary;

#[derive(Boundary)]
#[boundary(direction = "output")]
#[serde(tag = "kind")]
enum Renamed {
    #[serde(rename = "other")]
    Original,
}

fn main() {}
