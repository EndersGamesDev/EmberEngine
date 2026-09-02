//! Browser-only GL device, surface, scope, and first-clear initialization.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use ember_julibrot_present::CLASSIC_PALETTE;
use ember_lab_heap::{install_logging_handler, publish_browser_error};
use wasm_bindgen::{JsCast, JsValue};

use crate::{AppError, SurfaceState};

const STATUS_ID: &str = "status";
const INITIAL_WIDTH: u32 = 960;
const INITIAL_HEIGHT: u32 = 540;

thread_local! {
    static PANIC_HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Adapter and floor facts available before sibling construction.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFacts {
    /// Human-readable adapter name.
    pub adapter_name: String,
    /// Selected backend label, required to be `Gl`.
    pub backend: String,
    /// Whether all required RGBA32F usages were exposed.
    pub rgba32f_renderable: bool,
    /// Configured surface width.
    pub width: u32,
    /// Configured surface height.
    pub height: u32,
}

/// App-owned browser device and sole surface.
pub struct BrowserRuntime {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    lost: Arc<Mutex<Option<String>>>,
    surfaces: SurfaceState<wgpu::SurfaceTexture>,
    facts: DeviceFacts,
}

impl BrowserRuntime {
    /// Selects only GL/WebGL2, installs handlers, scopes initialization, and presents one clear
    /// colour frame before any scene exists.
    ///
    /// # Errors
    ///
    /// Returns a typed browser, capability, device, surface, or validation-scope failure.
    pub async fn start(canvas_id: &str, status_id: &str) -> Result<Self, AppError> {
        install_julibrot_panic_hook();
        if status_id != STATUS_ID {
            return Err(AppError::Capability {
                operation: "page contract",
                detail: format!("status id must be {STATUS_ID}, got {status_id}"),
            });
        }
        let canvas = canvas_by_id(canvas_id)?;
        validate_webgl2_floor()?;
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| AppError::Surface {
                detail: format!("creation failed: {error}"),
            })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| AppError::Capability {
                operation: "adapter selection",
                detail: "no WebGL2 adapter".to_string(),
            })?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Gl {
            return Err(AppError::Capability {
                operation: "adapter selection",
                detail: format!("requested GL but selected {:?}", info.backend),
            });
        }
        validate_adapter_floor(&adapter)?;
        let adapter_limits = adapter.limits();
        let required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter_limits);
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Julibrot GL-only device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| AppError::Capability {
                operation: "device request",
                detail: error.to_string(),
            })?;

        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        install_logging_handler(&device, "Julibrot");

        let scope = ValidationScope::begin(&device, 0, "initialization and surface selection");
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| AppError::Capability {
                operation: "surface selection",
                detail: "surface exposes no format".to_string(),
            })?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| AppError::Capability {
                operation: "surface selection",
                detail: "surface exposes no present mode".to_string(),
            })?;
        let alpha_mode =
            capabilities
                .alpha_modes
                .first()
                .copied()
                .ok_or_else(|| AppError::Capability {
                    operation: "surface selection",
                    detail: "surface exposes no alpha mode".to_string(),
                })?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let mut runtime = Self {
            surface,
            config,
            device: Arc::new(device),
            queue: Arc::new(queue),
            lost,
            surfaces: SurfaceState::new(),
            facts: DeviceFacts {
                adapter_name: info.name,
                backend: format!("{:?}", info.backend),
                rgba32f_renderable: true,
                width,
                height,
            },
        };
        runtime.clear_first_frame(0)?;
        scope.finish().await?;
        Ok(runtime)
    }

    /// Returns immutable initialization facts for the honesty overlay.
    #[must_use]
    pub const fn facts(&self) -> &DeviceFacts {
        &self.facts
    }

    /// Returns the selected device for sibling construction after handlers are installed.
    #[must_use]
    pub fn device(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    /// Returns the selected queue for sibling construction after handlers are installed.
    #[must_use]
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Returns the configured surface format.
    #[must_use]
    pub const fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Returns a device-lost failure if the callback has supplied one.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::DeviceLost`] after loss, including callback detail.
    pub fn check_device(&self, operation: &'static str) -> Result<(), AppError> {
        let reason = self.lost.lock().ok().and_then(|slot| slot.clone());
        reason.map_or(Ok(()), |detail| {
            Err(AppError::DeviceLost { operation, detail })
        })
    }

    fn clear_first_frame(&mut self, generation: u32) -> Result<(), AppError> {
        self.check_device("clear first frame")?;
        self.surfaces.claim(generation)?;
        let frame = match self.acquire_surface_texture() {
            Ok(frame) => frame,
            Err(error) => {
                let released = self.surfaces.release_unsubmitted(generation);
                debug_assert!(released, "failed acquisition must release its owner");
                return Err(error);
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot clear-first-frame encoder"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Julibrot honest initial clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_colour()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
        let released = self.surfaces.release_unsubmitted(generation);
        debug_assert!(released, "presented initial clear must release its owner");
        Ok(())
    }

    fn acquire_surface_texture(&mut self) -> Result<wgpu::SurfaceTexture, AppError> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(frame) => Ok(frame),
                    Err(wgpu::SurfaceError::Timeout) => Err(AppError::SurfaceSkipped {
                        detail: "timed out after one reconfiguration".to_string(),
                    }),
                    Err(error) => Err(AppError::Surface {
                        detail: format!("acquisition after reconfiguration failed: {error}"),
                    }),
                }
            }
            Err(wgpu::SurfaceError::Timeout) => Err(AppError::SurfaceSkipped {
                detail: "acquisition timed out".to_string(),
            }),
            Err(error) => Err(AppError::Surface {
                detail: error.to_string(),
            }),
        }
    }
}

struct ValidationScope {
    device: wgpu::Device,
    generation: u64,
    operation: &'static str,
}

impl ValidationScope {
    fn begin(device: &wgpu::Device, generation: u64, operation: &'static str) -> Self {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        Self {
            device: device.clone(),
            generation,
            operation,
        }
    }

    async fn finish(self) -> Result<(), AppError> {
        self.device.pop_error_scope().await.map_or(Ok(()), |error| {
            Err(AppError::CapturedGpu {
                operation: self.operation,
                generation: self.generation,
                detail: error.to_string(),
            })
        })
    }
}

fn canvas_by_id(canvas_id: &str) -> Result<web_sys::HtmlCanvasElement, AppError> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(canvas_id))
        .ok_or_else(|| AppError::Capability {
            operation: "page lookup",
            detail: format!("canvas {canvas_id} was not found"),
        })?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| AppError::Capability {
            operation: "page lookup",
            detail: format!("element {canvas_id} is not a canvas"),
        })
}

fn validate_webgl2_floor() -> Result<(), AppError> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: "window document is unavailable".to_string(),
        })?;
    let probe = document
        .create_element("canvas")
        .map_err(|error| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: format!("probe canvas creation failed: {error:?}"),
        })?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: "probe element is not a canvas".to_string(),
        })?;
    probe.set_width(1);
    probe.set_height(1);
    let context = probe
        .get_context("webgl2")
        .map_err(|error| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: format!("context creation failed: {error:?}"),
        })?
        .ok_or_else(|| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: "WebGL2 is unavailable".to_string(),
        })?
        .dyn_into::<web_sys::WebGl2RenderingContext>()
        .map_err(|_| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: "browser returned a non-WebGL2 context".to_string(),
        })?;
    let extension = context
        .get_extension("EXT_color_buffer_float")
        .map_err(|error| AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: format!("extension query failed: {error:?}"),
        })?;
    if extension.is_none() {
        return Err(AppError::Capability {
            operation: "WebGL2 floor probe",
            detail: "EXT_color_buffer_float is unavailable".to_string(),
        });
    }
    Ok(())
}

fn validate_adapter_floor(adapter: &wgpu::Adapter) -> Result<(), AppError> {
    let required_usage = wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST;
    let rgba = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
    if !rgba.allowed_usages.contains(required_usage) {
        return Err(AppError::Capability {
            operation: "RGBA32F floor probe",
            detail: format!(
                "allowed usages {:?} omit required {:?}",
                rgba.allowed_usages, required_usage
            ),
        });
    }
    let limits = adapter.limits();
    if limits.max_color_attachments < 4
        || limits.max_texture_dimension_2d < 2_048
        || limits.max_texture_array_layers < 256
        || limits.max_uniform_buffer_binding_size < 16 * 1_024
        || limits.max_sampled_textures_per_shader_stage < 16
    {
        return Err(AppError::Capability {
            operation: "WebGL2 minimum limits",
            detail: format!(
                "colour={} dimension={} layers={} uniform={} sampled={}",
                limits.max_color_attachments,
                limits.max_texture_dimension_2d,
                limits.max_texture_array_layers,
                limits.max_uniform_buffer_binding_size,
                limits.max_sampled_textures_per_shader_stage
            ),
        });
    }
    Ok(())
}

fn clear_colour() -> wgpu::Color {
    let [r, g, b, a] = CLASSIC_PALETTE.clear_rgba;
    wgpu::Color {
        r: f64::from(r),
        g: f64::from(g),
        b: f64::from(b),
        a: f64::from(a),
    }
}

/// Installs the page panic hook before any exported startup path can request a device.
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn install_julibrot_panic_hook() {
    let already_installed = PANIC_HOOK_INSTALLED.with(|flag| flag.replace(true));
    if already_installed {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let location = info.location().map_or_else(
            || "unknown location".to_string(),
            |location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            },
        );
        let message = format!("Julibrot panic at {location}: {payload}");
        LAST_PANIC.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                *slot = Some(message.clone());
            }
        });
        publish_browser_error(&message);
    }));
}

/// Returns and clears the most recent panic message.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn take_julibrot_panic() -> Option<String> {
    LAST_PANIC.with(|slot| {
        slot.try_borrow_mut()
            .ok()
            .and_then(|mut message| message.take())
    })
}

/// Publishes a typed startup failure to the fixed status element and JavaScript rejection.
pub fn publish_start_error(error: &AppError) -> JsValue {
    let message = error.to_string();
    publish_browser_error(&message);
    JsValue::from_str(&message)
}

#[allow(dead_code, reason = "documents the fixed initial fallback dimensions")]
const _: [u32; 2] = [INITIAL_WIDTH, INITIAL_HEIGHT];
