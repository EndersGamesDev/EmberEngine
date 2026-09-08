//! Shared castle measurements. Rendering and traversal consume the same solids.
//! Coordinates are metres, +Y up; the dungeon exit meets (0, 0, -7).
use std::sync::OnceLock;

pub const PLAYER_RADIUS: f32 = 0.22;
pub const PLAYER_HEIGHT: f32 = 1.70;
pub const STEP_HEIGHT: f32 = 0.28;
const EPS: f32 = 0.0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Hall,
    GreatRoom,
    GrandStair,
    Garden,
    WallWalk,
    Tower,
    Lookout,
}
impl Zone {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hall => "Lower passage",
            Self::GreatRoom => "Great hall",
            Self::GrandStair => "Grand stair",
            Self::Garden => "Backyard garden",
            Self::WallWalk => "Castle walls",
            Self::Tower => "Tower",
            Self::Lookout => "Tower summit",
        }
    }
    pub const fn bit(self) -> u32 {
        1 << self as u32
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Stone,
    Paving,
    Grass,
    Iron,
    Timber,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolidKind {
    Floor,
    Wall,
    Ceiling,
    Step,
    Landing,
    Rail,
    PropCollider,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}
impl Aabb {
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }
    pub fn center(self) -> [f32; 3] {
        std::array::from_fn(|i| (self.min[i] + self.max[i]) * 0.5)
    }
    pub fn size(self) -> [f32; 3] {
        std::array::from_fn(|i| self.max[i] - self.min[i])
    }
    fn overlaps_disk(self, x: f32, z: f32, r: f32) -> bool {
        let dx = x - x.clamp(self.min[0], self.max[0]);
        let dz = z - z.clamp(self.min[2], self.max[2]);
        dx * dx + dz * dz < r * r - EPS * EPS
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Solid {
    pub name: &'static str,
    pub index: u16,
    pub bounds: Aabb,
    pub material: Surface,
    pub zone: Zone,
    pub kind: SolidKind,
}
#[derive(Clone, Copy, Debug)]
pub struct PropPlacement {
    pub name: &'static str,
    pub position: [f32; 3],
    pub scale: f32,
    pub yaw: f32,
    pub zone: Zone,
}
#[derive(Clone, Debug, Default)]
pub struct Castle {
    pub solids: Vec<Solid>,
    pub props: Vec<PropPlacement>,
}
impl Castle {
    fn add(
        &mut self,
        name: &'static str,
        lo: [f32; 3],
        hi: [f32; 3],
        zone: Zone,
        kind: SolidKind,
        material: Surface,
    ) {
        assert!((0..3).all(|i| lo[i] < hi[i]));
        self.solids.push(Solid {
            name,
            index: self.solids.len() as u16,
            bounds: Aabb::new(lo, hi),
            zone,
            kind,
            material,
        });
    }
    fn stone(
        &mut self,
        name: &'static str,
        lo: [f32; 3],
        hi: [f32; 3],
        zone: Zone,
        kind: SolidKind,
    ) {
        self.add(name, lo, hi, zone, kind, Surface::Stone)
    }
    fn slab(
        &mut self,
        name: &'static str,
        x: [f32; 2],
        z: [f32; 2],
        y: f32,
        zone: Zone,
        kind: SolidKind,
    ) {
        self.add(
            name,
            [x[0], y - 0.4, z[0]],
            [x[1], y, z[1]],
            zone,
            kind,
            Surface::Paving,
        )
    }
    fn flight(
        &mut self,
        name: &'static str,
        x: [f32; 2],
        start_z: f32,
        end_z: f32,
        base: f32,
        rise: f32,
        steps: usize,
        zone: Zone,
    ) {
        for i in 0..steps {
            let a = start_z + (end_z - start_z) * i as f32 / steps as f32;
            let b = start_z + (end_z - start_z) * (i + 1) as f32 / steps as f32;
            let top = base + rise * (i + 1) as f32 / steps as f32;
            self.stone(
                name,
                [x[0], base, a.min(b)],
                [x[1], top, a.max(b)],
                zone,
                SolidKind::Step,
            );
            for edge in [[x[0] - 0.10, x[0]], [x[1], x[1] + 0.10]] {
                self.add(
                    "stair.guardrail",
                    [edge[0], top, a.min(b)],
                    [edge[1], top + 1.10, a.max(b)],
                    zone,
                    SolidKind::Rail,
                    Surface::Iron,
                );
            }
        }
    }
    /// The viewing gap is 0.9 m: visible ironwork prevents even a crouched body
    /// fitting above the stone parapet. This also blocks stair-rail jump routes.
    fn viewing_fence(&mut self, x: [f32; 2], z: [f32; 2], parapet: f32, zone: Zone) {
        self.add(
            "parapet.toprail",
            [x[0], parapet + 0.9, z[0]],
            [x[1], parapet + 1.05, z[1]],
            zone,
            SolidKind::Rail,
            Surface::Iron,
        );
        let along_x = x[1] - x[0] > z[1] - z[0];
        let length = if along_x { x[1] - x[0] } else { z[1] - z[0] };
        let count = (length / 2.5).ceil() as usize;
        for i in 0..=count {
            let t = i as f32 / count.max(1) as f32;
            let mut lo = [x[0], parapet, z[0]];
            let mut hi = [x[1], parapet + 1.05, z[1]];
            if along_x {
                let p = x[0] + (x[1] - x[0] - 0.12) * t;
                lo[0] = p;
                hi[0] = p + 0.12;
            } else {
                let p = z[0] + (z[1] - z[0] - 0.12) * t;
                lo[2] = p;
                hi[2] = p + 0.12;
            }
            self.add("parapet.post", lo, hi, zone, SolidKind::Rail, Surface::Iron);
        }
    }
    pub fn occupied(&self, p: [f32; 3], height: f32, radius: f32) -> bool {
        self.solids.iter().any(|s| {
            p[1] < s.bounds.max[1] - EPS
                && p[1] + height > s.bounds.min[1] + EPS
                && s.bounds.overlaps_disk(p[0], p[2], radius)
        })
    }
    /// Highest surface reachable from BELOW `maximum`, never an upper floor
    /// selected merely because it shares X/Z with the player's feet.
    pub fn support_below(&self, x: f32, z: f32, maximum: f32, radius: f32) -> Option<f32> {
        self.solids
            .iter()
            .filter(|s| s.bounds.max[1] <= maximum + EPS && s.bounds.overlaps_disk(x, z, radius))
            .map(|s| s.bounds.max[1])
            .max_by(f32::total_cmp)
    }
}

pub fn castle() -> &'static Castle {
    static MAP: OnceLock<Castle> = OnceLock::new();
    MAP.get_or_init(build_castle)
}
fn build_castle() -> Castle {
    use SolidKind::*;
    use Zone::*;
    let mut m = Castle::default();
    // Existing cell renderer owns this geometry; shared support continues
    // through the exit seam while legacy furniture collision stays in core.
    m.stone(
        "basement.floor",
        [-5., -0.4, -7.],
        [5., 0., 5.],
        Hall,
        PropCollider,
    );
    for x in [[-5., -0.9], [0.9, 5.]] {
        m.stone(
            "basement.exitjamb",
            [x[0], 0., -7.3],
            [x[1], 3.6, -6.85],
            Hall,
            PropCollider,
        );
    }
    m.stone(
        "basement.exitlintel",
        [-0.9, 2.7, -7.3],
        [0.9, 3.6, -6.85],
        Hall,
        PropCollider,
    );
    m.slab("hall.floor", [-2., 2.], [-22., -7.], 0., Hall, Floor);
    for x in [[-2.6, -2.], [2., 2.6]] {
        m.stone("hall.wall", [x[0], 0., -22.], [x[1], 3.8, -7.], Hall, Wall);
    }
    m.stone(
        "hall.ceiling",
        [-2.6, 3.4, -22.],
        [2.6, 3.8, -7.],
        Hall,
        Ceiling,
    );
    m.slab(
        "great.floor",
        [-14., 14.],
        [-54., -22.],
        0.,
        GreatRoom,
        Floor,
    );
    for x in [[-14.6, -14.], [14., 14.6]] {
        m.stone(
            "great.wall",
            [x[0], 0., -54.],
            [x[1], 13.6, -22.],
            GreatRoom,
            Wall,
        );
    }
    for x in [[-14., -2.], [2., 14.]] {
        m.stone(
            "great.south",
            [x[0], 0., -22.],
            [x[1], 13.6, -21.4],
            GreatRoom,
            Wall,
        );
    }
    for x in [[-14., -3.], [3., 14.]] {
        m.stone(
            "great.north",
            [x[0], 0., -54.6],
            [x[1], 13.6, -54.],
            GreatRoom,
            Wall,
        );
    }
    m.stone(
        "great.ceiling",
        [-14.6, 13., -54.6],
        [14.6, 13.6, -21.4],
        GreatRoom,
        Ceiling,
    );
    m.flight("grand.step", [-3., 3.], -54., -64., 0., 6., 24, GrandStair);
    for x in [[-3.6, -3.1], [3.1, 3.6]] {
        m.stone(
            "grand.wall",
            [x[0], 0., -64.],
            [x[1], 9., -54.],
            GrandStair,
            Wall,
        );
    }
    m.add(
        "garden.ground",
        [-25., 5.5, -102.],
        [25., 6., -64.],
        Garden,
        Floor,
        Surface::Grass,
    );
    for x in [[-25.6, -25.], [25., 25.6]] {
        m.stone(
            "garden.outerwall",
            [x[0], 6., -102.6],
            [x[1], 15.2, -63.4],
            Garden,
            Wall,
        );
    }
    m.stone(
        "garden.northwall",
        [-25., 6., -102.6],
        [25., 15.2, -102.],
        Garden,
        Wall,
    );
    for x in [[-25., -3.], [3., 25.]] {
        m.stone(
            "garden.southwall",
            [x[0], 6., -64.],
            [x[1], 15.2, -63.4],
            Garden,
            Wall,
        );
    }
    m.stone(
        "garden.entrylintel",
        [-3., 9., -64.],
        [3., 15.2, -63.4],
        Garden,
        Wall,
    );
    m.slab(
        "walk.west",
        [-25., -23.],
        [-102., -64.],
        14.,
        WallWalk,
        Floor,
    );
    m.slab("walk.east", [23., 25.], [-102., -64.], 14., WallWalk, Floor);
    m.slab(
        "walk.north",
        [-23., 23.],
        [-102., -100.],
        14.,
        WallWalk,
        Floor,
    );
    m.slab(
        "walk.south",
        [-23., 23.],
        [-66., -64.],
        14.,
        WallWalk,
        Floor,
    );
    for x in [[-23., -22.85], [22.85, 23.]] {
        let spans: &[[f32; 2]] = if x[0] > 0. {
            &[[-100., -98.], [-86., -66.]]
        } else {
            &[[-100., -66.]]
        };
        for z in spans {
            m.stone(
                "walk.innerrail",
                [x[0], 14., z[0]],
                [x[1], 15.1, z[1]],
                WallWalk,
                Rail,
            );
        }
    }
    for z in [[-100., -99.85], [-66.15, -66.]] {
        m.stone(
            "walk.innerrail",
            [-23., 14., z[0]],
            [23., 15.1, z[1]],
            WallWalk,
            Rail,
        );
    }
    m.slab("tower.base", [15., 25.], [-98., -86.], 6., Tower, Floor);
    // West entrance follows the generator's measured body/head envelope.
    for z in [[-98., -87.95], [-87.05, -86.]] {
        m.stone(
            "tower.westwall",
            [14.6, 6., z[0]],
            [15., 23.2, z[1]],
            Tower,
            Wall,
        );
    }
    for z in [[-87.95, -87.86], [-87.14, -87.05]] {
        m.stone(
            "tower.archshoulder",
            [14.6, 7.7, z[0]],
            [15., 8., z[1]],
            Tower,
            Wall,
        );
    }
    m.stone(
        "tower.entrylintel",
        [14.6, 8., -87.95],
        [15., 23.2, -87.05],
        Tower,
        Wall,
    );
    for z in [[-98.5, -98.], [-86., -85.5]] {
        m.stone(
            "tower.endwall",
            [15., 6., z[0]],
            [23., 23.2, z[1]],
            Tower,
            Wall,
        );
        m.stone(
            "tower.walksill",
            [23., 6., z[0]],
            [25., 14., z[1]],
            Tower,
            Wall,
        );
        m.stone(
            "tower.walklintel",
            [23., 16.4, z[0]],
            [25., 23.2, z[1]],
            Tower,
            Wall,
        );
    }
    m.stone(
        "tower.eastwall",
        [25., 15.2, -98.5],
        [25.6, 23.2, -85.5],
        Tower,
        Wall,
    );
    for base in [6., 14.] {
        m.flight(
            "tower.northflight",
            [17., 20.],
            -89.,
            -96.,
            base,
            4.,
            16,
            Tower,
        );
        m.slab(
            "tower.northlanding",
            [17., 23.],
            [-98., -96.],
            base + 4.,
            Tower,
            Landing,
        );
        m.flight(
            "tower.southflight",
            [20.5, 23.],
            -96.,
            -89.,
            base + 4.,
            4.,
            16,
            Tower,
        );
        m.slab(
            "tower.southlanding",
            [17., 25.],
            [-89., -86.],
            base + 8.,
            Tower,
            Landing,
        );
        for x in [[16.88, 17.], [23., 23.12]] {
            m.stone(
                "tower.landingrail",
                [x[0], base + 4., -98.],
                [x[1], base + 5.1, -96.],
                Tower,
                Rail,
            );
        }
        if base < 10. {
            m.stone(
                "tower.southrail",
                [16.88, base + 8., -89.],
                [17., base + 9.1, -86.],
                Tower,
                Rail,
            );
        }
    }
    m.slab(
        "lookout.south",
        [15., 17.],
        [-89., -86.],
        22.,
        Lookout,
        Landing,
    );
    m.slab(
        "lookout.west",
        [15., 17.],
        [-98., -89.],
        22.,
        Lookout,
        Floor,
    );
    m.slab(
        "lookout.north",
        [17., 25.],
        [-98., -96.],
        22.,
        Lookout,
        Floor,
    );
    m.stone(
        "lookout.innerrail",
        [16.88, 22., -96.],
        [17., 23.1, -89.],
        Lookout,
        Rail,
    );
    m.stone(
        "lookout.innerrail",
        [17., 22., -96.],
        [25., 23.1, -95.88],
        Lookout,
        Rail,
    );
    for x in [[17., 20.5], [23., 25.]] {
        m.stone(
            "lookout.innerrail",
            [x[0], 22., -89.12],
            [x[1], 23.1, -89.],
            Lookout,
            Rail,
        );
    }
    for side in [-1., 1.] {
        for z in [-30., -40., -50.] {
            let x = 11. * side;
            m.props.push(PropPlacement {
                name: "gothic-pillar",
                position: [x, 0., z],
                scale: 1.,
                yaw: 0.,
                zone: GreatRoom,
            });
            m.stone(
                "pillar.collider",
                [x - 1.214, 0., z - 1.367],
                [x + 1.205, 4.397, z + 1.361],
                GreatRoom,
                PropCollider,
            );
        }
    }
    m.props.push(PropPlacement {
        name: "courtyard-fountain",
        position: [-7., 6., -80.],
        scale: 1.,
        yaw: 0.,
        zone: Garden,
    });
    m.stone(
        "fountain.collider",
        [-8.13, 6., -81.14],
        [-5.87, 7.16, -78.87],
        Garden,
        PropCollider,
    );
    m.props.push(PropPlacement {
        name: "tower-doorway",
        position: [15., 5.6, -87.5],
        scale: 1.25,
        yaw: std::f32::consts::FRAC_PI_2,
        zone: Tower,
    });
    // Generated portal geometry is 3.37 m deep after its quarter turn, so the
    // decorative stone needs depth-covering jamb proxies, not only wall-plane
    // collision. Keep the measured 0.90 m body/0.72 m upper opening unobstructed.
    for z in [[-89.658, -87.95], [-87.05, -85.34]] {
        m.stone(
            "portal.jamb",
            [13.315, 6., z[0]],
            [16.686, 13.103, z[1]],
            Tower,
            PropCollider,
        );
    }
    for z in [[-87.95, -87.86], [-87.14, -87.05]] {
        m.stone(
            "portal.shoulder",
            [13.315, 7.7, z[0]],
            [16.686, 8., z[1]],
            Tower,
            PropCollider,
        );
    }
    m.stone(
        "portal.lintel",
        [13.315, 8., -87.95],
        [16.686, 13.103, -87.05],
        Tower,
        PropCollider,
    );
    for x in [[-25.52, -25.08], [25.08, 25.52]] {
        m.viewing_fence(x, [-102.6, -63.4], 15.2, WallWalk);
    }
    for z in [[-102.52, -102.08], [-63.92, -63.48]] {
        m.viewing_fence([-25.6, 25.6], z, 15.2, WallWalk);
    }
    for x in [[14.68, 14.92], [25.08, 25.52]] {
        m.viewing_fence(x, [-98.5, -85.5], 23.2, Lookout);
    }
    for z in [[-98.42, -98.08], [-85.92, -85.58]] {
        m.viewing_fence([14.6, 25.6], z, 23.2, Lookout);
    }
    m
}

pub fn region_at(p: [f32; 3]) -> Zone {
    let [x, y, z] = p;
    if (14.6..=25.6).contains(&x) && (-98.5..=-85.5).contains(&z) {
        return if y >= 21.5 {
            Zone::Lookout
        } else {
            Zone::Tower
        };
    }
    if z < -64. {
        if y >= 13.5 {
            Zone::WallWalk
        } else {
            Zone::Garden
        }
    } else if z < -54. {
        Zone::GrandStair
    } else if z < -22. {
        Zone::GreatRoom
    } else {
        Zone::Hall
    }
}

/// Ordered connected center lanes. Reverse these to test the full descent.
pub const ROUTE: &[[f32; 3]] = &[
    [0., 0., -7.1],
    [0., 0., -23.],
    [0., 0., -53.5],
    [0., 6., -65.],
    [0., 6., -80.],
    [13.5, 6., -87.5],
    [16., 6., -87.5],
    [18.5, 6., -87.5],
    [18.5, 10., -97.],
    [21.75, 10., -97.],
    [21.75, 14., -87.5],
    [18.5, 14., -87.5],
    [18.5, 18., -97.],
    [21.75, 18., -97.],
    [21.75, 22., -87.5],
    [16., 22., -87.5],
    [16., 22., -97.],
    [21., 22., -97.],
];

#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub position: [f32; 3],
    pub velocity_y: f32,
    pub grounded: bool,
    pub radius: f32,
    pub height: f32,
}
impl Body {
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            velocity_y: 0.,
            grounded: true,
            radius: PLAYER_RADIUS,
            height: PLAYER_HEIGHT,
        }
    }
    pub fn jump(&mut self, speed: f32) -> bool {
        if !self.grounded {
            return false;
        }
        self.velocity_y = speed;
        self.grounded = false;
        true
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Contacts {
    pub blocked_x: bool,
    pub blocked_z: bool,
    pub landed: bool,
    pub head_hit: bool,
}

pub fn occupied_with(map: &Castle, extra: &[Aabb], p: [f32; 3], height: f32, radius: f32) -> bool {
    map.occupied(p, height, radius)
        || extra.iter().any(|s| {
            p[1] < s.max[1] - EPS
                && p[1] + height > s.min[1] + EPS
                && s.overlaps_disk(p[0], p[2], radius)
        })
}
fn support_with(
    map: &Castle,
    extra: &[Aabb],
    x: f32,
    z: f32,
    maximum: f32,
    radius: f32,
) -> Option<f32> {
    map.support_below(x, z, maximum, radius)
        .into_iter()
        .chain(
            extra
                .iter()
                .filter(|s| s.max[1] <= maximum + EPS && s.overlaps_disk(x, z, radius))
                .map(|s| s.max[1]),
        )
        .max_by(f32::total_cmp)
}
fn move_axis(map: &Castle, extra: &[Aabb], body: &mut Body, axis: usize, delta: f32) -> bool {
    if delta == 0. {
        return true;
    }
    let mut candidate = body.position;
    candidate[axis] += delta;
    if body.grounded && body.velocity_y <= 0. {
        if let Some(top) = support_with(
            map,
            extra,
            candidate[0],
            candidate[2],
            body.position[1] + STEP_HEIGHT,
            body.radius,
        ) {
            if (top - body.position[1]).abs() <= STEP_HEIGHT + EPS {
                candidate[1] = top;
            }
        }
    }
    if occupied_with(map, extra, candidate, body.height, body.radius) {
        return false;
    }
    body.position = candidate;
    body.grounded = support_with(
        map,
        extra,
        candidate[0],
        candidate[2],
        candidate[1] + EPS,
        body.radius,
    )
    .is_some_and(|y| (y - candidate[1]).abs() < EPS * 2.);
    true
}

/// Fixed-step cylinder movement. Horizontal substeps are smaller than the
/// radius; vertical motion sweeps actual slab bottoms/tops, including jumps.
pub fn advance_body(map: &Castle, body: &mut Body, delta: [f32; 2], dt: f32) -> Contacts {
    advance_body_with(map, &[], body, delta, dt)
}
pub fn advance_body_with(
    map: &Castle,
    extra: &[Aabb],
    body: &mut Body,
    delta: [f32; 2],
    dt: f32,
) -> Contacts {
    let mut result = Contacts::default();
    if !delta
        .iter()
        .chain(body.position.iter())
        .all(|x| x.is_finite())
        || !dt.is_finite()
    {
        return result;
    }
    let dt = dt.clamp(0., 0.1);
    let steps = (delta[0].hypot(delta[1]) / 0.08).ceil().max(1.) as usize;
    for _ in 0..steps {
        result.blocked_x |= !move_axis(map, extra, body, 0, delta[0] / steps as f32);
        result.blocked_z |= !move_axis(map, extra, body, 2, delta[1] / steps as f32);
    }
    let was_grounded = body.grounded;
    body.velocity_y -= 9.81 * dt;
    let old = body.position[1];
    let mut next = old + body.velocity_y * dt;
    if body.velocity_y <= 0. {
        if let Some(top) = support_with(
            map,
            extra,
            body.position[0],
            body.position[2],
            old + EPS,
            body.radius,
        ) {
            if top >= next - EPS {
                next = top;
                body.velocity_y = 0.;
                body.grounded = true;
                result.landed = !was_grounded;
            } else {
                body.grounded = false;
            }
        } else {
            body.grounded = false;
        }
    } else {
        let bottom = map
            .solids
            .iter()
            .map(|s| &s.bounds)
            .chain(extra)
            .filter(|s| {
                s.overlaps_disk(body.position[0], body.position[2], body.radius)
                    && s.min[1] >= old + body.height - EPS
                    && s.min[1] <= next + body.height
            })
            .map(|s| s.min[1])
            .min_by(f32::total_cmp);
        if let Some(bottom) = bottom {
            next = bottom - body.height;
            body.velocity_y = 0.;
            result.head_hit = true;
        }
        body.grounded = false;
    }
    body.position[1] = next;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn travel(map: &Castle, body: &mut Body, target: [f32; 3]) {
        for _ in 0..4000 {
            let dx = target[0] - body.position[0];
            let dz = target[2] - body.position[2];
            let d = dx.hypot(dz);
            if d < 0.025 {
                assert!(
                    (body.position[1] - target[1]).abs() < 0.28,
                    "height {:?} target {:?}",
                    body.position,
                    target
                );
                return;
            }
            let step = d.min(3. / 60.);
            advance_body(map, body, [dx / d * step, dz / d * step], 1. / 60.);
            assert!(
                !map.occupied(body.position, body.height, body.radius),
                "penetration {:?}",
                body.position
            );
        }
        panic!("route stuck {:?} -> {:?}", body.position, target);
    }
    #[test]
    fn route_climbs_all_flights_and_returns_without_teleports() {
        let map = castle();
        let mut body = Body::new(ROUTE[0]);
        for p in ROUTE.iter().skip(1) {
            travel(map, &mut body, *p);
        }
        for p in ROUTE.iter().rev().skip(1) {
            travel(map, &mut body, *p);
        }
        assert!(body.grounded);
        assert!(body.position[1].abs() < EPS);
    }
    #[test]
    fn tower_midlanding_connects_the_complete_perimeter_walk() {
        let map = castle();
        let mut body = Body::new([21.75, 14., -87.5]);
        for p in [
            [24., 14., -87.5],
            [24., 14., -65.],
            [-24., 14., -65.],
            [-24., 14., -101.],
            [24., 14., -101.],
            [24., 14., -87.5],
            [21.75, 14., -87.5],
        ] {
            travel(map, &mut body, p);
        }
    }
    #[test]
    fn support_does_not_snap_to_floors_above_the_player() {
        let map = castle();
        assert_eq!(map.support_below(24., -80., 6.28, PLAYER_RADIUS), Some(6.));
        let mut b = Body::new([24., 6., -80.]);
        for _ in 0..120 {
            advance_body(map, &mut b, [0., 0.], 1. / 60.);
        }
        assert_eq!(b.position[1], 6.);
    }
    #[test]
    fn step_height_and_all_route_samples_have_standing_headroom() {
        let map = castle();
        for s in &map.solids {
            if s.kind == SolidKind::Step {
                assert!((s.bounds.max[1] * 4. - (s.bounds.max[1] * 4.).round()).abs() < EPS);
            }
        }
        for p in ROUTE {
            assert!(
                !map.occupied(*p, PLAYER_HEIGHT, PLAYER_RADIUS),
                "blocked authored lane {p:?}"
            );
        }
    }
    #[test]
    fn thin_walls_block_large_horizontal_deltas() {
        let mut map = Castle::default();
        map.slab(
            "floor",
            [-5., 5.],
            [-5., 5.],
            0.,
            Zone::Hall,
            SolidKind::Floor,
        );
        map.stone(
            "thinwall",
            [1., 0., -5.],
            [1.01, 3., 5.],
            Zone::Hall,
            SolidKind::Wall,
        );
        let mut b = Body::new([0., 0., 0.]);
        let c = advance_body(&map, &mut b, [4., 0.], 1. / 60.);
        assert!(c.blocked_x);
        assert!(b.position[0] <= 1. - PLAYER_RADIUS + EPS);
    }
    #[test]
    fn swept_jump_hits_slab_bottom_and_fall_lands_on_first_top() {
        let mut map = Castle::default();
        map.slab(
            "floor",
            [-5., 5.],
            [-5., 5.],
            0.,
            Zone::Hall,
            SolidKind::Floor,
        );
        map.stone(
            "ceiling",
            [-2., 2., -2.],
            [2., 2.02, 2.],
            Zone::Hall,
            SolidKind::Ceiling,
        );
        let mut b = Body::new([0., 0., 0.]);
        b.jump(100.);
        let c = advance_body(&map, &mut b, [0., 0.], 0.1);
        assert!(c.head_hit);
        assert!((b.position[1] - 0.3).abs() < EPS);
        b.position = [0., 5., 0.];
        b.velocity_y = -100.;
        b.grounded = false;
        advance_body(&map, &mut b, [0., 0.], 0.1);
        assert_eq!(b.position[1], 2.02);
        assert!(b.grounded);
    }
    #[test]
    fn walking_off_a_platform_falls_instead_of_snapping_or_air_stepping() {
        let mut map = Castle::default();
        map.slab(
            "floor",
            [-5., 5.],
            [-5., 5.],
            0.,
            Zone::Hall,
            SolidKind::Floor,
        );
        map.slab(
            "ledge",
            [-1., 1.],
            [-1., 1.],
            4.,
            Zone::Hall,
            SolidKind::Landing,
        );
        let mut b = Body::new([0., 4., 0.]);
        advance_body(&map, &mut b, [2., 0.], 1. / 60.);
        assert!(!b.grounded);
        assert!(b.position[1] > 3.9);
        for _ in 0..120 {
            advance_body(&map, &mut b, [0., 0.], 1. / 60.);
        }
        assert_eq!(b.position[1], 0.);
    }
    #[test]
    fn generated_portal_jambs_collide_through_their_full_decorative_depth() {
        let map = castle();
        for x in [13.4, 13.8, 14.8, 16.4] {
            assert!(map.occupied([x, 6., -86.4], PLAYER_HEIGHT, PLAYER_RADIUS));
            assert!(map.occupied([x, 6., -88.6], PLAYER_HEIGHT, PLAYER_RADIUS));
            assert!(!map.occupied([x, 6., -87.5], PLAYER_HEIGHT, PLAYER_RADIUS));
        }
    }
    #[test]
    fn stair_rail_jump_cannot_cross_the_visible_exterior_fence() {
        fn go(b: &mut Body, target: [f32; 2], speed: f32, ticks: usize) {
            for _ in 0..ticks {
                let dx = target[0] - b.position[0];
                let dz = target[1] - b.position[2];
                let d = dx.hypot(dz);
                let step = d.min(speed / 60.);
                let delta = if d > 0.0001 {
                    [dx / d * step, dz / d * step]
                } else {
                    [0., 0.]
                };
                advance_body(castle(), b, delta, 1. / 60.);
            }
        }
        for height in [PLAYER_HEIGHT, 1.2] {
            let mut b = Body::new([22.75, 22., -89.2]);
            b.height = height;
            b.jump(4.1);
            go(&mut b, [23.04, -90.05], 3., 75);
            b.jump(4.1);
            go(&mut b, [25.30, -90.05], 4., 90);
            go(&mut b, [26.3, -90.05], 4., 180);
            assert!(b.position[0] < 25., "escaped exterior {b:?}");
            assert!(b.position[1] >= 14.);
        }
    }
}
