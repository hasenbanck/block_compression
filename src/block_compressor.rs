use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use bytemuck::{cast_slice, Pod, Zeroable};
use wgpu::{
    self, include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ComputePass, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineCompilationOptions, PipelineLayoutDescriptor, Queue,
    ShaderModule, ShaderStages, TextureSampleType, TextureView, TextureViewDimension,
};

use crate::{
    settings::{BC6HSettings, Settings},
    BC7Settings, CompressionVariantLDR,
};

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct Uniforms {
    /// The width of the image data.
    width: u32,
    /// The height of the image data.
    height: u32,
    /// Start of the blocks data in u32 elements.
    blocks_offset: u32,
}

struct Task {
    variant: CompressionVariantLDR,
    width: u32,
    height: u32,
    uniform_offset: u32,
    setting_offset: u32,
    buffer_offset: u32,
    bind_group: BindGroup,
    settings: Option<Settings>,
}

/// Compresses texture data with a block compression algorithm using WGPU compute shader.
pub struct BlockCompressor {
    scratch_buffer: Vec<u8>,
    task: Vec<Task>,
    uniforms_buffer: Buffer,
    bc6h_settings_buffer: Buffer,
    bc7_settings_buffer: Buffer,
    bind_group_layouts: HashMap<CompressionVariantLDR, BindGroupLayout>,
    pipelines: HashMap<CompressionVariantLDR, ComputePipeline>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniforms_aligned_size: usize,
    bc6h_aligned_size: usize,
    bc7_aligned_size: usize,
}

impl BlockCompressor {
    /// Creates a new block compressor instance.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let limits = device.limits();

        let alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let size = size_of::<Uniforms>();
        let uniforms_aligned_size = size.div_ceil(alignment) * alignment;

        let alignment = limits.min_storage_buffer_offset_alignment as usize;
        let size = size_of::<BC6HSettings>();
        let bc6h_aligned_size = size.div_ceil(alignment) * alignment;

        let alignment = limits.min_storage_buffer_offset_alignment as usize;
        let size = size_of::<BC7Settings>();
        let bc7_aligned_size = size.div_ceil(alignment) * alignment;

        let shader_module_bc1_to_5 =
            device.create_shader_module(include_wgsl!("shader/BC1_to_5.wgsl"));
        let shader_module_bc6h = device.create_shader_module(include_wgsl!("shader/BC6H.wgsl"));
        let shader_module_bc7 = device.create_shader_module(include_wgsl!("shader/BC7.wgsl"));

        let uniforms_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("uniforms"),
            size: (uniforms_aligned_size * 16) as _,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let bc6h_settings_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("bc6h settings"),
            size: (bc6h_aligned_size * 16) as _,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let bc7_settings_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("bc7 settings"),
            size: (bc7_aligned_size * 16) as _,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let mut bind_group_layouts = HashMap::new();
        let mut pipelines = HashMap::new();

        Self::create_pipeline(
            &device,
            &shader_module_bc1_to_5,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC1,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc1_to_5,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC2,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc1_to_5,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC3,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc1_to_5,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC4,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc1_to_5,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC5,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc6h,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC6H,
        );
        Self::create_pipeline(
            &device,
            &shader_module_bc7,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariantLDR::BC7,
        );

        Self {
            scratch_buffer: Vec::default(),
            task: Vec::default(),
            uniforms_buffer,
            bc6h_settings_buffer,
            bc7_settings_buffer,
            bind_group_layouts,
            pipelines,
            device,
            queue,
            uniforms_aligned_size,
            bc6h_aligned_size,
            bc7_aligned_size,
        }
    }

    fn create_pipeline(
        device: &Device,
        shader_module: &ShaderModule,
        bind_group_layouts: &mut HashMap<CompressionVariantLDR, BindGroupLayout>,
        pipelines: &mut HashMap<CompressionVariantLDR, ComputePipeline>,
        variant: CompressionVariantLDR,
    ) {
        let mut layout_entries = vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            },
        ];

        match variant {
            CompressionVariantLDR::BC6H => {
                layout_entries.push(BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(size_of::<BC6HSettings>() as _),
                    },
                    count: None,
                });
            }
            CompressionVariantLDR::BC7 => {
                layout_entries.push(BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(size_of::<BC7Settings>() as _),
                    },
                    count: None,
                });
            }
            _ => {}
        }

        let name = variant.name();

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&format!("{name} bind group layout")),
            entries: &layout_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{name} block compression pipeline layout")),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(&format!("{name} block compression pipeline")),
            layout: Some(&pipeline_layout),
            module: shader_module,
            entry_point: Some(variant.entry_point()),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        bind_group_layouts.insert(variant, bind_group_layout);
        pipelines.insert(variant, pipeline);
    }

    /// Adds an 8-bit LDR texture compression task to the queue.
    ///
    /// This API is designed to be very flexible. For example, it is possible to fill the mip map
    /// levels of a texture with multiple calls to this function.
    ///
    /// # Texture View Requirements
    /// The source texture should provide enough channels for the texture compression. If only a
    /// single red channel is provided and BC1 is used, only the red channel will be properly
    /// encoded. All texture compression need to work on the raw texture data. The texture can
    /// use a sRGB texture format, but it needs to provide a view with a non-sRGB texture format.
    /// For example for a texture with a `Rgba8UnormSrgb` texture format, you will need to provide
    /// a texture view with the `Rgba8Unorm` format.
    ///
    /// # Buffer Requirements
    /// The destination buffer must have sufficient capacity to store the compressed blocks at the
    /// specified offset. The required size can be calculated using
    /// [`CompressionVariantLDR::blocks_byte_size()`].
    ///
    /// For example:
    ///
    /// ```ignore
    /// let required_size = variant.blocks_byte_size(width, height);
    /// let total_size = offset + required_size;
    /// assert!(buffer.size() >= total_size);
    /// ```
    ///
    /// # Arguments
    /// * `variant` - The block compression format to use
    /// * `texture_view` - View into the source texture to compress
    /// * `width` - Width of the texture view in pixels
    /// * `height` - Height of the texture view in pixels
    /// * `buffer` - Destination storage buffer for the compressed data
    /// * `offset` - Optional offset in bytes into the destination buffer
    /// * `settings` - Optional compression settings for BC6H/BC7.
    ///                If none provided, defaults to the slowest preset
    ///
    /// # Panics
    /// - If `width` or `height` is not a multiple of 4
    /// - If the destination `buffer` is not a storage buffer
    /// - If the destination `buffer` is too small to hold the compressed blocks at the specified offset
    /// - If the wrong `settings` where used for a variant with settings
    #[allow(clippy::too_many_arguments)]
    pub fn add_compression_task_ldr(
        &mut self,
        variant: CompressionVariantLDR,
        texture_view: &TextureView,
        width: u32,
        height: u32,
        buffer: &Buffer,
        offset: Option<u32>,
        settings: impl Into<Option<Settings>>,
    ) {
        let mut settings = settings.into();

        if variant == CompressionVariantLDR::BC6H {
            match &settings {
                None => settings = Some(Settings::BC6H(BC6HSettings::very_slow())),
                Some(Settings::BC6H(..)) => { /* Nothing to do */ }
                Some(Settings::BC7(..)) => {
                    panic!("BC7 settings cannot be used with BC6H variant")
                }
            }
        }

        if variant == CompressionVariantLDR::BC7 {
            match &settings {
                None => settings = Some(Settings::BC7(BC7Settings::alpha_slow())),
                Some(Settings::BC6H(..)) => {
                    panic!("BC6H settings cannot be used with BC7 variant")
                }
                Some(Settings::BC7(..)) => { /* Nothing to do */ }
            }
        }

        self.add_task(
            variant,
            texture_view,
            width,
            height,
            buffer,
            offset,
            settings,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_task(
        &mut self,
        variant: CompressionVariantLDR,
        texture_view: &TextureView,
        width: u32,
        height: u32,
        buffer: &Buffer,
        offset: Option<u32>,
        settings: Option<Settings>,
    ) {
        assert_eq!(height % 4, 0);
        assert_eq!(width % 4, 0);
        assert!(
            buffer.usage().contains(BufferUsages::STORAGE),
            "buffer needs to be a storage buffer"
        );

        let required_size = variant.blocks_byte_size(width, height);
        let total_size = offset.unwrap_or(0) as usize + required_size;

        assert!(
            buffer.size() as usize >= total_size,
            "buffer size ({}) is too small to hold compressed blocks at offset {}. Required size: {}",
            buffer.size(),
            offset.unwrap_or(0),
            total_size
        );

        let bind_group_layout = self
            .bind_group_layouts
            .get(&variant)
            .expect("Can't find bind group layout for variant");

        let bind_group = match variant {
            CompressionVariantLDR::BC1
            | CompressionVariantLDR::BC2
            | CompressionVariantLDR::BC3
            | CompressionVariantLDR::BC4
            | CompressionVariantLDR::BC5 => self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("bind group"),
                layout: bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.uniforms_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.uniforms_aligned_size as u64).unwrap()),
                        }),
                    },
                ],
            }),
            CompressionVariantLDR::BC6H => self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("bind group"),
                layout: bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.uniforms_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.uniforms_aligned_size as u64).unwrap()),
                        }),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.bc6h_settings_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.bc6h_aligned_size as u64).unwrap()),
                        }),
                    },
                ],
            }),
            CompressionVariantLDR::BC7 => self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("bind group"),
                layout: bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.uniforms_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.uniforms_aligned_size as u64).unwrap()),
                        }),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.bc7_settings_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.bc7_aligned_size as u64).unwrap()),
                        }),
                    },
                ],
            }),
        };

        self.task.push(Task {
            variant,
            width,
            height,
            uniform_offset: 0,
            setting_offset: 0,
            buffer_offset: offset.unwrap_or(0),
            bind_group,
            settings,
        });
    }

    fn update_buffer_sizes(&mut self) {
        let total_uniforms_size = self.uniforms_aligned_size * self.task.len();
        if total_uniforms_size > self.uniforms_buffer.size() as usize {
            self.uniforms_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("uniforms buffer"),
                size: total_uniforms_size as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });
        }

        let bc6_setting_count = self
            .task
            .iter()
            .filter(|task| task.variant == CompressionVariantLDR::BC6H)
            .count();

        let total_bc6h_size = self.bc6h_aligned_size * bc6_setting_count;
        if total_bc6h_size > self.bc6h_settings_buffer.size() as usize {
            self.bc6h_settings_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("bc6h settings buffer"),
                size: total_bc6h_size as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
        }

        let bc7_setting_count = self
            .task
            .iter()
            .filter(|task| task.variant == CompressionVariantLDR::BC7)
            .count();

        let total_bc7_size = self.bc7_aligned_size * bc7_setting_count;
        if total_bc7_size > self.bc7_settings_buffer.size() as usize {
            self.bc7_settings_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("bc7 settings buffer"),
                size: total_bc7_size as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
        }
    }

    fn upload(&mut self) {
        self.scratch_buffer.clear();
        for (index, task) in self.task.iter_mut().enumerate() {
            let offset = index * self.uniforms_aligned_size;
            task.uniform_offset = offset as u32;

            let uniforms = Uniforms {
                width: task.width,
                height: task.height,
                blocks_offset: task.buffer_offset / 4,
            };

            self.scratch_buffer
                .resize(offset + self.uniforms_aligned_size, 0);
            self.scratch_buffer[offset..offset + size_of::<Uniforms>()]
                .copy_from_slice(cast_slice(&[uniforms]));
        }
        if !self.scratch_buffer.is_empty() {
            if let Some(mut data) = self.queue.write_buffer_with(
                &self.uniforms_buffer,
                0,
                NonZeroU64::new(self.scratch_buffer.len() as u64).unwrap(),
            ) {
                data.copy_from_slice(&self.scratch_buffer);
            }
        }

        self.scratch_buffer.clear();
        for (index, task) in self
            .task
            .iter_mut()
            .filter(|task| task.variant == CompressionVariantLDR::BC6H)
            .enumerate()
        {
            let offset = index * self.bc6h_aligned_size;
            task.setting_offset = offset as u32;

            match &task.settings {
                Some(Settings::BC6H(settings)) => {
                    self.scratch_buffer
                        .resize(offset + self.bc6h_aligned_size, 0);
                    self.scratch_buffer[offset..offset + size_of::<BC6HSettings>()]
                        .copy_from_slice(cast_slice(&[*settings]));
                }
                _ => panic!("internal error: No BC6H setting found"),
            }
        }
        if !self.scratch_buffer.is_empty() {
            if let Some(mut data) = self.queue.write_buffer_with(
                &self.bc6h_settings_buffer,
                0,
                NonZeroU64::new(self.scratch_buffer.len() as u64).unwrap(),
            ) {
                data.copy_from_slice(&self.scratch_buffer);
            }
        }

        self.scratch_buffer.clear();
        for (index, task) in self
            .task
            .iter_mut()
            .filter(|task| task.variant == CompressionVariantLDR::BC7)
            .enumerate()
        {
            let offset = index * self.bc7_aligned_size;
            task.setting_offset = offset as u32;

            match &task.settings {
                Some(Settings::BC7(settings)) => {
                    self.scratch_buffer
                        .resize(offset + self.bc7_aligned_size, 0);
                    self.scratch_buffer[offset..offset + size_of::<BC7Settings>()]
                        .copy_from_slice(cast_slice(&[*settings]));
                }
                _ => panic!("internal error: No BC7 setting found"),
            }
        }
        if !self.scratch_buffer.is_empty() {
            if let Some(mut data) = self.queue.write_buffer_with(
                &self.bc7_settings_buffer,
                0,
                NonZeroU64::new(self.scratch_buffer.len() as u64).unwrap(),
            ) {
                data.copy_from_slice(&self.scratch_buffer);
            }
        }
    }

    /// Will upload all dispatch data and then dispatches all compression tasks to the GPU.
    ///
    /// # Arguments
    /// * `pass` - The compute pass to record commands into
    pub fn compress(&mut self, pass: &mut ComputePass) {
        self.update_buffer_sizes();
        self.upload();

        for task in self.task.drain(..) {
            let pipeline = self
                .pipelines
                .get(&task.variant)
                .expect("can't find pipeline for variant");

            pass.set_pipeline(pipeline);

            match task.variant {
                CompressionVariantLDR::BC6H | CompressionVariantLDR::BC7 => {
                    pass.set_bind_group(
                        0,
                        &task.bind_group,
                        &[task.uniform_offset, task.setting_offset],
                    );
                }
                _ => {
                    pass.set_bind_group(0, &task.bind_group, &[task.uniform_offset]);
                }
            }

            let block_width = (task.width + 3) / 4;
            let block_height = (task.height + 3) / 4;

            let workgroup_width = (block_width + 7) / 8;
            let workgroup_height = (block_height + 7) / 8;

            pass.dispatch_workgroups(workgroup_width, workgroup_height, 1);
        }
    }
}
