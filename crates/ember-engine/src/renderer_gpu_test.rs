//! Offscreen regression checks use the shipping pipeline builders and WGSL.
//! Run explicitly with `cargo test -p ember-engine environment_gpu -- --ignored`.
//! Optional `EMBER_ENV_CAPTURE_DIR` saves diagnostic PNGs; no window is created.

use super::*;
use crate::OcclusionBox;

const SIZE: u32 = 256;

struct Rig {
    device: wgpu::Device,
    queue: wgpu::Queue,
    scene: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    particles: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bind: wgpu::BindGroup,
    mesh_bind: wgpu::BindGroup,
    shadow: ShadowTargets,
    occlusion_layout: wgpu::BindGroupLayout,
    vertices: wgpu::Buffer,
    instances: wgpu::Buffer,
    particle_buffer: wgpu::Buffer,
    target: wgpu::Texture,
    depth: wgpu::TextureView,
}

impl Rig {
    #[allow(clippy::too_many_lines)]
    async fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("explicit GPU regression requires a headless adapter");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("environment floor regression device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("device at WebGL2 limit floor");
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shipping environment WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("test scene uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let mesh_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("test mesh texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shadow_layout = create_shadow_layout(&device);
        let shadow = create_shadow_targets(&device, &shadow_layout);
        let occlusion_layout = create_occlusion_layout(&device);
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test scene pipeline layout"),
            bind_group_layouts: &[
                &uniform_layout,
                &mesh_layout,
                &shadow_layout,
                &occlusion_layout,
            ],
            push_constant_ranges: &[],
        });
        let effect_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test effects layout"),
            bind_group_layouts: &[&uniform_layout],
            push_constant_ranges: &[],
        });
        let scene = build_scene_pipeline(&device, &scene_layout, &shader);
        let sky = build_effect_pipeline(&device, &effect_layout, &shader, EffectPass::Sky);
        let shadow_pipeline =
            build_effect_pipeline(&device, &effect_layout, &shader, EffectPass::Shadow);
        let particles =
            build_effect_pipeline(&device, &effect_layout, &shader, EffectPass::Particle);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test scene uniform"),
            size: SceneUniform::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("test uniform bind"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let white = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("test white texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let white_view = white.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let mesh_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("test white mesh bind"),
            layout: &mesh_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("test actual cube vertices"),
            contents: bytemuck::cast_slice(&cube_vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instances = create_instance_buf(&device, 32);
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test particles"),
            size: (MAX_PARTICLES * std::mem::size_of::<ParticleRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let target = texture(
            "test scene colour",
            SCENE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            "test scene depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&wgpu::TextureViewDescriptor::default());
        assert!(
            device.pop_error_scope().await.is_none(),
            "all actual pipelines must compile at the requested WebGL2 limits"
        );
        Self {
            device,
            queue,
            scene,
            sky,
            shadow_pipeline,
            particles,
            uniform,
            uniform_bind,
            mesh_bind,
            shadow,
            occlusion_layout,
            vertices,
            instances,
            particle_buffer,
            target,
            depth,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &Frame, cast_shadows: bool) -> Vec<u8> {
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let uniform = SceneUniform::new(frame, 1.0);
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let occlusion_bind = create_occlusion_bind(
            &self.device,
            &self.queue,
            &self.occlusion_layout,
            frame.occlusion.as_deref(),
        );
        let instances: Vec<InstanceRaw> = frame.instances.iter().map(InstanceRaw::from).collect();
        if !instances.is_empty() {
            self.queue
                .write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        }
        let particles = particle_instances(frame, &uniform);
        if !particles.is_empty() {
            self.queue
                .write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&particles));
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test environment encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("test real shadow pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if cast_shadows && frame.environment.enabled {
                pass.set_pipeline(&self.shadow_pipeline);
                pass.set_bind_group(0, &self.uniform_bind, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_vertex_buffer(1, self.instances.slice(..));
                for (i, instance) in frame.instances.iter().enumerate() {
                    if instance.casts_shadow {
                        pass.draw(0..36, i as u32..i as u32 + 1);
                    }
                }
            }
        }
        let target_view = self
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("test actual scene/sky/particles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.028,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if frame.environment.enabled {
                pass.set_pipeline(&self.sky);
                pass.set_bind_group(0, &self.uniform_bind, &[]);
                pass.draw(0..3, 0..1);
            }
            if !instances.is_empty() {
                pass.set_pipeline(&self.scene);
                pass.set_bind_group(0, &self.uniform_bind, &[]);
                pass.set_bind_group(1, &self.mesh_bind, &[]);
                pass.set_bind_group(2, &self.shadow.bind, &[]);
                pass.set_bind_group(3, &occlusion_bind, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_vertex_buffer(1, self.instances.slice(..));
                pass.draw(0..36, 0..instances.len() as u32);
            }
            if !particles.is_empty() {
                pass.set_pipeline(&self.particles);
                pass.set_bind_group(0, &self.uniform_bind, &[]);
                pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
                pass.draw(0..6, 0..particles.len() as u32);
            }
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test RGBA readback"),
            size: u64::from(SIZE * SIZE * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 4),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).expect("readback callback channel");
            });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("readback callback")
            .expect("readback map succeeds");
        let pixels = readback.slice(..).get_mapped_range().to_vec();
        readback.unmap();
        assert!(
            pollster::block_on(self.device.pop_error_scope()).is_none(),
            "render submission must validate"
        );
        pixels
    }
}

fn changed_pixels(a: &[u8], b: &[u8], threshold: u8) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a[..3]
                .iter()
                .zip(&b[..3])
                .any(|(x, y)| x.abs_diff(*y) > threshold)
        })
        .count()
}

fn save_capture(name: &str, pixels: &[u8]) {
    if let Some(directory) = std::env::var_os("EMBER_ENV_CAPTURE_DIR") {
        let path = std::path::PathBuf::from(directory).join(format!("{name}.png"));
        image::save_buffer(path, pixels, SIZE, SIZE, image::ColorType::Rgba8)
            .expect("save requested environment diagnostic capture");
    }
}

const fn occlusion_floor() -> OcclusionBox {
    OcclusionBox {
        min: Vec3::new(-12.0, -1.0, -12.0),
        max: Vec3::new(12.0, 0.0, 12.0),
    }
}

fn occlusion_test_field(boxes: &[OcclusionBox]) -> Arc<OcclusionField> {
    Arc::new(
        OcclusionField::bake(
            crate::OcclusionSettings {
                min: Vec3::new(-10.0, -1.5, -10.0),
                max: Vec3::new(10.0, 6.5, 10.0),
                cell_size: 0.5,
                radius: 2.25,
            },
            boxes,
        )
        .expect("small authored AO fixture fits the field limits"),
    )
}

fn occlusion_box_instance(bounds: OcclusionBox) -> Instance {
    Instance::new(
        (bounds.min + bounds.max) * 0.5,
        bounds.max - bounds.min,
        Vec3::splat(0.45),
    )
    .with_surface(0.9, 0.0)
}

fn occlusion_test_frame() -> Frame {
    Frame {
        camera: Camera {
            eye: Vec3::new(4.0, 3.0, 6.0),
            target: Vec3::ZERO,
            fov_y_deg: 60.0,
        },
        environment: Environment {
            enabled: true,
            sun_direction: Vec3::new(-0.48, 0.76, -0.30).normalize(),
            // Disable direct sunlight to isolate the indirect-light effect.
            sun_intensity: 0.0,
            cloud_coverage: 0.0,
            wetness: 0.0,
            ..Environment::default()
        },
        fog: Fog {
            color: [0.0; 3],
            density: 0.0,
        },
        instances: vec![occlusion_box_instance(occlusion_floor())],
        ..Frame::default()
    }
}

fn projected_patch(frame: &Frame, point: Vec3) -> impl Fn(u32, u32) -> bool {
    let clip = frame.camera.view_proj(1.0) * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    assert!(clip.w > 0.0 && ndc.x.abs() < 0.9 && ndc.y.abs() < 0.9);
    let image_size = f32::from(u16::try_from(SIZE).expect("test image size fits u16"));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pixel = [
        (ndc.x.mul_add(0.5, 0.5) * image_size).round() as u32,
        (ndc.y.mul_add(-0.5, 0.5) * image_size).round() as u32,
    ];
    move |x: u32, y: u32| x.abs_diff(pixel[0]) <= 3 && y.abs_diff(pixel[1]) <= 3
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_open_floor_and_outside_field_are_neutral() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = occlusion_test_frame();
    let neutral = rig.render(&frame, false);
    frame.occlusion = Some(occlusion_test_field(&[occlusion_floor()]));
    assert_eq!(
        rig.render(&frame, false),
        neutral,
        "up-facing open ground must not read the occupied floor or downward visibility"
    );
    save_capture("occlusion-open-floor", &neutral);

    // A completely occupied volume would darken any receiver inside it;
    // translating the same scene outside must select neutral, not edge clamp.
    frame.occlusion = Some(occlusion_test_field(&[OcclusionBox {
        min: Vec3::new(-10.0, -1.5, -10.0),
        max: Vec3::new(10.0, 6.5, 10.0),
    }]));
    let shift = Vec3::new(40.0, 0.0, 0.0);
    frame.camera.eye += shift;
    frame.camera.target += shift;
    frame.instances[0].position += shift;
    let outside = rig.render(&frame, false);
    frame.occlusion = None;
    assert_eq!(
        outside,
        rig.render(&frame, false),
        "receivers outside the baked bounds must remain neutral"
    );
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_thin_wall_recessed_faces_do_not_self_stain() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = occlusion_test_frame();
    let field = occlusion_test_field(&[OcclusionBox {
        min: Vec3::new(-3.0, -1.0, -0.05),
        max: Vec3::new(3.0, 5.0, 0.05),
    }]);
    // The rendered front/back faces lie 35 mm inside a 100 mm solid wall,
    // as a recessed artist face can do. Its normal-facing hemisphere remains
    // open once sampled outside the surface; the opposite side must not stain it.
    frame.instances = vec![
        Instance::new(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(4.0, 3.0, 0.03),
            Vec3::splat(0.45),
        )
        .with_surface(0.9, 0.0),
    ];
    for sign in [-1.0, 1.0] {
        frame.camera.eye = Vec3::new(0.0, 2.0, sign * 5.0);
        frame.camera.target = Vec3::new(0.0, 2.0, sign * 0.015);
        frame.occlusion = None;
        let plain = rig.render(&frame, false);
        frame.occlusion = Some(Arc::clone(&field));
        assert_eq!(
            changed_pixels_in(&plain, &rig.render(&frame, false), 0, |x, y| {
                (90..=166).contains(&x) && (90..=166).contains(&y)
            }),
            0,
            "recessed face on side {sign} must not read its own occupied/opposite-side cells"
        );
    }
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_darkens_local_contacts_for_both_material_paths() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = occlusion_test_frame();
    let wall = OcclusionBox {
        min: Vec3::new(-0.5, 0.0, -2.0),
        max: Vec3::new(0.5, 3.0, 2.0),
    };
    frame.instances.push(occlusion_box_instance(wall));
    let plain = rig.render(&frame, false);
    frame.occlusion = Some(occlusion_test_field(&[occlusion_floor(), wall]));
    let contact = rig.render(&frame, false);
    assert!(
        changed_pixels(&plain, &contact, 3) > 300,
        "nearby cover must shade indirect light with the direct-shadow pass disabled"
    );
    assert!(
        mean_luminance_in(&contact, |_, _| true) < mean_luminance_in(&plain, |_, _| true),
        "contact accessibility must darken, not add light"
    );
    assert!(
        mean_luminance_in(&plain, projected_patch(&frame, Vec3::new(1.0, 0.0, 0.0)))
            > mean_luminance_in(&contact, projected_patch(&frame, Vec3::new(1.0, 0.0, 0.0))) + 3.0,
        "the known floor/wall junction must darken, not just the wall's own face"
    );
    assert_eq!(
        changed_pixels_in(
            &plain,
            &contact,
            1,
            projected_patch(&frame, Vec3::new(3.5, 0.0, 0.0))
        ),
        0,
        "open ground beyond the radius stays unaltered"
    );
    assert_eq!(
        changed_pixels_in(&plain, &contact, 0, |_, y| y < 20),
        0,
        "the sky above the wall must not receive the static field"
    );
    save_capture("occlusion-contact-disabled", &plain);
    save_capture("occlusion-contact-enabled", &contact);

    for instance in &mut frame.instances {
        instance.surface = None;
    }
    let legacy_shaded = rig.render(&frame, false);
    frame.occlusion = None;
    assert!(
        mean_luminance_in(
            &rig.render(&frame, false),
            projected_patch(&frame, Vec3::new(1.0, 0.0, 0.0))
        ) > mean_luminance_in(
            &legacy_shaded,
            projected_patch(&frame, Vec3::new(1.0, 0.0, 0.0))
        ) + 3.0,
        "outdoor legacy-material receivers also consume contact visibility"
    );
}

fn mean_scene_radiance_in(pixels: &[u8], selected: impl Fn(u32, u32) -> bool) -> [f64; 3] {
    let mut sum = [0.0; 3];
    let mut count = 0_u32;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        let index = u32::try_from(index).expect("test pixels fit u32");
        if !selected(index % SIZE, index / SIZE) {
            continue;
        }
        for (channel, byte) in sum.iter_mut().zip(&pixel[..3]) {
            let encoded = f64::from(*byte) / 255.0;
            let mapped = if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            };
            assert!(
                mapped < 0.95,
                "radiance fixture must not reach tone-map clipping"
            );
            // Invert the actual shipping ACES-fit rational curve using its
            // positive quadratic root. Comparing encoded differences directly
            // would confuse nonlinear tone mapping with a changed sun term.
            let a = mapped.mul_add(2.43, -2.51);
            let b = mapped.mul_add(0.59, -0.03);
            let c = mapped * 0.14;
            *channel += (-b - b.mul_add(b, -4.0 * a * c).sqrt()) / (2.0 * a);
        }
        count += 1;
    }
    assert!(count > 0);
    sum.map(|value| value / f64::from(count))
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_preserves_direct_sun_energy() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = occlusion_test_frame();
    let wall = OcclusionBox {
        min: Vec3::new(-0.5, 0.0, -2.0),
        max: Vec3::new(0.5, 3.0, 2.0),
    };
    frame.instances.push(occlusion_box_instance(wall));
    let field = occlusion_test_field(&[occlusion_floor(), wall]);
    let mut contribution = Vec::new();
    for enabled in [false, true] {
        frame.occlusion = enabled.then(|| Arc::clone(&field));
        let mut measured = Vec::new();
        for sunlight in [0.0, 0.35] {
            frame.environment.sun_intensity = sunlight;
            let pixels = rig.render(&frame, false);
            measured.push(mean_scene_radiance_in(
                &pixels,
                projected_patch(&frame, Vec3::new(1.0, 0.0, 0.0)),
            ));
        }
        contribution.push(std::array::from_fn::<_, 3, _>(|channel| {
            measured[1][channel] - measured[0][channel]
        }));
    }
    for channel in 0..3 {
        assert!(
            contribution[0][channel] > 0.05,
            "fixture needs meaningful direct sun: {contribution:?}"
        );
        assert!(
            (contribution[0][channel] - contribution[1][channel]).abs() < 0.005,
            "AO must preserve the direct-light contribution within RGBA8 quantization: {contribution:?}"
        );
    }
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_lifted_roof_preserves_empty_space_and_distance_limit() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = occlusion_test_frame();
    let roof = OcclusionBox {
        min: Vec3::new(-3.0, 1.5, -3.0),
        max: Vec3::new(3.0, 2.0, 3.0),
    };
    frame.instances = vec![
        occlusion_box_instance(occlusion_floor()),
        occlusion_box_instance(roof),
    ];
    frame.camera.eye = Vec3::new(0.0, 0.8, 4.0);
    frame.camera.target = Vec3::ZERO;
    frame.occlusion = None;
    let under_plain = rig.render(&frame, false);
    frame.occlusion = Some(occlusion_test_field(&[occlusion_floor(), roof]));
    let under_shaded = rig.render(&frame, false);
    let center = |x, y| (116..=140).contains(&x) && (116..=140).contains(&y);
    assert!(
        mean_luminance_in(&under_plain, center) > mean_luminance_in(&under_shaded, center) + 3.0,
        "raised cover must shade the visible floor under its actual bottom, not fill the tunnel"
    );
    save_capture("occlusion-roof-disabled", &under_plain);
    save_capture("occlusion-roof-enabled", &under_shaded);
    let high_roof = OcclusionBox {
        min: roof.min + Vec3::Y * 2.0,
        max: roof.max + Vec3::Y * 2.0,
    };
    frame.instances[1] = occlusion_box_instance(high_roof);
    frame.occlusion = None;
    let far_plain = rig.render(&frame, false);
    frame.occlusion = Some(occlusion_test_field(&[occlusion_floor(), high_roof]));
    assert_eq!(
        changed_pixels_in(&far_plain, &rig.render(&frame, false), 0, center),
        0,
        "roof beyond the contact radius must stop darkening the floor"
    );
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_occlusion_bypasses_nonreceivers_sky_particles_and_legacy() {
    let rig = pollster::block_on(Rig::new());
    let field = occlusion_test_field(&[OcclusionBox {
        min: Vec3::new(-10.0, -1.5, -10.0),
        max: Vec3::new(10.0, 6.5, 10.0),
    }]);
    let mut frame = occlusion_test_frame();
    frame.camera.eye = Vec3::new(0.0, 1.0, 5.0);
    frame.camera.target = Vec3::new(0.0, 1.0, 0.0);
    frame.instances = vec![Instance::new(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(4.0, 4.0, 0.1),
        Vec3::splat(0.45),
    )];
    let reference = rig.render(&frame, false);
    frame.occlusion = Some(Arc::clone(&field));
    assert!(changed_pixels(&reference, &rig.render(&frame, false), 3) > 1000);
    frame.instances[0] = frame.instances[0].without_shadow();
    assert_eq!(
        rig.render(&frame, false),
        reference,
        "viewmodel and FX nonreceivers must bypass even fully occupied visibility"
    );
    frame.instances[0].casts_shadow = true;
    frame.environment.enabled = false;
    let legacy = rig.render(&frame, false);
    frame.occlusion = None;
    assert_eq!(
        rig.render(&frame, false),
        legacy,
        "legacy games must remain unchanged"
    );

    frame.environment.enabled = true;
    frame.instances.clear();
    frame.particles = vec![Particle {
        position: Vec3::new(0.0, 1.0, 1.0),
        size: glam::Vec2::splat(2.0),
        color: Vec3::new(2.0, 0.5, 0.1),
        opacity: 0.6,
    }];
    let effects = rig.render(&frame, false);
    frame.occlusion = Some(field);
    assert_eq!(
        rig.render(&frame, false),
        effects,
        "sky and alpha particles must not sample static ambient visibility"
    );
}

fn material_test_frame() -> Frame {
    Frame {
        camera: Camera {
            eye: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            fov_y_deg: 60.0,
        },
        environment: Environment {
            enabled: true,
            sun_direction: Vec3::Z,
            sun_color: Vec3::ONE,
            sun_intensity: 0.7,
            sky_zenith: Vec3::ZERO,
            sky_horizon: Vec3::ZERO,
            cloud_coverage: 0.0,
            wetness: 0.0,
            ..Environment::default()
        },
        fog: Fog {
            color: [0.0; 3],
            density: 0.0,
        },
        // White texture, constant albedo and front-face normal isolate material
        // response. The plate leaves a wide untouched background border.
        instances: vec![
            Instance::new(
                Vec3::ZERO,
                Vec3::new(4.0, 4.0, 0.1),
                Vec3::new(0.3, 0.12, 0.04),
            )
            .without_shadow(),
        ],
        ..Frame::default()
    }
}

fn mean_luminance_in(pixels: &[u8], selected: impl Fn(u32, u32) -> bool) -> f64 {
    let mut sum = 0.0;
    let mut count = 0_u32;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        let index = u32::try_from(index).expect("test image fits u32 indexing");
        if selected(index % SIZE, index / SIZE) {
            sum += 0.2126_f64.mul_add(
                f64::from(pixel[0]),
                0.7152_f64.mul_add(f64::from(pixel[1]), 0.0722 * f64::from(pixel[2])),
            );
            count += 1;
        }
    }
    assert!(count > 0, "material pixel region must not be empty");
    sum / f64::from(count)
}

fn changed_pixels_in(
    a: &[u8],
    b: &[u8],
    threshold: u8,
    selected: impl Fn(u32, u32) -> bool,
) -> usize {
    assert_eq!(a.len(), b.len());
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .enumerate()
        .filter(|(index, (a, b))| {
            let index = u32::try_from(*index).expect("test image fits u32 indexing");
            selected(index % SIZE, index / SIZE)
                && a[..3]
                    .iter()
                    .zip(&b[..3])
                    .any(|(x, y)| x.abs_diff(*y) > threshold)
        })
        .count()
}

const fn material_background(x: u32, y: u32) -> bool {
    x < 24 || y < 24 || x >= SIZE - 24 || y >= SIZE - 24
}

const fn material_center(x: u32, y: u32) -> bool {
    x.abs_diff(SIZE / 2) <= 3 && y.abs_diff(SIZE / 2) <= 3
}

fn material_annulus(x: u32, y: u32) -> bool {
    let dx = x.abs_diff(SIZE / 2);
    let dy = y.abs_diff(SIZE / 2);
    (64 * 64..=78 * 78).contains(&(dx * dx + dy * dy))
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_material_roughness_controls_sun_lobe_width() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = material_test_frame();
    let mut response = Vec::new();
    for (name, roughness) in [("smooth", 0.12), ("rough", 0.9)] {
        frame.instances[0].surface = Some(Surface {
            roughness,
            metallic: 1.0,
        });
        frame.environment.sun_intensity = 0.7;
        let lit = rig.render(&frame, false);
        frame.environment.sun_intensity = 0.0;
        let ambient = rig.render(&frame, false);
        let center =
            mean_luminance_in(&lit, material_center) - mean_luminance_in(&ambient, material_center);
        let annulus = mean_luminance_in(&lit, material_annulus)
            - mean_luminance_in(&ambient, material_annulus);
        save_capture(&format!("material-{name}-sun"), &lit);
        save_capture(&format!("material-{name}-ambient"), &ambient);
        response.push((center, annulus, lit));
    }
    let (smooth_center, smooth_annulus, smooth) = &response[0];
    let (rough_center, rough_annulus, rough) = &response[1];
    // Subtract each material's no-sun reference so the roughness-dependent
    // broad sky approximation cannot masquerade as a changed direct lobe.
    assert!(
        *smooth_center > *rough_center + 20.0,
        "smooth metal needs a stronger central sun highlight: smooth={smooth_center}, rough={rough_center}"
    );
    assert!(
        *rough_annulus > *smooth_annulus + 5.0,
        "rough metal must spread sun energy farther off-axis: smooth={smooth_annulus}, rough={rough_annulus}"
    );
    assert!(changed_pixels(smooth, rough, 3) > 250);
    assert_eq!(
        changed_pixels_in(smooth, rough, 0, material_background),
        0,
        "instance roughness must not affect sky/background pixels"
    );

    // Very low light keeps the narrow peak below tone-map saturation. A too
    // large denominator epsilon makes the supported .08 surface incorrectly
    // dimmer than .10 at normal incidence, even though both are valid inputs.
    frame.environment.sun_intensity = 0.0001;
    frame.instances[0].color = Vec3::splat(0.05);
    let mut peaks = Vec::new();
    for (name, roughness) in [("minimum", 0.08), ("near-minimum", 0.1)] {
        frame.instances[0] = frame.instances[0].with_surface(roughness, 1.0);
        let pixels = rig.render(&frame, false);
        peaks.push(mean_luminance_in(&pixels, |x, y| {
            (127..=128).contains(&x) && (127..=128).contains(&y)
        }));
        save_capture(&format!("material-low-light-{name}"), &pixels);
    }
    assert!(
        peaks[0] > peaks[1] + 3.0,
        "decreasing valid minimum roughness must strengthen the unsaturated central highlight: {peaks:?}"
    );
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_material_metallic_is_per_instance_and_opt_in() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = material_test_frame();
    let legacy = frame.instances[0];
    frame.instances[0] = legacy.with_surface(0.35, 0.0);
    let dielectric = rig.render(&frame, false);
    frame.instances[0] = legacy.with_surface(0.35, 1.0);
    let metal = rig.render(&frame, false);
    assert!(
        changed_pixels(&dielectric, &metal, 3) > 250,
        "bare metal must not render identically to dielectric paint"
    );
    assert_eq!(
        changed_pixels_in(&dielectric, &metal, 0, material_background),
        0,
        "instance metallic must not affect sky/background pixels"
    );
    save_capture("material-dielectric", &dielectric);
    save_capture("material-metal", &metal);

    frame.environment.enabled = false;
    frame.instances[0] = legacy;
    let disabled_legacy = rig.render(&frame, false);
    for (roughness, metallic) in [(0.08, 0.0), (0.08, 1.0), (1.0, 0.0), (1.0, 1.0)] {
        frame.instances[0] = legacy.with_surface(roughness, metallic);
        assert_eq!(
            rig.render(&frame, false),
            disabled_legacy,
            "disabled environment must ignore all explicit surface values"
        );
    }

    frame.environment.enabled = true;
    frame.instances = [-1.1, 1.1]
        .map(|x| {
            Instance::new(
                Vec3::new(x, 0.0, 0.0),
                Vec3::new(1.4, 2.0, 0.1),
                legacy.color,
            )
            .without_shadow()
            .with_surface(0.65, 0.0)
        })
        .to_vec();
    let pair_before = rig.render(&frame, false);
    frame.instances[0] = frame.instances[0].with_surface(0.15, 1.0);
    let pair_after = rig.render(&frame, false);
    assert!(
        changed_pixels_in(&pair_before, &pair_after, 3, |x, y| {
            (60..=88).contains(&x) && (108..=148).contains(&y)
        }) > 200,
        "left instance must receive its own changed material"
    );
    assert_eq!(
        changed_pixels_in(&pair_before, &pair_after, 0, |x, _| x >= SIZE / 2),
        0,
        "changing the left material must not leak into the right instance or background"
    );
    frame.instances.reverse();
    assert_eq!(
        rig.render(&frame, false),
        pair_after,
        "non-overlapping material instances must survive reversed upload order"
    );
    save_capture("material-isolated-pair", &pair_after);
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_material_highlight_tracks_view_without_changing_legacy() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = material_test_frame();
    let legacy = frame.instances[0];
    let mut centers = Vec::new();
    for (name, eye_x) in [("frontal", 0.0), ("oblique", 2.0)] {
        // The camera always targets the same constant-albedo front-face point;
        // its central patch stays far from silhouettes and uses no fog.
        frame.camera.eye.x = eye_x;
        frame.instances[0] = legacy;
        let legacy_pixels = rig.render(&frame, false);
        frame.instances[0] = legacy.with_surface(0.12, 1.0);
        let material_pixels = rig.render(&frame, false);
        centers.push((
            mean_luminance_in(&legacy_pixels, material_center),
            mean_luminance_in(&material_pixels, material_center),
        ));
        save_capture(&format!("material-view-{name}"), &material_pixels);
    }
    assert!(
        (centers[0].0 - centers[1].0).abs() <= 1.0,
        "legacy constant front-face lighting must remain view-independent: {centers:?}"
    );
    assert!(
        centers[0].1 > centers[1].1 + 20.0,
        "moving away from the sun reflection must move the sharp material highlight: {centers:?}"
    );
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
fn environment_gpu_isolated_receivers_do_not_shadow_themselves() {
    let rig = pollster::block_on(Rig::new());
    let mut failures = Vec::new();
    for extent in [55.0, 75.0] {
        for (sun_name, sun) in [
            ("harbor", Vec3::new(-0.48, 0.76, -0.30)),
            ("low", Vec3::new(-0.9, 0.25, -0.3)),
        ] {
            for column in [false, true] {
                let (name, camera, receiver) = if column {
                    (
                        "column",
                        Camera {
                            eye: Vec3::new(-5.0, 8.0, -8.0),
                            target: Vec3::new(0.0, 6.0, 0.0),
                            fov_y_deg: 60.0,
                        },
                        Instance::new(
                            Vec3::new(0.0, 6.0, 0.0),
                            Vec3::new(1.2, 12.0, 1.2),
                            Vec3::splat(0.45),
                        ),
                    )
                } else {
                    (
                        "ground",
                        Camera {
                            eye: Vec3::new(12.0, 6.0, 18.0),
                            target: Vec3::ZERO,
                            fov_y_deg: 60.0,
                        },
                        Instance::new(
                            Vec3::new(0.0, -0.1, 0.0),
                            Vec3::new(240.0, 0.2, 240.0),
                            Vec3::splat(0.45),
                        ),
                    )
                };
                let frame = Frame {
                    camera,
                    environment: Environment {
                        enabled: true,
                        sun_direction: sun.normalize(),
                        shadow_extent: extent,
                        cloud_coverage: 0.0,
                        wetness: 0.0,
                        ..Environment::default()
                    },
                    fog: Fog {
                        color: [0.55, 0.65, 0.75],
                        density: 0.0,
                    },
                    // A convex object has no other caster in front of its
                    // sun-facing surfaces. The texture is the rig's white
                    // pixel: any periodic bands are shadow sampling, not art.
                    instances: vec![receiver],
                    ..Frame::default()
                };
                let shadowed = rig.render(&frame, true);
                let unshadowed = rig.render(&frame, false);
                let label = format!("receiver-{name}-{sun_name}-{extent}");
                let changed = changed_pixels(&shadowed, &unshadowed, 2);
                save_capture(&label, &shadowed);
                save_capture(&format!("{label}-reference"), &unshadowed);
                // Permit a tiny silhouette fringe from the finite 3x3
                // footprint; never a band or triangle pattern on the face.
                if changed > 64 {
                    failures.push(format!("{label}: {changed} self-shadowed pixels"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "isolated receivers must match the no-caster reference:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "requires an actual headless GPU adapter; opt-in release gate"]
#[allow(clippy::too_many_lines)]
fn environment_gpu_pixels_cover_sky_shadows_reflections_and_particles() {
    let rig = pollster::block_on(Rig::new());
    let mut frame = Frame {
        camera: Camera {
            eye: Vec3::new(9.0, 7.0, 12.0),
            target: Vec3::new(0.0, 1.0, 0.0),
            fov_y_deg: 60.0,
        },
        environment: Environment {
            enabled: true,
            sun_direction: Vec3::new(-1.0, 0.8, -0.4).normalize(),
            shadow_extent: 24.0,
            cloud_coverage: 0.5,
            ..Environment::default()
        },
        fog: Fog {
            color: [0.55, 0.65, 0.75],
            density: 0.003,
        },
        instances: vec![
            Instance::new(
                Vec3::new(0.0, -0.1, 0.0),
                Vec3::new(40.0, 0.2, 40.0),
                Vec3::splat(0.42),
            )
            .with_wetness(),
            Instance::new(
                Vec3::new(0.0, 1.5, 0.0),
                Vec3::new(3.0, 3.0, 3.0),
                Vec3::new(0.65, 0.18, 0.08),
            ),
        ],
        ..Frame::default()
    };
    let shadowed = rig.render(&frame, true);
    let unshadowed = rig.render(&frame, false);
    assert!(
        changed_pixels(&shadowed, &unshadowed, 12) > 80,
        "caster must darken receiver pixels"
    );
    save_capture("environment-shadowed", &shadowed);
    save_capture("environment-unshadowed", &unshadowed);
    frame.environment.wetness = 0.9;
    let wet = rig.render(&frame, true);
    assert!(
        changed_pixels(&wet, &shadowed, 4) > 300,
        "wet receiver reflects environment"
    );
    save_capture("environment-wet", &wet);
    frame.instances[0].wettable = false;
    assert_eq!(
        rig.render(&frame, true),
        shadowed,
        "unmarked geometry never gets global wet gloss"
    );

    frame.instances.clear();
    frame.environment.wetness = 0.0;
    frame.camera = Camera {
        eye: Vec3::ZERO,
        target: Vec3::new(0.0, 0.6, -1.0),
        fov_y_deg: 70.0,
    };
    frame.environment.sun_direction = frame.camera.target.normalize();
    frame.environment.cloud_coverage = 0.0;
    let sun = rig.render(&frame, true);
    frame.environment.sun_intensity = 0.0;
    let no_sun = rig.render(&frame, true);
    assert!(
        changed_pixels(&sun, &no_sun, 25) > 25,
        "visible sun uses directional light vector"
    );
    save_capture("environment-sun", &sun);
    frame.environment.sun_intensity = 1.15;
    frame.environment.cloud_coverage = 0.68;
    let clouds = rig.render(&frame, true);
    frame.environment.time = 180.0;
    let moved_clouds = rig.render(&frame, true);
    assert!(
        changed_pixels(&clouds, &moved_clouds, 5) > 1000,
        "wind moves clouds while camera is fixed"
    );
    save_capture("environment-clouds", &clouds);
    save_capture("environment-clouds-later", &moved_clouds);

    frame.environment.enabled = false;
    frame.camera = Camera {
        eye: Vec3::ZERO,
        target: -Vec3::Z,
        fov_y_deg: 60.0,
    };
    frame.instances = vec![Instance::new(
        Vec3::new(0.0, 0.0, -5.0),
        Vec3::new(4.0, 4.0, 1.0),
        Vec3::splat(0.25),
    )];
    let base = rig.render(&frame, true);
    let particle = |z, color, opacity| Particle {
        position: Vec3::new(0.0, 0.0, z),
        size: glam::Vec2::splat(2.0),
        color,
        opacity,
    };
    frame.particles = vec![particle(-7.0, Vec3::new(0.0, 4.0, 0.0), 0.8)];
    assert_eq!(
        rig.render(&frame, true),
        base,
        "opaque wall occludes particles behind it"
    );
    frame.particles = vec![particle(-3.0, Vec3::new(3.0, 0.1, 0.0), 0.4)];
    let foreground = rig.render(&frame, true);
    assert!(
        changed_pixels(&base, &foreground, 8) > 100,
        "foreground particles alpha blend"
    );
    frame
        .particles
        .push(particle(-3.5, Vec3::new(0.0, 0.1, 3.0), 0.5));
    let sorted = rig.render(&frame, true);
    frame.particles.reverse();
    assert_eq!(
        rig.render(&frame, true),
        sorted,
        "upload order does not affect sorted alpha composition"
    );
    save_capture("environment-particles", &sorted);
}
