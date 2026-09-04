//! Offscreen regression checks use the shipping pipeline builders and WGSL.
//! Run explicitly with `cargo test -p ember-engine environment_gpu -- --ignored`.
//! Optional `EMBER_ENV_CAPTURE_DIR` saves diagnostic PNGs; no window is created.

use super::*;

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
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test scene pipeline layout"),
            bind_group_layouts: &[&uniform_layout, &mesh_layout, &shadow_layout],
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
        let instances: Vec<InstanceRaw> = frame
            .instances
            .iter()
            .map(|instance| InstanceRaw {
                pos: instance.position.to_array(),
                scale: instance.scale.to_array(),
                color: instance.color.to_array(),
                rot: instance.rot.to_array(),
                material: [
                    f32::from(u8::from(instance.wettable)),
                    f32::from(u8::from(instance.casts_shadow)),
                ],
            })
            .collect();
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
