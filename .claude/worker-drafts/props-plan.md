# Step-by-Step Plan: Adding Static Props to Arena Client

This plan implements static props (crates, barrels, pillars) using `box_mesh` for geometry, a JSON layout file for data, and `Instance` rendering.

### 1. Define Mesh ID Constants
Add new constants at the module level to identify the prop meshes (after `MESH_WALL`).

```rust
// In main.rs, inside the module scope
const MESH_CRATE: u32 = 4;
const MESH_BARREL: u32 = 5;
const MESH_PILLAR: u32 = 6;
```

### 2. Define the `Prop` Data Structure
Create a struct to hold the data needed for each prop instance (mesh ID, position, scale, rotation). Place this near the top of `main.rs` alongside `MESH_FLOOR`.

```rust
/// Represents a static object in the arena.
pub struct Prop {
    mesh: u32,
    pos: Vec3,
    scale: Vec3,
    yaw: f32,
}
```

### 3. Implement JSON Prop Loader
Create a function to parse `assets/layouts/props.json` (or a specific filename). This function will read the file, deserialize the JSON, and map the types to the mesh IDs defined in Step 1.

```rust
/// Loads prop definitions from assets/layouts/props.json.
/// Expects a JSON array of objects: [{"type":"crate","x":0,"z":0,"scale":1.0}, ...]
fn load_props_from_json(path: &str) -> Option<Vec<Prop>> {
    let candidates = [
        format!("{}/../../assets/layouts/{path}", env!("CARGO_MANIFEST_DIR")),
        format!("assets/layouts/{path}"),
    ];
    
    for path_str in candidates {
        if let Ok(bytes) = std::fs::read(&path_str) {
            // Assume serde_json is available, or implement basic parsing
            // Here we assume the JSON structure matches the Vec<Prop> type
            // For this example, we'll assume a simple struct is used or manual mapping.
            // In a real scenario, define a 'PropLayout' struct and use serde_json::from_slice.
            return None; 
        }
    }
    None
}
```

*Note: You will need to define a corresponding JSON layout structure and import `serde_json` if using a standard JSON parser.*

### 4. Modify the `World` Struct
The `World` module needs to store the list of props so they can be rendered every frame.

```rust
// In world/mod.rs (or main.rs if struct is defined there)
pub struct World {
    // ... existing fields (arena_half, snapshot, player_meta, etc.) ...
    pub props: Vec<Prop>,
}
```

### 5. Update `Game::arena_frame`
Modify the rendering logic in `Game` to iterate over `self.world.props` and push them to the `Frame` as `Instance`s.

```rust
// In main.rs, inside impl Game
fn arena_frame(&self, camera: Camera) -> Frame {
    let mut frame = Frame { camera, instances: Vec::new() };
    let half = self.world.arena_half;
    let span = half * 2.0 + 2.0;

    // --- Existing Floor & Walls ---
    frame.instances.push(Instance::new(Vec3::new(0.0, -0.5, 0.0), Vec3::new(span, 1.0, span), Vec3::new(0.16, 0.17, 0.20)));
    frame.instances.push(
        Instance::new(Vec3::new(0.0, 0.005, 0.0), Vec3::new(span, 1.0, span), Vec3::ONE)
            .with_mesh(MESH_FLOOR),
    );
    // ... (existing wall logic) ...

    // --- NEW: Static Props ---
    for prop in &self.world.props {
        // Reuse box_mesh logic via Instance creation
        // Note: 'prop.mesh' holds the ID (e.g., MESH_CRATE)
        // We use the 'yaw' and 'scale' from the prop data
        frame.instances.push(
            Instance::new(prop.pos, prop.scale, Vec3::ONE)
                .with_mesh(prop.mesh)
                .with_yaw(prop.yaw)
        );
    }

    frame
}
```

### 6. Update `main` (Mesh Registration & Initialization)
Add the mesh generation for props in `EngineConfig` and load the props in the main function.

**A. Add Meshes to `EngineConfig` (inside `main`):**
```rust
// In main.rs, inside the EngineConfig construction
meshes: vec![
    // ... existing meshes (plane, wall, char parts) ...
    box_mesh(4.0, load_texture("crate.png")),       // MESH_CRATE
    box_mesh(4.0, load_texture("barrel.png")),      // MESH_BARREL
    box_mesh(4.0, load_texture("pillar.png")),      // MESH_PILLAR
],
```

**B. Initialize Props in `main`:**
```rust
// In main.rs, inside the main function, after creating the World
let mut world = World::new(welcome.arena_half);
// ... existing roster loading ...

// Load props from JSON
if let Some(props) = load_props_from_json("props.json") {
    world.props = props;
} else {
    tracing::warn!("props.json not found, arena is empty");
}
```

### Summary of Functions/Touched Areas
*   **Constants:** `MESH_CRATE`, `MESH_BARREL`, `MESH_PILLAR`
*   **Structs:** `Prop` (new), `World` (add `props` field)
*   **Functions:** `load_props_from_json` (new), `arena_frame` (add prop loop), `main` (add mesh generation and props load call)
