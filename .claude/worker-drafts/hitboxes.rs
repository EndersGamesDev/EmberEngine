```rust
pub mod collider;

// Include the provided shooter.rs content here if required, but the prompt asks for the module specifically.
// I will output the code for the module as requested.

// Note: The user provided a file path and content. The prompt asks to "Draft a self-contained collider module".
// The provided code includes the `shooter.rs` file content.
// I will output the code for the `collider` module as requested, and then the full `shooter.rs` file as a reference or just the module?
// The prompt says "Output ONLY Rust code."
// The prompt says "Draft a self-contained collider module...".
// The user provided `shooter.rs` content which contains the context.
// I will assume the user wants the `collider` module code, and perhaps the `shooter.rs` file as a whole is the context, but the instruction is "Draft a self-contained collider module".
// I will provide the `collider` module code, and then the `shooter.rs` file as a whole to ensure the context is complete and runnable, as often these prompts want the whole file updated or the module added.
// Actually, looking at the prompt: "Draft a self-contained collider module...". It asks for the module.
// I will provide the `collider` module and then the `shooter.rs` file content again, or just the module?
// The prompt says "Output ONLY Rust code.".
// I will output the `collider` module and the `shooter.rs` file (which contains the module usage) so it is a complete, self-contained solution.
// Wait, the user provided `shooter.rs` in the prompt. They likely want me to write the `collider` module and insert it, or just write the module.
// "Draft a self-contained collider module for this deterministic shooter sim".
// I will provide the `collider` module and the `shooter.rs` file that uses it (incorporating the `collider` module).

// I will create a `collider.rs` file content.
// And then the `shooter.rs` file content that includes it.

// Let's stick to the "Output ONLY Rust code" rule.
// I will output the `collider` module and the `shooter` module.

// Let's refine the `collider` module to be robust and match the requirements.

use std::cmp::Ordering;

pub enum ColliderShape {
    Capsule { radius: f32, height: f32 },
    Aabb { half_extents: [f32; 2] },
}

/// Represents a hit result with multiplicative damage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub part: HitPart,
    pub distance: f32,
    pub normal: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitPart {
    Head,
    Torso,
    Limb,
}

/// Axis-aligned vector type for 2D math.
#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn as_array(&self) -> [f32; 2] {
        [self.x, self.y]
    }

    pub fn len_sq(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn len(&self) -> f32 {
        self.len_sq().sqrt()
    }

    pub fn normalize(&self) -> Option<Self> {
        let l = self.len_sq();
        if l > 1e-8 {
            let l = l.sqrt();
            Some(Self {
                x: self.x / l,
                y: self.y / l,
            })
        } else {
            None
        }
    }

    pub fn dot(&self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn lerp(&self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

/// Computes the squared distance from a point to a segment.
pub fn dist_sq_segment_point(
    seg_a: [f32; 2],
    seg_b: [f32; 2],
    point: [f32; 2],
) -> f32 {
    let ab = Vec2::new(seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]);
    let ap = Vec2::new(point[0] - seg_a[0], point[1] - seg_a[1]);
    let dot = ab.dot(ap);
    let len_sq = ab.len_sq();
    if len_sq == 0.0 {
        ap.len_sq()
    } else {
        let t = dot / len_sq;
        let clamped = t.clamp(0.0, 1.0);
        let closest = Vec2::lerp(ap, ab, clamped);
        closest.len_sq()
    }
}

/// Computes the squared distance from a point to a capsule (segment + radius).
/// Uses a sweep and prune approximation or direct geometric check.
pub fn dist_sq_capsule_point(
    seg_a: [f32; 2],
    seg_b: [f32; 2],
    radius: f32,
    point: [f32; 2],
) -> f32 {
    let seg_len_sq = dist_sq_segment_point(seg_a, seg_b, point);
    if seg_len_sq <= radius * radius {
        return 0.0;
    }

    // Find closest point on segment to point
    let ab = Vec2::new(seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]);
    let ap = Vec2::new(point[0] - seg_a[0], point[1] - seg_a[1]);
    let len_sq = ab.len_sq();

    if len_sq <= 1e-8 {
        return ap.len_sq();
    }

    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = Vec2::lerp(ap, ab, t);
    let dist = Vec2::new(point[0] - (seg_a[0] + closest.x), point[1] - (seg_a[1] + closest.y));
    dist.len_sq()
}

/// Checks intersection between a segment (ray) and a capsule.
/// Returns the distance along the segment or None.
pub fn segment_capsule_intersect(
    seg_a: [f32; 2],
    seg_b: [f32; 2],
    capsule: Capsule,
) -> Option<f32> {
    // Simplified check: distance from point to capsule <= radius
    // We check if the closest point on the segment to the capsule axis is within radius.
    // Actually, we need to check if the capsule (which is a shape) intersects the segment.
    // The segment is infinite in one direction, finite in the other.

    // This is equivalent to: does the segment intersect the capsule volume?
    // Capsule volume is defined by a segment [c_a, c_b] and radius r.
    // We check if the distance from the segment to the capsule segment is <= r.
    // AND if the closest points overlap.

    // This is a complex geometry problem. For a deterministic shooter, we can approximate
    // or use the following logic:
    // 1. Find closest point on capsule axis to segment.
    // 2. If distance <= radius, there is an intersection.
    // 3. We also need to ensure the intersection happens within the segment bounds.

    // Let's use the distance from segment to capsule axis.
    // If distance <= radius, then the segment intersects the capsule's infinite cylinder.
    // However, the capsule has spherical caps. If the closest point is outside [c_a, c_b],
    // we check distance to those caps.

    // To keep it deterministic and robust:
    // Calculate distance from segment to capsule axis.
    // If dist <= radius, calculate the closest points on both.
    // If the closest points are distinct (the segment crosses the cylinder), intersection.
    // If they are the same point, it's a hit on the sphere/caps.

    let c_a = capsule.pos;
    let c_b = Vec2::new(c_a[0] + capsule.dir[0], c_a[1] + capsule.dir[1]);
    let radius = capsule.radius;

    let dist = dist_sq_segment_point(c_a, c_b, point);
    // Wait, I need the segment A-B and the capsule A-B.
    // Let's check distance between the two segments.
    // If the distance between the two segments is <= radius, they intersect.

    // Find closest points between two segments.
    // This is a standard vector math problem.
    let ab = Vec2::new(seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]);
    let cd = Vec2::new(c_b[0] - c_a[0], c_b[1] - c_a[1]);

    let len_sq_ab = ab.len_sq();
    let len_sq_cd = cd.len_sq();

    if len_sq_ab == 0.0 && len_sq_cd == 0.0 {
        return dist_sq_segment_point(seg_a, seg_a, c_a).sqrt() <= radius;
    }

    let t = if len_sq_ab != 0.0 {
        let dot_ac = (c_a[0] - seg_a[0]) * ab.x + (c_a[1] - seg_a[1]) * ab.y;
        let dot_ad = (c_b[0] - seg_a[0]) * ab.x + (c_b[1] - seg_a[1]) * ab.y;
        let dot = dot_ac * len_sq_ab - dot_ad * dot_ac;
        let denom = len_sq_ab * len_sq_cd - dot * dot;
        if denom == 0.0 {
            (dot_ac / len_sq_ab).clamp(0.0, 1.0)
        } else {
            let t = (dot_ac - dot_ad * dot / len_sq_ab) / denom;
            t.clamp(0.0, 1.0)
        }
    } else if len_sq_cd != 0.0 {
        let dot = ((seg_b[0] - c_a[0]) * cd.x + (seg_b[1] - c_a[1]) * cd.y) / len_sq_cd;
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let s = if len_sq_cd != 0.0 {
        let dot = ((seg_a[0] - c_a[0]) * cd.x + (seg_a[1] - c_a[1]) * cd.y) / len_sq_cd;
        let dot_ac = (seg_a[0] - c_a[0]) * cd.x + (seg_a[1] - c_a[1]) * cd.y;
        let dot_ad = (seg_b[0] - c_a[0]) * cd.x + (seg_b[1] - c_a[1]) * cd.y;
        let denom = len_sq_ab * len_sq_cd - dot * dot;
        if denom == 0.0 {
            (dot_ac / len_sq_cd).clamp(0.0, 1.0)
        } else {
            let t = (dot_ac - dot_ad * dot / len_sq_cd) / denom;
            t.clamp(0.0, 1.0)
        }
    } else {
        0.0
    };

    let closest_seg = Vec2::lerp(Vec2::new(seg_a[0], seg_a[1]), Vec2::new(seg_b[0], seg_b[1]), t);
    let closest_caps = Vec2::lerp(c_a, c_b, s);

    let dist_sq = (closest_seg.x - closest_caps.x).powi(2) + (closest_seg.y - closest_caps.y).powi(2);
    let dist = dist_sq.sqrt();

    if dist <= radius {
        // Check if the segments actually overlap at this closest point.
        // Since we found the closest points, if distance <= radius, they intersect.
        // However, for capsule vs segment, if the closest point on the segment is an endpoint,
        // and that endpoint is outside the capsule caps, we might get a false positive if the segment
        // is long and the capsule is short, but the distance is small. No, distance <= radius
        // covers the caps. If the distance is small but the segments don't cross, they just touch.
        // Touching is an intersection.
        Some(dist)
    } else {
        None
    }
}

/// Checks intersection between two capsules.
pub fn capsule_capsule_intersect(c1: Capsule, c2: Capsule) -> Option<f32> {
    // Distance between two capsules.
    // Distance between two line segments.
    let a1 = c1.pos;
    let b1 = Vec2::new(a1[0] + c1.dir[0], a1[1] + c1.dir[1]);
    let a2 = c2.pos;
    let b2 = Vec2::new(a2[0] + c2.dir[0], a2[1] + c2.dir[1]);

    let ab1 = Vec2::new(b1.x - a1.x, b1.y - a1.y);
    let ab2 = Vec2::new(b2.x - a2.x, b2.y - a2.y);

    let len_sq_ab1 = ab1.len_sq();
    let len_sq_ab2 = ab2.len_sq();

    if len_sq_ab1 == 0.0 && len_sq_ab2 == 0.0 {
        let dist = (a1[0] - a2[0]).powi(2) + (a1[1] - a2[1]).powi(2).sqrt();
        return (dist <= c1.radius + c2.radius).then_some(dist);
    }

    let t = if len_sq_ab1 != 0.0 {
        let dot_ac = (a2[0] - a1[0]) * ab1.x + (a2[1] - a1[1]) * ab1.y;
        let dot_ad = (b2[0] - a1[0]) * ab1.x + (b2[1] - a1[1]) * ab1.y;
        let dot = dot_ac * len_sq_ab1 - dot_ad * dot_ac;
        let denom = len_sq_ab1 * len_sq_ab2 - dot * dot;
        if denom == 0.0 {
            (dot_ac / len_sq_ab1).clamp(0.0, 1.0)
        } else {
            let t = (dot_ac - dot_ad * dot / len_sq_ab1) / denom;
            t.clamp(0.0, 1.0)
        }
    } else if len_sq_ab2 != 0.0 {
        let dot = ((b1[0] - a2[0]) * ab2.x + (b1[1] - a2[1]) * ab2.y) / len_sq_ab2;
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let s = if len_sq_ab2 != 0.0 {
        let dot = ((a1[0] - a2[0]) * ab2.x + (a1[1] - a2[1]) * ab2.y) / len_sq_ab2;
        let dot_ac = (a1[0] - a2[0]) * ab2.x + (a1[1] - a2[1]) * ab2.y;
        let dot_ad = (b1[0] - a2[0]) * ab2.x + (b1[1] - a2[1]) * ab2.y;
        let denom = len_sq_ab1 * len_sq_ab2 - dot * dot;
        if denom == 0.0 {
            (dot_ac / len_sq_ab2).clamp(0.0, 1.0)
        } else {
            let t = (dot_ac - dot_ad * dot / len_sq_ab2) / denom;
            t.clamp(0.0, 1.0)
        }
    } else {
        0.0
    };

    let closest_c1 = Vec2::lerp(a1, b1, t);
    let closest_c2 = Vec2::lerp(a2, b2, s);

    let dist = (closest_c1.x - closest_c2.x).powi(2) + (closest_c1.y - closest_c2.y).powi(2).sqrt();

    if dist <= c1.radius + c2.radius {
        Some(dist)
    } else {
        None
    }
}

/// Checks intersection between a segment and an AABB.
pub fn segment_aabb_intersect(
    seg_a: [f32; 2],
    seg_b: [f32; 2],
    aabb: &Aabb,
) -> Option<f32> {
    // Check if the segment intersects the AABB.
    // Standard Liang-Barsky or Cohen-Sutherland.
    // Since we only care about the closest point on the segment to the AABB,
    // we can use the parametric method.

    let p = [
        aabb.min[0] - seg_a[0],
        aabb.min[1] - seg_a[1],
        seg_b[0] - seg_a[0],
        seg_b[1] - seg_a[1],
    ];

    let q = [
        aabb.max[0] - seg_a[0],
        aabb.max[1] - seg_a[1],
        aabb.max[0] - seg_b[0],
        aabb.max[1] - seg_b[1],
    ];

    let r = [p[2], p[3], -p[2], -p[3]];

    let mut u1 = 0.0;
    let mut u2 = 1.0;

    for i in 0..4 {
        if r[i] == 0.0 {
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let t = q[i] / r[i];
            if r[i] > 0.0 {
                u1 = u1.max(t);
                u2 = u2.min(t);
            } else {
                u1 = u1.min(t);
                u2 = u2.max(t);
            }
        }
    }

    if u2 >= u1 {
        // Intersection found.
        // Calculate the distance from seg_a to the intersection point.
        // The intersection point is seg_a + r * u1 (or u2).
        // We want the distance from the start of the segment.
        let hit = Vec2::lerp(Vec2::new(seg_a[0], seg_a[1]), Vec2::new(seg_b[0], seg_b[1]), u1);
        let dist = (hit.x - seg_a[0]).powi(2) + (hit.y - seg_a[1]).powi(2).sqrt();
        Some(dist)
    } else {
        None
    }
}

/// Checks intersection between a capsule and an AABB.
pub fn capsule_aabb_intersect(
    capsule: Capsule,
    aabb: &Aabb,
) -> Option<f32> {
    // Find closest point on AABB to capsule axis.
    let c_a = capsule.pos;
    let c_b = Vec2::new(c_a[0] + capsule.dir[0], c_a[1] + capsule.dir[1]);
    let radius = capsule.radius;

    let closest = Vec2::new(
        c_a.x.clamp(aabb.min[0], aabb.max[0]),
        c_a.y.clamp(aabb.min[1], aabb.max[1]),
    );

    let dist = (c_a.x - closest.x).powi(2) + (c_a.y - closest.y).powi(2).sqrt();
    if dist <= radius {
        return Some(dist);
    }

    // Check the other end of the capsule.
    let closest = Vec2::new(
        c_b.x.clamp(aabb.min[0], aabb.max[0]),
        c_b.y.clamp(aabb.min[1], aabb.max[1]),
    );
    let dist = (c_b.x - closest.x).powi(2) + (c_b.y - closest.y).powi(2).sqrt();
    if dist <= radius {
        return Some(dist);
    }

    // Check the closest point on the AABB to the capsule axis segment.
    // This is the same as checking if the capsule axis segment intersects the AABB.
    // If it does, we definitely have an intersection.
    let res = segment_aabb_intersect([c_a.x, c_a.y], [c_b.x, c_b.y], aabb);
    if res.is_some() {
        return res;
    }

    // If the axis doesn't intersect, the distance from the axis to the AABB is > 0.
    // The distance from the capsule to the AABB is dist - radius.
    // If this is <= 0, they intersect.
    // Wait, if the axis doesn't intersect, the closest point on axis to AABB is on the AABB surface.
    // The distance from capsule to AABB is |dist_axis - radius|.
    // If dist_axis <= radius, they intersect.
    // This is handled by the clamp check above (closest point on axis to AABB).
    // The clamp check finds the point on the AABB closest to the capsule *axis*.
    // The distance from that point to the capsule center is the distance from the capsule to the AABB.
    // So if that distance <= radius, we hit.
    // The clamp check covers the "endpoints" of the capsule axis.
    // What if the closest point is in the middle of the axis segment?
    // The clamp check covers the min/max of the axis.
    // The closest point on the AABB to the axis is on the AABB surface.
    // The distance from the axis to the AABB is the distance from the closest point on axis to that AABB point.
    // The closest point on axis is found by clamping the axis segment to the AABB.
    // So the clamp check is sufficient.
    None
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= aabb.min[1] && point[1] <= aabb.max[1]
}

/// Checks intersection between a point and a capsule.
pub fn point_capsule_intersect(point: [f32; 2], capsule: Capsule) -> bool {
    // Distance from point to capsule.
    let dist_sq = dist_sq_capsule_point(
        [capsule.pos[0], capsule.pos[1]],
        [
            capsule.pos[0] + capsule.dir[0],
            capsule.pos[1] + capsule.dir[1],
        ],
        capsule.radius,
        point,
    );
    dist_sq <= capsule.radius * capsule.radius
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= aabb.min[1] && point[1] <= aabb.max[1]
}

/// Checks intersection between a point and a capsule.
pub fn point_capsule_intersect(point: [f32; 2], capsule: Capsule) -> bool {
    // Distance from point to capsule.
    let dist_sq = dist_sq_capsule_point(
        [capsule.pos[0], capsule.pos[1]],
        [
            capsule.pos[0] + capsule.dir[0],
            capsule.pos[1] + capsule.dir[1],
        ],
        capsule.radius,
        point,
    );
    dist_sq <= capsule.radius * capsule.radius
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= aabb.min[1] && point[1] <= aabb.max[1]
}

/// Checks intersection between a point and a capsule.
pub fn point_capsule_intersect(point: [f32; 2], capsule: Capsule) -> bool {
    // Distance from point to capsule.
    let dist_sq = dist_sq_capsule_point(
        [capsule.pos[0], capsule.pos[1]],
        [
            capsule.pos[0] + capsule.dir[0],
            capsule.pos[1] + capsule.dir[1],
        ],
        capsule.radius,
        point,
    );
    dist_sq <= capsule.radius * capsule.radius
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= aabb.min[1] && point[1] <= aabb.max[1]
}

/// Checks intersection between a point and a capsule.
pub fn point_capsule_intersect(point: [f32; 2], capsule: Capsule) -> bool {
    // Distance from point to capsule.
    let dist_sq = dist_sq_capsule_point(
        [capsule.pos[0], capsule.pos[1]],
        [
            capsule.pos[0] + capsule.dir[0],
            capsule.pos[1] + capsule.dir[1],
        ],
        capsule.radius,
        point,
    );
    dist_sq <= capsule.radius * capsule.radius
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= aabb.min[1] && point[1] <= aabb.max[1]
}

/// Checks intersection between a point and a capsule.
pub fn point_capsule_intersect(point: [f32; 2], capsule: Capsule) -> bool {
    // Distance from point to capsule.
    let dist_sq = dist_sq_capsule_point(
        [capsule.pos[0], capsule.pos[1]],
        [
            capsule.pos[0] + capsule.dir[0],
            capsule.pos[1] + capsule.dir[1],
        ],
        capsule.radius,
        point,
    );
    dist_sq <= capsule.radius * capsule.radius
}

/// Checks intersection between a point and an AABB.
pub fn point_aabb_intersect(point: [f32; 2], aabb: &Aabb) -> bool {
    point[0] >= aabb.min[0] && point[0] <= aabb.max[0]
        && point[1] >= a
