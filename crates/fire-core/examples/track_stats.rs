//! Print the circuit's measured geometry. `cargo run -p fire-core --example track_stats`
// Printing measurements is the sole purpose of this command-line example.
#![allow(clippy::print_stdout)]

fn main() {
    let t = fire_core::castle::track();
    println!("lap length      : {:.1} m", t.length());
    println!("half width      : {:.1} m", t.half_width());
    println!("min corner radius: {:.1} m", t.min_curvature_radius());
    println!("self-intersects : {:?}", t.self_intersection());
    println!("centreline pts  : {}", t.centreline().len());
    let r = fire_core::sim::Race::new(fire_core::castle::track(), 8, 3);
    println!("grid slots      : {}", r.racers.len());
}
