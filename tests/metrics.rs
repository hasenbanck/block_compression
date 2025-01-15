use block_compression::{
    decode::decompress_blocks_as_rgba8, BC6HSettings, BC7Settings, BlockCompressor,
    CompressionVariantLDR, Settings,
};
use bytemuck::cast_slice;
use half::f16;
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use intel_tex_2::{bc1, bc3, bc6h, bc7, RgbaSurface};
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, TextureFormat::Rgba8Unorm,
    TextureViewDescriptor,
};

use self::common::{
    create_blocks_buffer, create_wgpu_resources, download_blocks_data,
    read_image_and_create_texture, BRICK_FILE_PATH, MARBLE_FILE_PATH,
};

mod common;

pub const BRICK_ALPHA_FILE_PATH: &str = "tests/images/brick-alpha.png";
pub const MARBLE_ALPHA_FILE_PATH: &str = "tests/images/marble-alpha.png";

#[derive(Debug, Clone)]
pub struct PsnrResult {
    pub overall_psnr: f64,
    pub overall_mse: f64,
    pub channel_results: ChannelResults,
}

#[derive(Debug, Clone)]
pub struct ChannelResults {
    pub red: ChannelMetrics,
    pub green: ChannelMetrics,
    pub blue: ChannelMetrics,
    pub alpha: ChannelMetrics,
}

#[derive(Debug, Clone)]
pub struct ChannelMetrics {
    pub psnr: f64,
    pub mse: f64,
}

/// Calculates quality metrics for a given image. The input data and output data must be RGBA data.
pub fn calculate_image_metrics_rgba8(
    original: &[u8],
    compressed: &[u8],
    width: u32,
    height: u32,
    channels: u32,
) -> PsnrResult {
    if original.len() != compressed.len() {
        panic!("Image buffers must have same length");
    }
    if original.len() != (width * height * 4) as usize {
        panic!("Buffer size doesn't match dimensions");
    }

    let mut channel_mse = [0.0; 4];
    let pixel_count = (width * height) as f64;

    for index in (0..original.len()).step_by(4) {
        for channel in 0..channels as usize {
            let orig = if channel < 3 {
                srgb_to_linear(original[index + channel])
            } else {
                (original[index + channel] as f64) / 255.0
            };

            let comp = if channel < 3 {
                srgb_to_linear(compressed[index + channel])
            } else {
                (compressed[index + channel] as f64) / 255.0
            };

            let diff = orig - comp;
            channel_mse[channel] += diff * diff;
        }
    }

    // Normalize MSE values
    channel_mse.iter_mut().for_each(|mse| *mse /= pixel_count);

    let calculate_psnr = |mse: f64| -> f64 {
        if mse == 0.0 {
            0.0
        } else {
            20.0 * (1.0 / mse.sqrt()).log10()
        }
    };

    let overall_mse = channel_mse.iter().sum::<f64>() / channels as f64;
    let overall_psnr = calculate_psnr(overall_mse);

    let channel_results = ChannelResults {
        red: ChannelMetrics {
            mse: channel_mse[0],
            psnr: calculate_psnr(channel_mse[0]),
        },
        green: ChannelMetrics {
            mse: channel_mse[1],
            psnr: calculate_psnr(channel_mse[1]),
        },
        blue: ChannelMetrics {
            mse: channel_mse[2],
            psnr: calculate_psnr(channel_mse[2]),
        },
        alpha: ChannelMetrics {
            mse: channel_mse[3],
            psnr: calculate_psnr(channel_mse[3]),
        },
    };

    PsnrResult {
        overall_psnr,
        overall_mse,
        channel_results,
    }
}

#[inline]
fn srgb_to_linear(srgb: u8) -> f64 {
    let v = (srgb as f64) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn print_metrics(name: &str, metrics: &PsnrResult) {
    println!("-----------------------");
    println!("Image name: {}", name);
    println!("Overall PSNR: {:.2} dB", metrics.overall_psnr);
    println!("Overall MSE: {:.6}", metrics.overall_mse);
    println!(
        "Red channel PSNR: {:.2} dB",
        metrics.channel_results.red.psnr
    );
    println!(
        "Green channel PSNR: {:.2} dB",
        metrics.channel_results.green.psnr
    );
    println!(
        "Blue channel PSNR: {:.2} dB",
        metrics.channel_results.blue.psnr
    );
    println!(
        "Alpha channel PSNR: {:.2} dB",
        metrics.channel_results.alpha.psnr
    );
    println!("-----------------------");
}

fn save_png(filename: &str, data: &[u8], width: u32, height: u32) {
    let file = std::fs::File::create(filename).unwrap();
    let encoder = PngEncoder::new(file);
    encoder
        .write_image(data, width, height, ExtendedColorType::Rgba8)
        .unwrap();
}

fn compress_image_reference_ldr(
    variant: CompressionVariantLDR,
    settings: Option<Settings>,
    width: u32,
    height: u32,
    data: &[u8],
) -> Vec<u8> {
    match (variant, settings) {
        (CompressionVariantLDR::BC1, None) => bc1::compress_blocks(&RgbaSurface {
            data,
            width,
            height,
            stride: width * 4,
        }),
        (CompressionVariantLDR::BC3, None) => bc3::compress_blocks(&RgbaSurface {
            data,
            width,
            height,
            stride: width * 4,
        }),
        (CompressionVariantLDR::BC6H, Some(Settings::BC6H(setting))) => {
            let settings = if setting == BC6HSettings::very_fast() {
                bc6h::very_fast_settings()
            } else if setting == BC6HSettings::fast() {
                bc6h::very_settings()
            } else if setting == BC6HSettings::basic() {
                bc6h::basic_settings()
            } else if setting == BC6HSettings::slow() {
                bc6h::slow_settings()
            } else if setting == BC6HSettings::very_slow() {
                bc6h::very_slow_settings()
            } else {
                panic!("Unsupported BC6H setting");
            };

            let rgba_f16_data: Vec<u8> = data
                .iter()
                .flat_map(|color| f16::from_f64(srgb_to_linear(*color)).to_le_bytes())
                .collect();

            bc6h::compress_blocks(
                &settings,
                &RgbaSurface {
                    data: &rgba_f16_data,
                    width,
                    height,
                    stride: width * 4 * size_of::<f16>() as u32,
                },
            )
        }
        (CompressionVariantLDR::BC7, Some(Settings::BC7(setting))) => {
            let settings = if setting == BC7Settings::alpha_ultrafast() {
                bc7::alpha_ultra_fast_settings()
            } else if setting == BC7Settings::alpha_very_fast() {
                bc7::alpha_very_fast_settings()
            } else if setting == BC7Settings::alpha_fast() {
                bc7::alpha_fast_settings()
            } else if setting == BC7Settings::alpha_basic() {
                bc7::alpha_basic_settings()
            } else if setting == BC7Settings::alpha_slow() {
                bc7::alpha_slow_settings()
            } else if setting == BC7Settings::opaque_ultra_fast() {
                bc7::opaque_ultra_fast_settings()
            } else if setting == BC7Settings::opaque_very_fast() {
                bc7::opaque_very_fast_settings()
            } else if setting == BC7Settings::opaque_fast() {
                bc7::opaque_fast_settings()
            } else if setting == BC7Settings::opaque_basic() {
                bc7::opaque_basic_settings()
            } else if setting == BC7Settings::opaque_slow() {
                bc7::opaque_slow_settings()
            } else {
                panic!("Unsupported BC7 setting");
            };

            bc7::compress_blocks(
                &settings,
                &RgbaSurface {
                    data,
                    width,
                    height,
                    stride: width * 4,
                },
            )
        }
        (_, _) => {
            panic!("Unsupported variant or setting")
        }
    }
}

fn compress_image_ldr(
    image_path: &str,
    variant: CompressionVariantLDR,
    settings: Option<Settings>,
) -> (u32, u32, Vec<u8>, Vec<u8>) {
    let (device, queue) = create_wgpu_resources();
    let mut block_compressor = BlockCompressor::new(device.clone(), queue.clone());

    let (texture, original_data) = read_image_and_create_texture(&device, &queue, image_path);
    let blocks_size = variant.blocks_byte_size(texture.width(), texture.height());

    let blocks = create_blocks_buffer(&device, blocks_size as u64);

    block_compressor.add_compression_task_ldr(
        variant,
        &texture.create_view(&TextureViewDescriptor {
            format: Some(Rgba8Unorm),
            ..Default::default()
        }),
        texture.width(),
        texture.height(),
        &blocks,
        None,
        settings,
    );

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("command encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("compute pass"),
            timestamp_writes: None,
        });

        block_compressor.compress(&mut pass);
    }

    queue.submit([encoder.finish()]);

    let blocks_data = download_blocks_data(&device, &queue, blocks);

    (
        texture.width(),
        texture.height(),
        original_data,
        blocks_data,
    )
}

fn calculate_psnr_ldr(
    variant: CompressionVariantLDR,
    channels: u32,
    width: u32,
    height: u32,
    original_data: &[u8],
    blocks_data: &[u8],
) -> PsnrResult {
    let size = width * height * 4;

    let mut decompressed_data = vec![0; size as usize];
    decompress_blocks_as_rgba8(variant, width, height, blocks_data, &mut decompressed_data);

    calculate_image_metrics_rgba8(original_data, &decompressed_data, width, height, channels)
}

fn compare_psnr_ldr(
    image_path: &str,
    variant: CompressionVariantLDR,
    channels: u32,
    settings: Option<Settings>,
) {
    let image_name = std::path::Path::new(image_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let (width, height, original_data, blocks_data) =
        compress_image_ldr(image_path, variant, settings);

    let psnr = calculate_psnr_ldr(
        variant,
        channels,
        width,
        height,
        &original_data,
        &blocks_data,
    );

    let reference_block_data =
        compress_image_reference_ldr(variant, settings, width, height, &original_data);

    let reference_psnr = calculate_psnr_ldr(
        variant,
        channels,
        width,
        height,
        &original_data,
        &reference_block_data,
    );

    print_metrics(image_name, &psnr);
    print_metrics(image_name, &reference_psnr);

    const DIFFERENCE: f64 = 0.01;

    if reference_psnr.overall_psnr - psnr.overall_psnr > DIFFERENCE {
        panic!(
            "Significant overall PSNR difference for image `{image_name}`: {:.3} > {:.3}",
            reference_psnr.overall_psnr, psnr.overall_psnr
        );
    }
}

#[test]
fn psnr_bc1() {
    compare_psnr_ldr(BRICK_FILE_PATH, CompressionVariantLDR::BC1, 3, None);
    compare_psnr_ldr(MARBLE_FILE_PATH, CompressionVariantLDR::BC1, 3, None);
}

#[test]
fn psnr_bc3() {
    compare_psnr_ldr(BRICK_ALPHA_FILE_PATH, CompressionVariantLDR::BC3, 4, None);
    compare_psnr_ldr(MARBLE_ALPHA_FILE_PATH, CompressionVariantLDR::BC3, 4, None);
}

#[test]
fn psnr_bc6h_very_fast_ldr() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::very_fast())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::very_fast())),
    );
}

#[test]
fn psnr_bc6h_fast_ldr() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::fast())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::fast())),
    );
}

#[test]
fn psnr_bc6h_basic_ldr() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::basic())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::basic())),
    );
}

#[test]
fn psnr_bc6h_slow_ldr() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::slow())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::slow())),
    );
}

#[test]
fn psnr_bc6h_very_slow_ldr() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::very_slow())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC6H,
        3,
        Some(Settings::BC6H(BC6HSettings::very_slow())),
    );
}

#[test]
fn psnr_bc7_alpha_ultra_fast() {
    compare_psnr_ldr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_ultrafast())),
    );
    compare_psnr_ldr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_ultrafast())),
    );
}

#[test]
fn psnr_bc7_alpha_very_fast() {
    compare_psnr_ldr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_very_fast())),
    );
    compare_psnr_ldr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_very_fast())),
    );
}

#[test]
fn psnr_bc7_alpha_fast() {
    compare_psnr_ldr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_fast())),
    );
    compare_psnr_ldr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_fast())),
    );
}

#[test]
fn psnr_bc7_alpha_basic() {
    compare_psnr_ldr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_basic())),
    );
    compare_psnr_ldr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_basic())),
    );
}

#[test]
fn psnr_bc7_alpha_slow() {
    compare_psnr_ldr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_slow())),
    );
    compare_psnr_ldr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariantLDR::BC7,
        4,
        Some(Settings::BC7(BC7Settings::alpha_slow())),
    );
}

#[test]
fn psnr_bc7_opaque_ultra_fast() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_ultra_fast())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_ultra_fast())),
    );
}

#[test]
fn psnr_bc7_opaque_very_fast() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_very_fast())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_very_fast())),
    );
}

#[test]
fn psnr_bc7_opaque_fast() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_fast())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_fast())),
    );
}

#[test]
fn psnr_bc7_opaque_basic() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_basic())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_basic())),
    );
}

#[test]
fn psnr_bc7_opaque_slow() {
    compare_psnr_ldr(
        BRICK_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_slow())),
    );
    compare_psnr_ldr(
        MARBLE_FILE_PATH,
        CompressionVariantLDR::BC7,
        3,
        Some(Settings::BC7(BC7Settings::opaque_slow())),
    );
}
