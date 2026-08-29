# Findings

### `assets/layouts/arena.json`
| Line | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- |
| 1 | **Critical** | **Missing `root` key**: The JSON top-level object does not contain a key (e.g., `"root"`). `serde` will fail to deserialize `Layouts`. | Add `"root": { "layouts": [...] }` to the JSON root. |
| 11 | **Low** | **Magic Numbers**: `yaw_deg: 0` is repeated 12 times. | Extract to a constant or use a loop in the generation script. |

### `assets/models/helmet.glb`
| Line | (N/A) | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- | :--- |
| - | **Info** | **Missing**: This is a new binary file. Ensure it is committed to version control. | - |

### `crates/ember-engine/src/shader.wgsl`
| Line | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- |
| 49 | **High** | **Undefined behavior**: `out.clip.w` is used as a view-space depth. The WGSL spec states `clip.w` is not guaranteed to be view-space depth. | Use `transform.position.z` or a manual projection matrix to calculate view depth. |
| 60 | **Low** | **Inconsistent Nomenclature**: Variable names (`sun`, `diff`, `lit`) were changed to `sun_dir`, `ndotl`, `sheen`. While readable, mixing naming styles within a shader can be confusing. | Stick to one naming convention (e.g., `sun_direction`, `diffuse_factor`) or align all new variables to the previous style. |
| 61 | **Low** | **Magic Constants**: `0.012` and `0.020` and `0.045` are repeated in the fog calculation. | Define constants (e.g., `FogDensity`, `FogColor`) at the top of the shader. |

### `crates/game/Cargo.toml`
| Line | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- |
| 11 | **Low** | **Unused Dependency**: `serde` and `serde_json` are only used in `props.rs`. | Move these dependencies to `props/Cargo.toml` or the workspace to keep `game` slim, or add `default-features = false` to `serde` if unused elsewhere. |

### `crates/game/src/main.rs`
| Line | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- |
| 43 | **Low** | **Hardcoded Path**: `assets/models/helmet.glb` is hardcoded as a candidate. | Use a configuration file or an environment variable to configure the monument path. |
| 159 | **Low** | **Inefficient Lookup**: `&layouts.layouts[0]` performs an index operation every frame. | Cache `&layouts.layouts[0]` in `Game` or use `layouts.layouts.first()` if the list is static. |
| 195 | **Medium** | **Potential Panic**: `&layouts.layouts[0]` will panic if `layouts` is `None`. | Add a `if !self.layouts.is_some() { return; }` check before the render block. |
| 367 | **Low** | **Unused Variable**: `monument` is loaded but never used (it is stored in `monument_parts`). | Remove `let monument = load_monument();` and `let monument_parts = monument.len() as u32;` and calculate length directly from `monument.len()`. |

### `crates/game/src/props.rs`
| Line | Severity | Issue | Fix |
| :--- | :--- | :--- | :--- |
| 12 | **Medium** | **Missing Error Handling**: `load_layouts` returns `Option<Layouts>`, but `main.rs` unwraps it with `?` or ignores it. If the file is missing, the arena renders with no props. | Ensure `None` handling in `main.rs` prevents rendering of props or provides a fallback layout. |
| 24 | **Low** | **Hardcoded Path**: `assets/layouts/arena.json` is hardcoded. | Use a configuration file (e.g., `config.toml`) to set the layout path. |
| 35 | **Low** | **Inefficient Lookup**: `&layouts.layouts[0]` is called every frame. | Cache the first layout in `Game`. |
| 50 | **Low** | **Inefficient Lookup**: `layouts.layouts.iter().find(...)` is called every frame. | Cache the first layout in `Game`. |
