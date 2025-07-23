use block_compression::{
    decode::decompress_blocks_as_rgba8, encode::compress_rgba8, BC6HSettings, BC7Settings,
    CompressionVariant, GpuBlockCompressor,
};
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor, TextureViewDescriptor};

use self::common::{
    create_blocks_buffer, create_wgpu_resources, download_blocks_data,
    read_image_and_create_texture, srgb_to_linear, BRICK_FILE_PATH, MARBLE_FILE_PATH,
};

mod common;

pub const BRICK_ALPHA_FILE_PATH: &str = "tests/images/brick-alpha.png";
pub const MARBLE_ALPHA_FILE_PATH: &str = "tests/images/marble-alpha.png";
pub const BLENDER_FILE_PATH: &str = "tests/images/blender.png";

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

fn print_metrics(name: &str, metrics: &PsnrResult) {
    println!("-----------------------");
    println!("Image name: {name}");
    println!("Overall PSNR: {:.4} dB", metrics.overall_psnr);
    println!("Overall MSE: {:.9}", metrics.overall_mse);
    println!(
        "Red channel PSNR: {:.4} dB",
        metrics.channel_results.red.psnr
    );
    println!(
        "Green channel PSNR: {:.4} dB",
        metrics.channel_results.green.psnr
    );
    println!(
        "Blue channel PSNR: {:.4} dB",
        metrics.channel_results.blue.psnr
    );
    println!(
        "Alpha channel PSNR: {:.4} dB",
        metrics.channel_results.alpha.psnr
    );
    println!("-----------------------");
}

fn compress_image_reference(
    variant: CompressionVariant,
    width: u32,
    height: u32,
    data: &[u8],
) -> Vec<u8> {
    let output_size = variant.blocks_byte_size(width, height);
    let mut blocks = vec![0; output_size];
    compress_rgba8(variant, data, &mut blocks, width, height, width * 4);
    blocks
}

fn compress_image(image_path: &str, variant: CompressionVariant) -> (u32, u32, Vec<u8>, Vec<u8>) {
    let (device, queue) = create_wgpu_resources();
    let mut block_compressor = GpuBlockCompressor::new(device.clone(), queue.clone());

    let (texture, original_data) =
        read_image_and_create_texture(&device, &queue, image_path, variant);
    let blocks_size = variant.blocks_byte_size(texture.width(), texture.height());

    let blocks = create_blocks_buffer(&device, blocks_size as u64);

    block_compressor.add_compression_task(
        variant,
        &texture.create_view(&TextureViewDescriptor::default()),
        texture.width(),
        texture.height(),
        &blocks,
        None,
        None,
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

fn calculate_psnr(
    variant: CompressionVariant,
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

fn compare_psnr(image_path: &str, variant: CompressionVariant, channels: u32) {
    let image_name = std::path::Path::new(image_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let (width, height, original_data, blocks_data) = compress_image(image_path, variant);

    let psnr = calculate_psnr(
        variant,
        channels,
        width,
        height,
        &original_data,
        &blocks_data,
    );

    let reference_block_data = compress_image_reference(variant, width, height, &original_data);

    let reference_psnr = calculate_psnr(
        variant,
        channels,
        width,
        height,
        &original_data,
        &reference_block_data,
    );

    print_metrics(image_name, &psnr);
    print_metrics(image_name, &reference_psnr);

    const DIFFERENCE: f64 = 0.0025;

    if f64::abs(reference_psnr.overall_psnr - psnr.overall_psnr) > DIFFERENCE {
        panic!(
            "Significant overall PSNR difference for image `{image_name}`: {:.3} != {:.3}",
            reference_psnr.overall_psnr, psnr.overall_psnr
        );
    }
}

#[test]
fn psnr_bc1() {
    compare_psnr(BRICK_FILE_PATH, CompressionVariant::BC1, 3);
    compare_psnr(MARBLE_FILE_PATH, CompressionVariant::BC1, 3);
    compare_psnr(BLENDER_FILE_PATH, CompressionVariant::BC1, 3);
}

#[test]
fn psnr_bc3() {
    compare_psnr(BRICK_ALPHA_FILE_PATH, CompressionVariant::BC3, 4);
    compare_psnr(MARBLE_ALPHA_FILE_PATH, CompressionVariant::BC3, 4);
}

#[test]
fn psnr_bc6h_very_fast() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_fast()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_fast()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_fast()),
        3,
    );
}

#[test]
fn psnr_bc6h_fast() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::fast()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::fast()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::fast()),
        3,
    );
}

#[test]
fn psnr_bc6h_basic() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::basic()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::basic()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::basic()),
        3,
    );
}

#[test]
fn psnr_bc6h_slow() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::slow()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::slow()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::slow()),
        3,
    );
}

#[test]
fn psnr_bc6h_very_slow() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_slow()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_slow()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC6H(BC6HSettings::very_slow()),
        3,
    );
}

#[test]
fn psnr_bc7_alpha_ultra_fast() {
    compare_psnr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_ultrafast()),
        4,
    );
    compare_psnr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_ultrafast()),
        4,
    );
}

#[test]
fn psnr_bc7_alpha_very_fast() {
    compare_psnr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_very_fast()),
        4,
    );
    compare_psnr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_very_fast()),
        4,
    );
}

#[test]
fn psnr_bc7_alpha_fast() {
    compare_psnr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_fast()),
        4,
    );
    compare_psnr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_fast()),
        4,
    );
}

#[test]
fn psnr_bc7_alpha_basic() {
    compare_psnr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_basic()),
        4,
    );
    compare_psnr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_basic()),
        4,
    );
}

#[test]
fn psnr_bc7_alpha_slow() {
    compare_psnr(
        BRICK_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_slow()),
        4,
    );
    compare_psnr(
        MARBLE_ALPHA_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::alpha_slow()),
        4,
    );
}

#[test]
fn psnr_bc7_opaque_ultra_fast() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_ultra_fast()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_ultra_fast()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_ultra_fast()),
        3,
    );
}

#[test]
fn psnr_bc7_opaque_very_fast() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_very_fast()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_very_fast()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_very_fast()),
        3,
    );
}

#[test]
fn psnr_bc7_opaque_fast() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_fast()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_fast()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_fast()),
        3,
    );
}

#[test]
fn psnr_bc7_opaque_basic() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_basic()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_basic()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_basic()),
        3,
    );
}

#[test]
fn psnr_bc7_opaque_slow() {
    compare_psnr(
        BRICK_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_slow()),
        3,
    );
    compare_psnr(
        MARBLE_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_slow()),
        3,
    );
    compare_psnr(
        BLENDER_FILE_PATH,
        CompressionVariant::BC7(BC7Settings::opaque_slow()),
        3,
    );
}
