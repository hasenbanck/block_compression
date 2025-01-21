//! CPU based encoding.

mod bc1_to_5;
mod bc7;

use crate::{
    encode::{bc1_to_5::BlockCompressorBC15, bc7::BlockCompressorBC7},
    BC7Settings, CompressionVariant,
};

/// Compresses raw RGBA8 data into BC1-7 block compressed format.
///
/// This function provides CPU-based texture compression for RGBA8 data. It supports BC1 through BC7
/// compression formats, with only BC6H being currently unimplemented.
///
/// # Data Layout Requirements
/// The input data must be in RGBA8 format (8 bits per channel, 32 bits per pixel). The data is
/// expected to be in row-major order, with optional stride for padding between rows.
///
/// # Buffer Requirements
/// The destination buffer must have sufficient capacity to store the compressed blocks.
/// The required size can be calculated using [`CompressionVariant::blocks_byte_size()`].
///
/// For example:
/// ```ignore
/// let required_size = variant.blocks_byte_size(width, height);
/// assert!(blocks_buffer.len() &gt;= required_size);
/// ```
///
/// # Arguments
/// * `variation` - The block compression format to use
/// * `rgba_data` - Source RGBA8 pixel data
/// * `blocks_buffer` - Destination buffer for the compressed blocks
/// * `width` - Width of the image in pixels
/// * `height` - Height of the image in pixels
/// * `stride` - Number of bytes per row in the source data (for padding).
///              Must be `width * 4` for tightly packed RGBA data.
///
/// # Panics
/// * If `width` or `height` is not a multiple of 4
/// * If the destination `blocks_buffer` is too small to hold the compressed data
/// * If BC6H compression is requested (currently unimplemented)
///
/// # Example
/// ```
/// use block_compression::{encode::compress_rgba8, CompressionVariant};
///
/// let rgba_data = vec![0u8; 256 * 256 * 4]; // Your RGBA data
/// let width = 256;
/// let height = 256;
/// let stride = width * 4; // Tightly packed rows
/// let variant = CompressionVariant::BC1;
///
/// let mut blocks_buffer = vec![0u8; variant.blocks_byte_size(width, height)];
///
/// compress_rgba8(
///     variant,
///     &rgba_data,
///     &mut blocks_buffer,
///     width,
///     height,
///     stride,
/// );
/// ```
pub fn compress_rgba8(
    variation: CompressionVariant,
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    width: u32,
    height: u32,
    stride: u32,
) {
    assert_eq!(height % 4, 0);
    assert_eq!(width % 4, 0);

    let required_size = variation.blocks_byte_size(width, height);

    assert!(
        blocks_buffer.len() >= required_size,
        "blocks_buffer size ({}) is too small to hold compressed blocks. Required size: {}",
        blocks_buffer.len(),
        required_size
    );

    let stride = stride as usize;
    let block_width = (width as usize + 3) / 4;
    let block_height = (height as usize + 3) / 4;

    match variation {
        CompressionVariant::BC1 => {
            compress_bc1(rgba_data, blocks_buffer, block_width, block_height, stride);
        }
        CompressionVariant::BC2 => {
            compress_bc2(rgba_data, blocks_buffer, block_width, block_height, stride);
        }
        CompressionVariant::BC3 => {
            compress_bc3(rgba_data, blocks_buffer, block_width, block_height, stride);
        }
        CompressionVariant::BC4 => {
            compress_bc4(rgba_data, blocks_buffer, block_width, block_height, stride);
        }
        CompressionVariant::BC5 => {
            compress_bc5(rgba_data, blocks_buffer, block_width, block_height, stride);
        }
        #[cfg(feature = "bc6h")]
        CompressionVariant::BC6H(_) => {
            unimplemented!("CPU based BC6H compression not yet implemented yet");
        }
        #[cfg(feature = "bc7")]
        CompressionVariant::BC7(settings) => {
            compress_bc7(
                rgba_data,
                blocks_buffer,
                block_width,
                block_height,
                stride,
                &settings,
            );
        }
    }
}

fn compress_bc1(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC15::default();

            block_compressor.load_block_interleaved_rgba(rgba_data, xx, yy, stride);
            let color_result = block_compressor.compress_block_bc1_core();
            block_compressor.store_data(blocks_buffer, block_width, xx, yy, &color_result);
        }
    }
}

fn compress_bc2(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC15::default();
            let mut compressed_data = [0; 4];

            let alpha_result = block_compressor.load_block_alpha_4bit(rgba_data, xx, yy, stride);

            compressed_data[0] = alpha_result[0];
            compressed_data[1] = alpha_result[1];

            block_compressor.load_block_interleaved_rgba(rgba_data, xx, yy, stride);

            let color_result = block_compressor.compress_block_bc1_core();
            compressed_data[2] = color_result[0];
            compressed_data[3] = color_result[1];

            block_compressor.store_data(blocks_buffer, block_width, xx, yy, &compressed_data);
        }
    }
}

fn compress_bc3(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC15::default();

            let mut compressed_data = [0; 4];

            block_compressor.load_block_interleaved_rgba(rgba_data, xx, yy, stride);

            let alpha_result = block_compressor.compress_block_bc3_alpha();
            compressed_data[0] = alpha_result[0];
            compressed_data[1] = alpha_result[1];

            let color_result = block_compressor.compress_block_bc1_core();
            compressed_data[2] = color_result[0];
            compressed_data[3] = color_result[1];

            block_compressor.store_data(blocks_buffer, block_width, xx, yy, &compressed_data);
        }
    }
}

fn compress_bc4(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC15::default();

            let mut compressed_data = [0; 2];

            block_compressor.load_block_r_8bit(rgba_data, xx, yy, stride);

            let color_result = block_compressor.compress_block_bc3_alpha();
            compressed_data[0] = color_result[0];
            compressed_data[1] = color_result[1];

            block_compressor.store_data(blocks_buffer, block_width, xx, yy, &compressed_data);
        }
    }
}

fn compress_bc5(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC15::default();

            let mut compressed_data = [0; 4];

            block_compressor.load_block_r_8bit(rgba_data, xx, yy, stride);

            let red_result = block_compressor.compress_block_bc3_alpha();
            compressed_data[0] = red_result[0];
            compressed_data[1] = red_result[1];

            block_compressor.load_block_g_8bit(rgba_data, xx, yy, stride);

            let green_result = block_compressor.compress_block_bc3_alpha();
            compressed_data[2] = green_result[0];
            compressed_data[3] = green_result[1];

            block_compressor.store_data(blocks_buffer, block_width, xx, yy, &compressed_data);
        }
    }
}

fn compress_bc7(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
    settings: &BC7Settings,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC7::new(settings);

            block_compressor.load_block_interleaved_rgba(rgba_data, xx, yy, stride);
            block_compressor.compute_opaque_err();
            block_compressor.compress_block_bc7_core();
            block_compressor.store_data(blocks_buffer, block_width, xx, yy);
        }
    }
}
