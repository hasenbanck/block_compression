//! CPU based decoding.

#[cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7"))]
mod block;

#[cfg(feature = "bc7")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc7")))]
pub use self::block::decode_block_bc7;
#[cfg(feature = "bc15")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc15")))]
pub use self::block::{
    decode_block_bc1, decode_block_bc2, decode_block_bc3, decode_block_bc4, decode_block_bc5,
};
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
pub use self::block::{decode_block_bc6h, decode_block_bc6h_float};
#[cfg(feature = "bc6h")]
use crate::BC6HSettings;
#[cfg(feature = "bc7")]
use crate::BC7Settings;
#[cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7"))]
use crate::CompressionVariant;

/// Trait to decode a BC variant into RGBA8 data.
#[cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7"))]
trait BlockRgba8Decoder {
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize);
    fn block_byte_size() -> u32;
}

/// Trait to decode a BC variant into RGBA16F data.
#[cfg(feature = "bc6h")]
trait BlockRgba16fDecoder {
    fn decode_block_rgba16f(compressed: &[u8], decompressed: &mut [half::f16], pitch: usize);
    fn block_byte_size() -> u32;
}

/// Trait to decode a BC variant into RGBA32F data.
#[cfg(feature = "bc6h")]
trait BlockRgba32fDecoder {
    fn decode_block_rgba32f(compressed: &[u8], decompressed: &mut [f32], pitch: usize);
    fn block_byte_size() -> u32;
}

#[cfg(feature = "bc15")]
struct BC1Decoder;
#[cfg(feature = "bc15")]
struct BC2Decoder;
#[cfg(feature = "bc15")]
struct BC3Decoder;
#[cfg(feature = "bc15")]
struct BC4Decoder;
#[cfg(feature = "bc15")]
struct BC5Decoder;
#[cfg(feature = "bc6h")]
struct BC6HDecoder;
#[cfg(feature = "bc7")]
struct BC7Decoder;

#[cfg(feature = "bc15")]
impl BlockRgba8Decoder for BC1Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        decode_block_bc1(compressed, decompressed, pitch)
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC1.block_byte_size()
    }
}

#[cfg(feature = "bc15")]
impl BlockRgba8Decoder for BC2Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        decode_block_bc2(compressed, decompressed, pitch)
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC2.block_byte_size()
    }
}

#[cfg(feature = "bc15")]
impl BlockRgba8Decoder for BC3Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        decode_block_bc3(compressed, decompressed, pitch)
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC3.block_byte_size()
    }
}

#[cfg(feature = "bc15")]
impl BlockRgba8Decoder for BC4Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        const PITCH: usize = 4;
        let mut buffer = [0u8; 16];
        decode_block_bc4(compressed, &mut buffer, 4);

        // Convert R8 to RGBA8
        for y in 0..4 {
            for x in 0..4 {
                let out_pos = y * pitch + x * 4;
                let in_pos = y * PITCH + x;

                decompressed[out_pos] = buffer[in_pos];
                decompressed[out_pos + 1] = 0;
                decompressed[out_pos + 2] = 0;
                decompressed[out_pos + 3] = 0;
            }
        }
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC4.block_byte_size()
    }
}

#[cfg(feature = "bc15")]
impl BlockRgba8Decoder for BC5Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        const PITCH: usize = 8;
        let mut buffer = [0u8; 32];
        decode_block_bc5(compressed, &mut buffer, PITCH);

        // Convert RG8 to RGBA8
        for y in 0..4 {
            for x in 0..4 {
                let out_pos = y * pitch + x * 4;
                let in_pos = y * PITCH + x * 2;

                decompressed[out_pos] = buffer[in_pos];
                decompressed[out_pos + 1] = buffer[in_pos + 1];
                decompressed[out_pos + 2] = 0;
                decompressed[out_pos + 3] = 0;
            }
        }
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC5.block_byte_size()
    }
}

#[cfg(feature = "bc6h")]
fn linear_to_srgb(linear: f32) -> u8 {
    let v = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };

    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(feature = "bc6h")]
impl BlockRgba8Decoder for BC6HDecoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        const PITCH: usize = 12;
        let mut buffer = [0.0_f32; 48];
        decode_block_bc6h_float(compressed, &mut buffer, PITCH, false);

        // Convert RGB16F to RGBA8
        for y in 0..4 {
            for x in 0..4 {
                let out_pos = y * pitch + x * 4;
                let in_pos = y * PITCH + x * 3;

                decompressed[out_pos] = linear_to_srgb(buffer[in_pos]) as _;
                decompressed[out_pos + 1] = linear_to_srgb(buffer[in_pos + 1]) as _;
                decompressed[out_pos + 2] = linear_to_srgb(buffer[in_pos + 2]) as _;
                decompressed[out_pos + 3] = 0;
            }
        }
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC6H(BC6HSettings::basic()).block_byte_size()
    }
}

#[cfg(feature = "bc7")]
impl BlockRgba8Decoder for BC7Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        decode_block_bc7(compressed, decompressed, pitch)
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC7(BC7Settings::alpha_basic()).block_byte_size()
    }
}

#[cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7"))]
fn decompress_rgba8<D: BlockRgba8Decoder>(
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [u8],
) {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let block_byte_size = D::block_byte_size() as usize;
    let output_row_pitch = width as usize * 4; // Always RGBA

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = (by * blocks_x + bx) as usize;
            let block_offset = block_index * block_byte_size;

            if block_offset + block_byte_size > blocks_data.len() {
                break;
            }

            let output_offset = (by * 4 * output_row_pitch as u32 + bx * 16) as usize;

            if output_offset < rgba_data.len() {
                D::decode_block_rgba8(
                    &blocks_data[block_offset..block_offset + block_byte_size],
                    &mut rgba_data[output_offset..],
                    output_row_pitch,
                );
            }
        }
    }
}

#[cfg(feature = "bc6h")]
impl BlockRgba16fDecoder for BC6HDecoder {
    #[inline(always)]
    fn decode_block_rgba16f(compressed: &[u8], decompressed: &mut [half::f16], pitch: usize) {
        decode_block_bc6h(compressed, decompressed, pitch, false);
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC6H(BC6HSettings::basic()).block_byte_size()
    }
}

#[cfg(feature = "bc6h")]
fn decompress_rgba16f<D: BlockRgba16fDecoder>(
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [half::f16],
) {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let block_byte_size = D::block_byte_size() as usize;
    let output_row_pitch = width as usize * 4; // Always RGBA16f

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = (by * blocks_x + bx) as usize;
            let block_offset = block_index * block_byte_size;

            if block_offset + block_byte_size > blocks_data.len() {
                break;
            }

            let output_offset = (by * 4 * output_row_pitch as u32 + bx * 16) as usize;

            if output_offset < rgba_data.len() {
                D::decode_block_rgba16f(
                    &blocks_data[block_offset..block_offset + block_byte_size],
                    &mut rgba_data[output_offset..],
                    output_row_pitch,
                );
            }
        }
    }
}

#[cfg(feature = "bc6h")]
impl BlockRgba32fDecoder for BC6HDecoder {
    #[inline(always)]
    fn decode_block_rgba32f(compressed: &[u8], decompressed: &mut [f32], pitch: usize) {
        decode_block_bc6h_float(compressed, decompressed, pitch, false);
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC6H(BC6HSettings::basic()).block_byte_size()
    }
}

#[cfg(feature = "bc6h")]
fn decompress_rgba32f<D: BlockRgba32fDecoder>(
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [f32],
) {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let block_byte_size = D::block_byte_size() as usize;
    let output_row_pitch = width as usize * 4; // Always RGBA32f

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = (by * blocks_x + bx) as usize;
            let block_offset = block_index * block_byte_size;

            if block_offset + block_byte_size > blocks_data.len() {
                break;
            }

            let output_offset = (by * 4 * output_row_pitch as u32 + bx * 16) as usize;

            if output_offset < rgba_data.len() {
                D::decode_block_rgba32f(
                    &blocks_data[block_offset..block_offset + block_byte_size],
                    &mut rgba_data[output_offset..],
                    output_row_pitch,
                );
            }
        }
    }
}

/// Helper function to easily decompress block data into RGBA8 data.
///
/// # Panics
/// - The `blocks_data` has not the expected size (`variant.blocks_byte_size()`)
/// - The `rgba_data` has not the expected size (`width * height * 4`)
#[cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "bc15", feature = "bc6h", feature = "bc7")))
)]
pub fn decompress_blocks_as_rgba8(
    variant: CompressionVariant,
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [u8],
) {
    let expected_input_size = variant.blocks_byte_size(width, height);
    assert_eq!(
        blocks_data.len(),
        expected_input_size,
        "the input bitstream slice has not the expected size"
    );

    let expected_output_size = width as usize * height as usize * 4;
    assert_eq!(
        rgba_data.len(),
        expected_output_size,
        "the output slice has not the expected size"
    );

    match variant {
        #[cfg(feature = "bc15")]
        CompressionVariant::BC1 => {
            decompress_rgba8::<BC1Decoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc15")]
        CompressionVariant::BC2 => {
            decompress_rgba8::<BC2Decoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc15")]
        CompressionVariant::BC3 => {
            decompress_rgba8::<BC3Decoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc15")]
        CompressionVariant::BC4 => {
            decompress_rgba8::<BC4Decoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc15")]
        CompressionVariant::BC5 => {
            decompress_rgba8::<BC5Decoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc6h")]
        CompressionVariant::BC6H(..) => {
            decompress_rgba8::<BC6HDecoder>(width, height, blocks_data, rgba_data)
        }
        #[cfg(feature = "bc7")]
        CompressionVariant::BC7(..) => {
            decompress_rgba8::<BC7Decoder>(width, height, blocks_data, rgba_data)
        }
    }
}

/// Helper function to easily decompress block data into RGBA16F data. Only BCH6 is currently supported.
///
/// # Panics
/// - The `blocks_data` has not the expected size (`variant.blocks_byte_size()`)
/// - The `rgba_data` has not the expected size (`width * height * 4`)
/// - If `variant` is any other value than BC6H.
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
pub fn decompress_blocks_as_rgba16f(
    variant: CompressionVariant,
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [half::f16],
) {
    let expected_input_size = variant.blocks_byte_size(width, height);

    assert_eq!(
        blocks_data.len(),
        expected_input_size,
        "the input bitstream slice has not the expected size"
    );

    let expected_output_size = width as usize * height as usize * 4;
    assert_eq!(
        rgba_data.len(),
        expected_output_size,
        "the output slice has not the expected size"
    );

    match variant {
        CompressionVariant::BC6H(..) => {
            decompress_rgba16f::<BC6HDecoder>(width, height, blocks_data, rgba_data)
        }
        #[allow(unreachable_patterns)]
        _ => {
            panic!("unsupported compression variant");
        }
    }
}

/// Helper function to easily decompress block data into RGBA32F data. Only BCH6 is currently supported.
///
/// # Panics
/// - The `blocks_data` has not the expected size (`variant.blocks_byte_size()`)
/// - The `rgba_data` has not the expected size (`width * height * 4`)
/// - If `variant` is any other value than BC6H.
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
pub fn decompress_blocks_as_rgba32f(
    variant: CompressionVariant,
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [f32],
) {
    let expected_input_size = variant.blocks_byte_size(width, height);
    assert_eq!(
        blocks_data.len(),
        expected_input_size,
        "the input bitstream slice has not the expected size"
    );

    let expected_output_size = width as usize * height as usize * 4;
    assert_eq!(
        rgba_data.len(),
        expected_output_size,
        "the output slice has not the expected size"
    );

    match variant {
        CompressionVariant::BC6H(..) => {
            decompress_rgba32f::<BC6HDecoder>(width, height, blocks_data, rgba_data)
        }
        #[allow(unreachable_patterns)]
        _ => {
            panic!("unsupported compression variant");
        }
    }
}
