//! The upscaler on the graphics card.
//!
//! The window already draws with `wgpu`, so the device is there to use and
//! nothing has to be carried for it. A phone's graphics core is ten to
//! thirty times the arithmetic of its processor, which is what makes an
//! upscaler usable rather than a coffee break.
//!
//! Everything runs in one command buffer: the layers are dispatched one
//! after another and the picture comes back once at the end. Feature maps
//! go back into a pool as soon as nothing reads them, since a thousand
//! layers would otherwise ask for gigabytes. The CPU pass stays as the way
//! out when there is no device or a kernel is refused.
//!
//! See `docs/behavior/ai-tools.md`.

use super::param::{Conv, Layer, Weighed};
use super::net::Tensor;
use eframe::wgpu;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// One module a kernel: a single module may name a binding once, and four
// kernels each wanting their own uniform at group 0 binding 0 is four
// modules. Sharing one had every kernel collide with the next, the module
// failed to build, and the whole card path fell back to the processor
// without a word.
const CONV: &str = include_str!("conv.wgsl");
const ADD: &str = include_str!("add.wgsl");
const COPY: &str = include_str!("copy.wgsl");
const RESIZE: &str = include_str!("resize.wgsl");

/// The workgroup size every kernel in the shader is written for.
const PER_GROUP: usize = 64;
/// The most workgroups one axis of a dispatch may name.
const MAX_GROUPS: usize = 65535;

/// The device and queue the window draws with, kept once as the app starts.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

static GPU: OnceLock<Gpu> = OnceLock::new();

/// Hand the window's device over, once. Called as the app starts.
pub fn set_device(device: wgpu::Device, queue: wgpu::Queue) {
    let _ = GPU.set(Gpu { device, queue });
}

/// The device the upscalers run on: the window's when it draws with wgpu,
/// and one made here for the work when it does not.
///
/// The window draws with OpenGL on this build, so it has no wgpu device to
/// hand over and the card was reported as missing on a phone that has one
/// to spare. A device of our own is made instead, which leaves the drawing
/// exactly as it was: this one is only for arithmetic.
fn device() -> Option<&'static Gpu> {
    if let Some(gpu) = GPU.get() {
        return Some(gpu);
    }
    let gpu = made_here()?;
    let _ = GPU.set(gpu);
    GPU.get()
}

/// A device of this build's own, on the fastest adapter there is.
#[cfg(not(target_arch = "wasm32"))]
fn made_here() -> Option<Gpu> {
    // No display handle: this instance never draws anything, it only
    // computes, so it needs no window to be presentable to.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .map_err(|e| log::error!("upscaling: no graphics card to make a device on: {e}"))
    .ok()?;
    let info = adapter.get_info();
    log::error!(
        "upscaling: a device of its own on {} ({:?})",
        info.name,
        info.device_type
    );
    // The card's own limits, not the cautious ones a device comes with: a
    // whole picture goes into one storage buffer, and the default cap of
    // 128 MiB is under a large one. A bind group over that cap is a
    // validation error, which this build handles as fatal, so the app
    // stopped as soon as an upscale of a big picture began.
    let caps = adapter.limits();
    let limits = wgpu::Limits {
        max_storage_buffer_binding_size: caps.max_storage_buffer_binding_size,
        max_buffer_size: caps.max_buffer_size,
        ..wgpu::Limits::default()
    };
    log::info!(
        "upscaling: a storage buffer up to {} MiB, a buffer up to {} MiB",
        limits.max_storage_buffer_binding_size / (1024 * 1024),
        limits.max_buffer_size / (1024 * 1024)
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("upscale"),
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|e| log::error!("upscaling: the card would not make a device: {e}"))
    .ok()?;
    // A card that goes away says why, in the driver's own words: a tile that
    // reads back as nothing is otherwise the only sign of it.
    device.set_device_lost_callback(|reason, message| {
        log::error!("upscaling: the card was lost: {reason:?}: {message}");
    });
    Some(Gpu { device, queue })
}

/// The browser build always draws with wgpu, so its device is the window's.
#[cfg(target_arch = "wasm32")]
fn made_here() -> Option<Gpu> {
    None
}

/// Whether the window handed its device over. Only a check: making one
/// here is left to the work that needs it, so a build without a card does
/// not try to make one every frame.
pub fn available() -> bool {
    GPU.get().is_some()
}

/// One compute pass of the shader and the layout its bind group follows.
struct Kernel {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

fn uniform() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn read_only() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn read_write() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: false },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn kernel(
    device: &wgpu::Device,
    entry: &str,
    source: &'static str,
    kinds: &[wgpu::BindingType],
) -> Kernel {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(entry),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let entries: Vec<wgpu::BindGroupLayoutEntry> = kinds
        .iter()
        .enumerate()
        .map(|(i, ty)| wgpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: ty.clone(),
            count: None,
        })
        .collect();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(entry),
        entries: &entries,
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    Kernel { pipeline, layout }
}

/// A storage buffer of the size asked for, at least four bytes.
fn storage(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("upscale"),
        size: size.max(4),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Buffers kept for the next layer that wants one of their size or less.
///
/// They are shared: a split puts one buffer under several names, and a
/// buffer goes back only when the last name that held it lets go.
struct Pool {
    free: Vec<(u64, Arc<wgpu::Buffer>)>,
}

impl Pool {
    fn new() -> Pool {
        Pool { free: Vec::new() }
    }

    fn take(&mut self, device: &wgpu::Device, size: u64) -> Arc<wgpu::Buffer> {
        let size = size.max(4);
        if let Some(i) = self.free.iter().position(|(have, _)| *have >= size) {
            return self.free.swap_remove(i).1;
        }
        // A large one is worth a line: the whole picture goes through these
        // buffers, and a card that runs out says nothing before it stops.
        if size >= 32 * 1024 * 1024 {
            log::info!("upscaling: a buffer of {} MiB", size / (1024 * 1024));
        }
        Arc::new(storage(device, size))
    }

    fn give(&mut self, size: u64, buffer: Arc<wgpu::Buffer>) {
        // A buffer of the size the next tile asks for is kept, so the tile
        // after this one reuses it instead of asking the card for another:
        // the card does not hand a buffer back when it is dropped, and a
        // picture of forty tiles ran it out of memory that way. Anything
        // larger is left to the driver.
        if size <= 128 * 1024 * 1024 && self.free.len() < 64 {
            self.free.push((size.max(4), buffer));
        }
    }
}

/// One thing the network does on the card.
enum Step {
    Input {
        top: String,
    },
    Conv {
        bottom: String,
        top: String,
        weight: wgpu::Buffer,
        bias: wgpu::Buffer,
        params: wgpu::Buffer,
        in_c: usize,
        out_c: usize,
        kh: usize,
        kw: usize,
        pad: usize,
        slope: f32,
    },
    Split {
        bottom: String,
        tops: Vec<String>,
    },
    Concat {
        bottoms: Vec<String>,
        top: String,
        /// The channels each input brings.
        channels: Vec<usize>,
        out_c: usize,
    },
    Add {
        bottoms: Vec<String>,
        top: String,
        coeffs: Vec<f32>,
    },
    Resize {
        bottom: String,
        top: String,
        scale: usize,
    },
}

/// A model ready to run on the card.
pub struct GpuNet {
    device: wgpu::Device,
    queue: wgpu::Queue,
    conv: Kernel,
    add: Kernel,
    copy: Kernel,
    resize: Kernel,
    steps: Vec<Step>,
    out: String,
    frees: Vec<Vec<String>>,
    /// The buffers the last tile used, kept for the next one: a picture is
    /// a few dozen tiles of nearly one size, and making them afresh for
    /// every tile grew what the card held until it was lost.
    pool: std::sync::Mutex<Pool>,
    /// What the network does to one pixel: four for both models.
    pub scale: usize,
    /// The channels it reads.
    pub channels: usize,
}

impl GpuNet {
    /// Build the network, its weights and its kernels.
    ///
    /// A kernel the phone's driver will not take is a validation error, and
    /// an uncaught one stops the app. It is caught here instead, and the
    /// processor is what enlarges the picture after that.
    pub fn new(layers: &[Layer], weighed: &[Weighed]) -> Result<GpuNet, String> {
        let gpu = device().ok_or("no graphics card answered")?;
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let built = Self::build(device.clone(), queue, layers, weighed);
        if let Ok(net) = &built {
            // One pixel through the whole network: a kernel that is taken
            // but cannot run is found before a picture is sent to it.
            let _ = net.run(&Tensor::new(3, 1, 1, vec![0.5; 3]));
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        }
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(format!("the card refused the network: {e}"));
        }
        built
    }

    fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        layers: &[Layer],
        weighed: &[Weighed],
    ) -> Result<GpuNet, String> {
        if layers.len() != weighed.len() {
            return Err("the layers and their weights do not line up".to_string());
        }
        let conv = kernel(
            &device,
            "conv",
            CONV,
            &[uniform(), read_only(), read_only(), read_only(), read_write()],
        );
        let add = kernel(
            &device,
            "add",
            ADD,
            &[uniform(), read_only(), read_only(), read_write()],
        );
        let copy = kernel(
            &device,
            "copy_channels",
            COPY,
            &[uniform(), read_only(), read_write()],
        );
        let resize = kernel(
            &device,
            "resize",
            RESIZE,
            &[uniform(), read_only(), read_write()],
        );
        let mut steps = Vec::with_capacity(layers.len());
        let mut scale = 1usize;
        let mut channels = 3usize;
        let mut out = String::new();
        for (layer, w) in layers.iter().zip(weighed) {
            let bottoms = layer.bottoms.clone();
            let tops = layer.tops.clone();
            // Read before `tops` goes into the step.
            let last_top = tops.last().cloned();
            let step = match layer.kind.as_str() {
                "Input" => Step::Input {
                    top: tops.first().cloned().unwrap_or_default(),
                },
                "Convolution" => {
                    let Weighed::Conv(c) = w else {
                        return Err(format!("{}: a convolution with no weights", layer.name));
                    };
                    let weight = upload(&device, &queue, &turned(c));
                    let bias = upload(&device, &queue, &bias_of(c));
                    let params = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("conv params"),
                        size: 48,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    channels = c.out_c;
                    Step::Conv {
                        bottom: bottoms.first().cloned().unwrap_or_default(),
                        top: tops.first().cloned().unwrap_or_default(),
                        weight,
                        bias,
                        params,
                        in_c: c.in_c,
                        out_c: c.out_c,
                        kh: c.kh,
                        kw: c.kw,
                        pad: c.pad_h.max(c.pad_w),
                        slope: c.leaky.unwrap_or(1.0),
                    }
                }
                "Split" => Step::Split {
                    bottom: bottoms.first().cloned().unwrap_or_default(),
                    tops,
                },
                "Concat" => {
                    // The channels each input brings are filled in once the
                    // whole network is known (`fill_concat`).
                    Step::Concat {
                        bottoms,
                        top: tops.first().cloned().unwrap_or_default(),
                        channels: Vec::new(),
                        out_c: 0,
                    }
                }
                "Eltwise" | "BinaryOp" => Step::Add {
                    bottoms,
                    top: tops.first().cloned().unwrap_or_default(),
                    coeffs: coeffs_of(layer),
                },
                "Interp" => {
                    let s = layer
                        .floats(1)
                        .first()
                        .copied()
                        .unwrap_or(1.0)
                        .round()
                        .max(1.0) as usize;
                    scale = scale.saturating_mul(s);
                    Step::Resize {
                        bottom: bottoms.first().cloned().unwrap_or_default(),
                        top: tops.first().cloned().unwrap_or_default(),
                        scale: s,
                    }
                }
                other => {
                    return Err(format!(
                        "{}: a {other} layer, which the upscalers do not use",
                        layer.name
                    ))
                }
            };
            if let Some(top) = last_top {
                if top == "output" {
                    out = top;
                }
            }
            steps.push(step);
        }
        if out.is_empty() {
            out = steps
                .iter()
                .rev()
                .find_map(|s| match s {
                    Step::Input { .. } => None,
                    Step::Conv { top, .. }
                    | Step::Concat { top, .. }
                    | Step::Add { top, .. }
                    | Step::Resize { top, .. } => Some(top.clone()),
                    Step::Split { tops, .. } => tops.last().cloned(),
                })
                .ok_or("the network has no output")?;
        }
        let frees = free_lists(&steps, &out);
        let mut net = GpuNet {
            device,
            queue,
            conv,
            add,
            copy,
            resize,
            steps,
            out,
            frees,
            pool: std::sync::Mutex::new(Pool::new()),
            scale,
            channels,
        };
        net.fill_concat();
        Ok(net)
    }

    /// A concat needs the channels its inputs bring, which the shapes the
    /// layers before it make are the only record of. Walked once here, since
    /// a layer's channels never change.
    fn fill_concat(&mut self) {
        let mut channels: HashMap<String, usize> = HashMap::new();
        let input_channels = self.channels;
        for i in 0..self.steps.len() {
            let made: Vec<(String, usize)> = match &mut self.steps[i] {
                Step::Input { top } => vec![(top.clone(), input_channels)],
                Step::Conv { top, out_c, .. } => vec![(top.clone(), *out_c)],
                Step::Split { bottom, tops } => {
                    let c = channels.get(bottom).copied().unwrap_or(0);
                    tops.iter().map(|t| (t.clone(), c)).collect()
                }
                Step::Concat {
                    bottoms,
                    top,
                    channels: brought,
                    out_c,
                } => {
                    let list: Vec<usize> = bottoms
                        .iter()
                        .map(|b| channels.get(b).copied().unwrap_or(0))
                        .collect();
                    let total: usize = list.iter().sum();
                    *out_c = total;
                    *brought = list;
                    vec![(top.clone(), total)]
                }
                Step::Add { bottoms, top, .. } => {
                    let c = bottoms
                        .first()
                        .and_then(|b| channels.get(b).copied())
                        .unwrap_or(0);
                    vec![(top.clone(), c)]
                }
                Step::Resize { bottom, top, .. } => {
                    let c = channels.get(bottom).copied().unwrap_or(0);
                    vec![(top.clone(), c)]
                }
            };
            for (name, c) in made {
                channels.insert(name, c);
            }
        }
    }

    /// Run a picture, channel last, `(h, w, c)`.
    pub fn run(&self, input: &Tensor) -> Result<Tensor, String> {
        let (h, w) = (input.h, input.w);
        if h == 0 || w == 0 || input.c != self.channels {
            return Err("the picture is not the shape the network reads".to_string());
        }
        log::info!(
            "upscaling on the card: a {w} x {h} tile, {} channels, {} steps",
            input.c,
            self.steps.len()
        );
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        let mut blobs: HashMap<String, (Arc<wgpu::Buffer>, usize, usize, usize)> = HashMap::new();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("upscale"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("upscale"),
                timestamp_writes: None,
            });
            let input_buffer = pool.take(&self.device, (input.data.len() * 4) as u64);
            self.queue
                .write_buffer(&input_buffer, 0, &bytes_of(&input.data));
            for (index, step) in self.steps.iter().enumerate() {
                match step {
                    Step::Input { top } => {
                        blobs.insert(top.clone(), (input_buffer.clone(), h, w, input.c));
                    }
                    Step::Conv {
                        bottom,
                        top,
                        weight,
                        bias,
                        params,
                        in_c,
                        out_c,
                        kh,
                        kw,
                        pad,
                        slope,
                    } => {
                        let Some((src, sh, sw, sc)) = blobs.get(bottom).cloned() else {
                            return Err(format!("{bottom} was never made"));
                        };
                        if sc != *in_c {
                            return Err(format!("{bottom} has {sc} channels, not {in_c}"));
                        }
                        let total = sh * sw * *out_c;
                        let size = (total * 4) as u64;
                        let dst = pool.take(&self.device, size);
                        let (gx, gy) = groups(total);
                        self.queue.write_buffer(
                            params,
                            0,
                            &u32s(&[
                                sw as u32,
                                sh as u32,
                                *in_c as u32,
                                *out_c as u32,
                                *pad as u32,
                                *kh as u32,
                                *kw as u32,
                                slope.to_bits(),
                                gx,
                                0,
                                0,
                                0,
                            ]),
                        );
                        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("conv"),
                            layout: &self.conv.layout,
                            entries: &[
                                entry(0, params),
                                entry_arc(1, &src),
                                entry(2, weight),
                                entry(3, bias),
                                entry_arc(4, &dst),
                            ],
                        });
                        pass.set_pipeline(&self.conv.pipeline);
                        pass.set_bind_group(0, &bind, &[]);
                        pass.dispatch_workgroups(gx, gy, 1);
                        blobs.insert(top.clone(), (dst, sh, sw, *out_c));
                    }
                    Step::Split { bottom, tops } => {
                        let Some(src) = blobs.get(bottom).cloned() else {
                            return Err(format!("{bottom} was never made"));
                        };
                        for top in tops {
                            blobs.insert(top.clone(), src.clone());
                        }
                    }
                    Step::Concat {
                        bottoms,
                        top,
                        channels,
                        out_c,
                    } => {
                        let first = bottoms.first().and_then(|b| blobs.get(b)).cloned();
                        let Some((_, sh, sw, _)) = first else {
                            return Err("a concat with no input".to_string());
                        };
                        let total = sh * sw * *out_c;
                        let size = (total * 4) as u64;
                        let dst = pool.take(&self.device, size);
                        let mut at = 0usize;
                        for (name, c) in bottoms.iter().zip(channels) {
                            let Some((src, _, _, _)) = blobs.get(name).cloned() else {
                                return Err(format!("{name} was never made"));
                            };
                            let px = sh * sw;
                            let n = px * *c;
                            let (gx, gy) = groups(n);
                            let params = self.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("copy params"),
                                size: 32,
                                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            self.queue.write_buffer(
                                &params,
                                0,
                                &u32s(&[
                                    px as u32,
                                    *c as u32,
                                    *out_c as u32,
                                    at as u32,
                                    gx,
                                    0,
                                    0,
                                    0,
                                ]),
                            );
                            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("copy"),
                                layout: &self.copy.layout,
                                entries: &[entry(0, &params), entry_arc(1, &src), entry_arc(2, &dst)],
                            });
                            pass.set_pipeline(&self.copy.pipeline);
                            pass.set_bind_group(0, &bind, &[]);
                            pass.dispatch_workgroups(gx, gy, 1);
                            at += *c;
                        }
                        blobs.insert(top.clone(), (dst, sh, sw, *out_c));
                    }
                    Step::Add {
                        bottoms,
                        top,
                        coeffs,
                    } => {
                        let Some((a, sh, sw, sc)) = bottoms
                            .first()
                            .and_then(|b| blobs.get(b))
                            .cloned()
                        else {
                            return Err("an add with no input".to_string());
                        };
                        let Some((b, _, _, _)) = bottoms.get(1).and_then(|n| blobs.get(n)).cloned()
                        else {
                            return Err("an add with one input".to_string());
                        };
                        let total = sh * sw * sc;
                        let size = (total * 4) as u64;
                        let dst = pool.take(&self.device, size);
                        let (gx, gy) = groups(total);
                        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("add params"),
                            size: 32,
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        self.queue.write_buffer(
                            &params,
                            0,
                            &u32s(&[
                                total as u32,
                                coeffs.first().copied().unwrap_or(1.0).to_bits(),
                                coeffs.get(1).copied().unwrap_or(1.0).to_bits(),
                                0,
                                gx,
                                0,
                                0,
                                0,
                            ]),
                        );
                        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("add"),
                            layout: &self.add.layout,
                            entries: &[
                                entry(0, &params),
                                entry_arc(1, &a),
                                entry_arc(2, &b),
                                entry_arc(3, &dst),
                            ],
                        });
                        pass.set_pipeline(&self.add.pipeline);
                        pass.set_bind_group(0, &bind, &[]);
                        pass.dispatch_workgroups(gx, gy, 1);
                        blobs.insert(top.clone(), (dst, sh, sw, sc));
                    }
                    Step::Resize {
                        bottom,
                        top,
                        scale,
                    } => {
                        let Some((src, sh, sw, sc)) = blobs.get(bottom).cloned() else {
                            return Err(format!("{bottom} was never made"));
                        };
                        let oh = sh * scale;
                        let ow = sw * scale;
                        let total = oh * ow * sc;
                        let size = (total * 4) as u64;
                        // A buffer larger than the card takes in one binding
                        // is a validation error, and this build stops the app
                        // on one: a picture too large for the card is reported
                        // instead, and the caller shows why.
                        let cap = self.device.limits().max_storage_buffer_binding_size as u64;
                        if size > cap {
                            return Err(format!(
                                "an enlarged picture of {ow} x {oh} needs {size} bytes in one buffer, and this card takes {cap}"
                            ));
                        }
                        let dst = pool.take(&self.device, size);
                        let (gx, gy) = groups(total);
                        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("resize params"),
                            size: 32,
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        self.queue.write_buffer(
                            &params,
                            0,
                            &u32s(&[
                                sw as u32,
                                sh as u32,
                                sc as u32,
                                *scale as u32,
                                gx,
                                0,
                                0,
                                0,
                            ]),
                        );
                        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("resize"),
                            layout: &self.resize.layout,
                            entries: &[entry(0, &params), entry_arc(1, &src), entry_arc(2, &dst)],
                        });
                        pass.set_pipeline(&self.resize.pipeline);
                        pass.set_bind_group(0, &bind, &[]);
                        pass.dispatch_workgroups(gx, gy, 1);
                        blobs.insert(top.clone(), (dst, oh, ow, sc));
                    }
                }
                if let Some(names) = self.frees.get(index) {
                    for name in names {
                        if let Some((buffer, h, w, c)) = blobs.remove(name) {
                            // A split leaves one buffer under several names:
                            // it goes back only when the last of them does.
                            if Arc::strong_count(&buffer) == 1 {
                                pool.give((h * w * c * 4) as u64, buffer);
                            }
                        }
                    }
                }
            }
        }
        let Some((result, oh, ow, oc)) = blobs.remove(&self.out) else {
            return Err(format!("the network never made {}", self.out));
        };
        let total = oh * ow * oc;
        let size = (total * 4) as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("upscale read"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&result, 0, &staging, 0, size);
        self.queue.submit(Some(encoder.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.recv()
            .map_err(|_| "the picture never came back".to_string())?
            .map_err(|e| format!("the picture could not be read: {e}"))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| format!("the picture could not be read: {e}"))?;
        let mut out = Vec::with_capacity(total);
        for i in 0..total {
            let o = i * 4;
            out.push(f32::from_le_bytes([
                data[o],
                data[o + 1],
                data[o + 2],
                data[o + 3],
            ]));
        }
        drop(data);
        staging.unmap();
        // What the tile still holds goes back for the next one. A picture is
        // cut into tiles of a few sizes, and the first of each size would
        // otherwise leave the card holding its buffers for good: a tile two
        // pixels wider than the one before asked for buffers of its own, and
        // the two sizes together ran the card out.
        for (_, (buffer, h, w, c)) in blobs.drain() {
            if Arc::strong_count(&buffer) == 1 {
                pool.give((h * w * c * 4) as u64, buffer);
            }
        }
        Ok(Tensor::new(oc, oh, ow, out))
    }
}

/// What a step reads.
fn bottoms_of(step: &Step) -> &[String] {
    match step {
        Step::Input { .. } => &[],
        Step::Conv { bottom, .. } => std::slice::from_ref(bottom),
        Step::Split { bottom, .. } => std::slice::from_ref(bottom),
        Step::Concat { bottoms, .. } | Step::Add { bottoms, .. } => bottoms,
        Step::Resize { bottom, .. } => std::slice::from_ref(bottom),
    }
}

/// For each step, the blobs read for the last time there.
fn free_lists(steps: &[Step], out: &str) -> Vec<Vec<String>> {
    let mut last: HashMap<String, usize> = HashMap::new();
    for (i, step) in steps.iter().enumerate() {
        for bottom in bottoms_of(step) {
            last.insert(bottom.clone(), i);
        }
    }
    let mut frees = vec![Vec::new(); steps.len()];
    for (name, i) in last {
        if name != out {
            if let Some(slot) = frees.get_mut(i) {
                slot.push(name);
            }
        }
    }
    frees
}

fn entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn entry_arc<'a>(binding: u32, buffer: &'a Arc<wgpu::Buffer>) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// How many workgroups a flat count of elements takes, split over two axes
/// since one axis may not name more than 65535 of them.
fn groups(total: usize) -> (u32, u32) {
    let n = (total + PER_GROUP - 1) / PER_GROUP;
    let gx = n.min(MAX_GROUPS).max(1);
    let gy = ((n + gx - 1) / gx).max(1);
    (gx as u32, gy as u32)
}

fn bytes_of(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn u32s(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, values: &[f32]) -> wgpu::Buffer {
    let buffer = storage(device, (values.len() * 4) as u64);
    queue.write_buffer(&buffer, 0, &bytes_of(values));
    buffer
}

/// The weights as the shader reads them: `[out][kh][kw][in]`.
fn turned(c: &Conv) -> Vec<f32> {
    let (oc, ic, kh, kw) = (c.out_c, c.in_c, c.kh, c.kw);
    if c.weight.len() != oc * ic * kh * kw {
        return c.weight.clone();
    }
    let mut out = vec![0f32; c.weight.len()];
    for o in 0..oc {
        for ci in 0..ic {
            for ky in 0..kh {
                for kx in 0..kw {
                    out[((o * kh + ky) * kw + kx) * ic + ci] =
                        c.weight[((o * ic + ci) * kh + ky) * kw + kx];
                }
            }
        }
    }
    out
}

/// The bias, or zeros when the layer has none: the shader always adds one.
fn bias_of(c: &Conv) -> Vec<f32> {
    match &c.bias {
        Some(b) => b.clone(),
        None => vec![0.0; c.out_c],
    }
}

/// The coefficients an element-wise layer weights its inputs by.
fn coeffs_of(layer: &Layer) -> Vec<f32> {
    for key in [-23301, 1] {
        let list = layer.floats(key);
        if list.len() >= 2 {
            return list[1..].to_vec();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_is_split_over_two_axes() {
        assert_eq!(groups(64), (1, 1));
        assert_eq!(groups(65), (2, 1));
        assert_eq!(groups(64 * 65535), (65535, 1));
        let (gx, gy) = groups(64 * 65535 * 3);
        assert_eq!(gx, 65535);
        assert_eq!(gy, 3);
        assert!(gx as usize * gy as usize * PER_GROUP >= 64 * 65535 * 3);
    }

    #[test]
    fn the_weights_turn_to_kernel_major() {
        // Two outputs, one input, a 1x2 kernel: [o0k0, o0k1, o1k0, o1k1].
        let c = Conv {
            out_c: 2,
            in_c: 1,
            kh: 1,
            kw: 2,
            stride: 1,
            pad_h: 0,
            pad_w: 0,
            weight: vec![1.0, 2.0, 3.0, 4.0],
            bias: None,
            leaky: None,
        };
        assert_eq!(turned(&c), vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(bias_of(&c), vec![0.0, 0.0]);
    }
}
