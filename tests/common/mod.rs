use std::sync::{Arc, LazyLock};

use block_compression::CompressionVariant;
use half::f16;
use image::ImageReader;
use pollster::block_on;
use wgpu::{
    util::{DeviceExt, TextureDataOrder},
    BackendOptions, Backends, Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
    Device, DeviceDescriptor, Dx12BackendOptions, Dx12Compiler, Error, Extent3d, Features,
    Instance, InstanceDescriptor, InstanceFlags, Limits, MapMode, MemoryHints, PollType,
    PowerPreference, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, Trace,
};

#[inline]
pub fn srgb_to_linear(srgb: u8) -> f64 {
    let v = (srgb as f64) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub const BRICK_FILE_PATH: &str = "tests/images/brick.png";
pub const MARBLE_FILE_PATH: &str = "tests/images/marble.png";

pub fn create_wgpu_resources() -> (Arc<Device>, Arc<Queue>) {
    static CACHE: LazyLock<(Arc<Device>, Arc<Queue>)> = LazyLock::new(|| {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::from_env().unwrap_or_default(),
            flags: InstanceFlags::from_build_config().with_env(),
            backend_options: BackendOptions {
                dx12: Dx12BackendOptions {
                    shader_compiler: Dx12Compiler::StaticDxc,
                }
                .with_env(),
                ..Default::default()
            },
        });

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("main device"),
            required_features: Features::default(),
            required_limits: Limits::default(),
            memory_hints: MemoryHints::Performance,
            trace: Trace::Off,
        }))
        .expect("Failed to create device");
        device.on_uncaptured_error(Box::new(error_handler));

        (Arc::new(device), Arc::new(queue))
    });

    CACHE.clone()
}

pub fn error_handler(error: Error) {
    let (message_type, message) = match error {
        Error::OutOfMemory { source } => ("OutOfMemory", source.to_string()),
        Error::Validation {
            source,
            description,
        } => ("Validation", format!("{source}: {description}")),
        Error::Internal {
            source,
            description,
        } => ("Internal", format!("{source}: {description}")),
    };

    panic!("wgpu [{message_type}] [error]: {message}");
}

pub fn read_image_and_create_texture(
    device: &Device,
    queue: &Queue,
    file_path: &str,
    variant: CompressionVariant,
) -> (Texture, Vec<u8>) {
    let image = ImageReader::open(file_path)
        .expect("can't open input image")
        .decode()
        .expect("can't decode image");

    let rgba_image = image.to_rgba8();
    let width = rgba_image.width();
    let height = rgba_image.height();

    let texture = if matches!(variant, CompressionVariant::BC6H(..)) {
        let rgba_f16_data: Vec<u8> = rgba_image
            .iter()
            .flat_map(|color| f16::from_f64(srgb_to_linear(*color)).to_ne_bytes())
            .collect();

        device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some(file_path),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            rgba_f16_data.as_slice(),
        )
    } else {
        device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some(file_path),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            &rgba_image,
        )
    };

    (texture, rgba_image.to_vec())
}

pub fn create_blocks_buffer(device: &Device, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("blocks buffer"),
        size,
        usage: BufferUsages::COPY_SRC | BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

pub fn download_blocks_data(device: &Device, queue: &Queue, block_buffer: Buffer) -> Vec<u8> {
    let size = block_buffer.size();

    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("staging buffer"),
        size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut copy_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("copy encoder"),
    });

    copy_encoder.copy_buffer_to_buffer(&block_buffer, 0, &staging_buffer, 0, size);

    queue.submit([copy_encoder.finish()]);

    let result;

    {
        let buffer_slice = staging_buffer.slice(..);

        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |v| tx.send(v).unwrap());

        let _ = device.poll(PollType::Wait);

        match rx.recv() {
            Ok(Ok(())) => {
                result = buffer_slice.get_mapped_range().to_vec();
            }
            _ => panic!("couldn't read from buffer"),
        }
    }

    staging_buffer.unmap();

    result
}
