//! Direct Rust port the "bcdec.h - v0.98"
//!
//! <https://github.com/iOrange/bcdec/blob/main/bcdec.h>
//!
//! # CREDITS
//!
//! Aras Pranckevicius (@aras-p)
//! - BC1/BC3 decoders optimizations (up to 3x the speed)
//! - BC6H/BC7 bits pulling routines optimizations
//! - optimized BC6H by moving unquantize out of the loop
//! - Split BC6H decompression function into 'half' and
//!   'float' variants
//!
//! Michael Schmidt (@RunDevelopment)
//! - Found better "magic" coefficients for integer interpolation
//!   of reference colors in BC1 color block, that match with
//!   the floating point interpolation. This also made it faster
//!   than integer division by 3!
//!
//! # License
//!
//! This is free and unencumbered software released into the public domain.
//!
//! Anyone is free to copy, modify, publish, use, compile, sell, or
//! distribute this software, either in source code form or as a compiled
//! binary, for any purpose, commercial or non-commercial, and by any
//! means.
//!
//! In jurisdictions that recognize copyright laws, the author or authors
//! of this software dedicate any and all copyright interest in the
//! software to the public domain. We make this dedication for the benefit
//! of the public at large and to the detriment of our heirs and
//! successors. We intend this dedication to be an overt act of
//! relinquishment in perpetuity of all present and future rights to this
//! software under copyright law.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
//! OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
//! ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
//! OTHER DEALINGS IN THE SOFTWARE.
//!
//! For more information, please refer to <https://unlicense.org>

/// Decodes a BC1 block by reading 8 bytes from `compressed_block` and writing the RGBA8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc15")]
#[inline(always)]
pub fn decode_block_bc1(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<false>(compressed_block, decompressed_block, destination_pitch);
}

/// Decodes a BC2 block by reading 16 bytes from `compressed_block` and writing the RGBA8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc15")]
#[inline(always)]
pub fn decode_block_bc2(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<true>(
        &compressed_block[8..],
        decompressed_block,
        destination_pitch,
    );
    decode_sharp_alpha_block(compressed_block, decompressed_block, destination_pitch);
}

/// Decodes a BC3 block by reading 16 bytes from `compressed_block` and writing the RGBA8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc15")]
#[inline(always)]
pub fn decode_block_bc3(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<true>(
        &compressed_block[8..],
        decompressed_block,
        destination_pitch,
    );
    decode_smooth_alpha_block::<4>(
        compressed_block,
        &mut decompressed_block[3..],
        destination_pitch,
    );
}

/// Decodes a BC4 block by reading 8 bytes from `compressed_block` and writing the R8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc15")]
#[inline(always)]
pub fn decode_block_bc4(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_smooth_alpha_block::<1>(compressed_block, decompressed_block, destination_pitch);
}

/// Decodes a BC5 block by reading 16 bytes from `compressed_block` and writing the RG8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc15")]
#[inline(always)]
pub fn decode_block_bc5(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_smooth_alpha_block::<2>(compressed_block, decompressed_block, destination_pitch);
    decode_smooth_alpha_block::<2>(
        &compressed_block[8..],
        &mut decompressed_block[1..],
        destination_pitch,
    );
}

/// Decompresses a BC1/DXT1 color block
#[cfg(feature = "bc15")]
#[inline(always)]
fn decode_color_block<const OPAQUE_MODE: bool>(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    let mut ref_colors = [0u32; 4];
    let c0 = u16::from_le_bytes([compressed_block[0], compressed_block[1]]);
    let c1 = u16::from_le_bytes([compressed_block[2], compressed_block[3]]);

    // Unpack 565 ref colors
    let r0 = (c0 >> 11) & 0x1F;
    let g0 = (c0 >> 5) & 0x3F;
    let b0 = c0 & 0x1F;

    let r1 = (c1 >> 11) & 0x1F;
    let g1 = (c1 >> 5) & 0x3F;
    let b1 = c1 & 0x1F;

    // Expand 565 ref colors to 888
    let r = (r0 as u32 * 527 + 23) >> 6;
    let g = (g0 as u32 * 259 + 33) >> 6;
    let b = (b0 as u32 * 527 + 23) >> 6;
    ref_colors[0] = 0xFF000000 | (b << 16) | (g << 8) | r;

    let r = (r1 as u32 * 527 + 23) >> 6;
    let g = (g1 as u32 * 259 + 33) >> 6;
    let b = (b1 as u32 * 527 + 23) >> 6;
    ref_colors[1] = 0xFF000000 | (b << 16) | (g << 8) | r;

    if c0 > c1 || OPAQUE_MODE {
        // Standard BC1 mode (also BC3 color block uses ONLY this mode)
        // color_2 = 2/3*color_0 + 1/3*color_1
        // color_3 = 1/3*color_0 + 2/3*color_1
        let r = ((2 * r0 as u32 + r1 as u32) * 351 + 61) >> 7;
        let g = ((2 * g0 as u32 + g1 as u32) * 2763 + 1039) >> 11;
        let b = ((2 * b0 as u32 + b1 as u32) * 351 + 61) >> 7;
        ref_colors[2] = 0xFF000000 | (b << 16) | (g << 8) | r;

        let r = ((r0 as u32 + r1 as u32 * 2) * 351 + 61) >> 7;
        let g = ((g0 as u32 + g1 as u32 * 2) * 2763 + 1039) >> 11;
        let b = ((b0 as u32 + b1 as u32 * 2) * 351 + 61) >> 7;
        ref_colors[3] = 0xFF000000 | (b << 16) | (g << 8) | r;
    } else {
        // Quite rare BC1A mode
        // color_2 = 1/2*color_0 + 1/2*color_1
        // color_3 = 0
        let r = ((r0 as u32 + r1 as u32) * 1053 + 125) >> 8;
        let g = ((g0 as u32 + g1 as u32) * 4145 + 1019) >> 11;
        let b = ((b0 as u32 + b1 as u32) * 1053 + 125) >> 8;
        ref_colors[2] = 0xFF000000 | (b << 16) | (g << 8) | r;
        ref_colors[3] = 0x00000000;
    }

    let mut color_indices = u32::from_le_bytes([
        compressed_block[4],
        compressed_block[5],
        compressed_block[6],
        compressed_block[7],
    ]);

    // Fill out the decompressed color block
    for i in 0..4 {
        for j in 0..4 {
            let idx = color_indices & 0x03;
            let offset = j * 4;
            let color = ref_colors[idx as usize];

            decompressed_block[i * destination_pitch + offset..][..4]
                .copy_from_slice(&color.to_le_bytes());

            color_indices >>= 2;
        }
    }
}

/// Decodes a BC2/DXT3 alpha block (sharp transitions)
#[cfg(feature = "bc15")]
#[inline(always)]
fn decode_sharp_alpha_block(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    for i in 0..4 {
        for j in 0..4 {
            let byte_index = i * 2 + (j / 2);
            let shift = (j % 2) * 4;
            let alpha_value = (compressed_block[byte_index] >> shift) & 0x0F;
            decompressed_block[i * destination_pitch + j * 4 + 3] = alpha_value * 17;
        }
    }
}

/// Decodes a BC2/DXT3 alpha block (smooth transitions)
#[cfg(feature = "bc15")]
#[inline(always)]
#[rustfmt::skip]
fn decode_smooth_alpha_block<const PIXEL_SIZE: usize>(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    let block = u64::from_le_bytes(compressed_block[0..8].try_into().unwrap());

    let mut alpha = [0u8; 8];
    alpha[0] = (block & 0xFF) as u8;
    alpha[1] = ((block >> 8) & 0xFF) as u8;

    if alpha[0] > alpha[1] {
        // 6 interpolated alpha values
        alpha[2] = ((6 * alpha[0] as u16 +     alpha[1] as u16) / 7) as u8;   // 6/7*alpha_0 + 1/7*alpha_1
        alpha[3] = ((5 * alpha[0] as u16 + 2 * alpha[1] as u16) / 7) as u8;   // 5/7*alpha_0 + 2/7*alpha_1
        alpha[4] = ((4 * alpha[0] as u16 + 3 * alpha[1] as u16) / 7) as u8;   // 4/7*alpha_0 + 3/7*alpha_1
        alpha[5] = ((3 * alpha[0] as u16 + 4 * alpha[1] as u16) / 7) as u8;   // 3/7*alpha_0 + 4/7*alpha_1
        alpha[6] = ((2 * alpha[0] as u16 + 5 * alpha[1] as u16) / 7) as u8;   // 2/7*alpha_0 + 5/7*alpha_1
        alpha[7] = ((    alpha[0] as u16 + 6 * alpha[1] as u16) / 7) as u8;   // 1/7*alpha_0 + 6/7*alpha_1
    } else {
        // 4 interpolated alpha values
        alpha[2] = ((4 * alpha[0] as u16 +     alpha[1] as u16) / 5) as u8;   // 4/5*alpha_0 + 1/5*alpha_1
        alpha[3] = ((3 * alpha[0] as u16 + 2 * alpha[1] as u16) / 5) as u8;   // 3/5*alpha_0 + 2/5*alpha_1
        alpha[4] = ((2 * alpha[0] as u16 + 3 * alpha[1] as u16) / 5) as u8;   // 2/5*alpha_0 + 3/5*alpha_1
        alpha[5] = ((    alpha[0] as u16 + 4 * alpha[1] as u16) / 5) as u8;   // 1/5*alpha_0 + 4/5*alpha_1
        alpha[6] = 0x00;
        alpha[7] = 0xFF;
    }

    let mut indices = block >> 16;

    for i in 0..4 {
        for j in 0..4 {
            decompressed_block[i * destination_pitch + j * PIXEL_SIZE] = alpha[(indices & 0x07) as usize];
            indices >>= 3;
        }
    }
}

/// Decodes a BC7 block by reading 16 bytes from `compressed_block` and writing the RGB16F data (half float) into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc6h")]
pub fn decode_block_bc6h(
    compressed_block: &[u8],
    decompressed_block: &mut [half::f16],
    destination_pitch: usize,
    is_signed: bool,
) {
    use half::f16;

    static ACTUAL_BITS_COUNT: &[[u8; 14]; 4] = &[
        [10, 7, 11, 11, 11, 9, 8, 8, 8, 6, 10, 11, 12, 16], // W
        [5, 6, 5, 4, 4, 5, 6, 5, 5, 6, 10, 9, 8, 4],        // dR
        [5, 6, 4, 5, 4, 5, 5, 6, 5, 6, 10, 9, 8, 4],        // dG
        [5, 6, 4, 4, 5, 5, 5, 5, 6, 6, 10, 9, 8, 4],        // dB
    ];

    // There are 32 possible partition sets for a two-region tile.
    // Each 4x4 block represents a single shape.
    //Here also every fix-up index has MSB bit set.
    static PARTITION_SETS: &[[[u8; 4]; 4]; 32] = &[
        [[128, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 129]], //  0
        [[128, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 129]], //  1
        [[128, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 129]], //  2
        [[128, 0, 0, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  3
        [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 1, 129]], //  4
        [[128, 0, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  5
        [[128, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  6
        [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  7
        [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 129]], //  8
        [[128, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], //  9
        [[128, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 129]], // 10
        [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 129]], // 11
        [[128, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 12
        [[128, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 129]], // 13
        [[128, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 14
        [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 129]], // 15
        [[128, 0, 0, 0], [1, 0, 0, 0], [1, 1, 1, 0], [1, 1, 1, 129]], // 16
        [[128, 1, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 17
        [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 1, 0]], // 18
        [[128, 1, 129, 1], [0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 19
        [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 20
        [[128, 0, 0, 0], [1, 0, 0, 0], [129, 1, 0, 0], [1, 1, 1, 0]], // 21
        [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 22
        [[128, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 0, 129]], // 23
        [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 24
        [[128, 0, 0, 0], [1, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 25
        [[128, 1, 129, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0]], // 26
        [[128, 0, 129, 1], [0, 1, 1, 0], [0, 1, 1, 0], [1, 1, 0, 0]], // 27
        [[128, 0, 0, 1], [0, 1, 1, 1], [129, 1, 1, 0], [1, 0, 0, 0]], // 28
        [[128, 0, 0, 0], [1, 1, 1, 1], [129, 1, 1, 1], [0, 0, 0, 0]], // 29
        [[128, 1, 129, 1], [0, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0]], // 30
        [[128, 0, 129, 1], [1, 0, 0, 1], [1, 0, 0, 1], [1, 1, 0, 0]], // 31
    ];

    const WEIGHT3: &[i32] = &[0, 9, 18, 27, 37, 46, 55, 64];
    const WEIGHT4: &[i32] = &[0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

    let mut bstream = BitStream::new(compressed_block);

    let mut r = [0i32; 4];
    let mut g = [0i32; 4];
    let mut b = [0i32; 4];

    let mut mode = bstream.read_bits(2);
    if mode > 1 {
        mode |= bstream.read_bits(3) << 2;
    }

    // modes >= 11 (10 in my code) are using 0 one, others will read it from the bitstream
    let mut partition = 0;

    match mode {
        // Mode 1
        0b00 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 75 bits (10.555, 10.555, 10.555)
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(5); // rx[4:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(5); // gx[4:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(5); // bx[4:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(5); // ry[4:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(5); // rz[4:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 0;
        }

        // Mode 2
        0b01 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 75 bits (7666, 7666, 7666)
            g[2] |= bstream.read_bit_i32() << 5; // gy[5]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[3] |= bstream.read_bit_i32() << 5; // gz[5]
            r[0] |= bstream.read_bits_i32(7); // rw[6:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(7); // gw[6:0]
            b[2] |= bstream.read_bit_i32() << 5; // by[5]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(7); // bw[6:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            b[3] |= bstream.read_bit_i32() << 5; // bz[5]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(6); // rx[5:0]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(6); // gx[5:0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(6); // bx[5:0]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(6); // ry[5:0]
            r[3] |= bstream.read_bits_i32(6); // rz[5:0]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 1;
        }

        // Mode 3
        0b00010 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (11.555, 11.444, 11.444)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(5); // rx[4:0]
            r[0] |= bstream.read_bit_i32() << 10; // rw[10]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(4); // gx[3:0]
            g[0] |= bstream.read_bit_i32() << 10; // gw[10]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(4); // bx[3:0]
            b[0] |= bstream.read_bit_i32() << 10; // bw[10]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(5); // ry[4:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(5); // rz[4:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 2;
        }
        // Mode 4
        0b00110 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (11.444, 11.555, 11.444)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(4); // rx[3:0]
            r[0] |= bstream.read_bit_i32() << 10; // rw[10]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(5); // gx[4:0]
            g[0] |= bstream.read_bit_i32() << 10; // gw[10]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(4); // bx[3:0]
            b[0] |= bstream.read_bit_i32() << 10; // bw[10]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(4); // ry[3:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(4); // rz[3:0]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 3;
        }
        // Mode 5
        0b01010 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (11.444, 11.444, 11.555)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(4); // rx[3:0]
            r[0] |= bstream.read_bit_i32() << 10; // rw[10]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(4); // gx[3:0]
            g[0] |= bstream.read_bit_i32() << 10; // gw[10]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(5); // bx[4:0]
            b[0] |= bstream.read_bit_i32() << 10; // bw[10]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(4); // ry[3:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(4); // rz[3:0]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 4;
        }
        // Mode 6
        0b01110 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (9555, 9555, 9555)
            r[0] |= bstream.read_bits_i32(9); // rw[8:0]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(9); // gw[8:0]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(9); // bw[8:0]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(5); // rx[4:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(5); // gx[4:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gx[3:0]
            b[1] |= bstream.read_bits_i32(5); // bx[4:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(5); // ry[4:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(5); // rz[4:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 5;
        }
        // Mode 7
        0b10010 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (8666, 8555, 8555)
            r[0] |= bstream.read_bits_i32(8); // rw[7:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(8); // gw[7:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(8); // bw[7:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(6); // rx[5:0]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(5); // gx[4:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(5); // bx[4:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(6); // ry[5:0]
            r[3] |= bstream.read_bits_i32(6); // rz[5:0]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 6;
        }
        // Mode 8
        0b10110 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (8555, 8666, 8555)
            r[0] |= bstream.read_bits_i32(8); // rw[7:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(8); // gw[7:0]
            g[2] |= bstream.read_bit_i32() << 5; // gy[5]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(8); // bw[7:0]
            g[3] |= bstream.read_bit_i32() << 5; // gz[5]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(5); // rx[4:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(6); // gx[5:0]
            g[3] |= bstream.read_bits_i32(4); // zx[3:0]
            b[1] |= bstream.read_bits_i32(5); // bx[4:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(5); // ry[4:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(5); // rz[4:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 7;
        }
        // Mode 9
        0b11010 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (8555, 8555, 8666)
            r[0] |= bstream.read_bits_i32(8); // rw[7:0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(8); // gw[7:0]
            b[2] |= bstream.read_bit_i32() << 5; // by[5]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(8); // bw[7:0]
            b[3] |= bstream.read_bit_i32() << 5; // bz[5]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(5); // bw[4:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(5); // gx[4:0]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(6); // bx[5:0]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(5); // ry[4:0]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            r[3] |= bstream.read_bits_i32(5); // rz[4:0]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 8;
        }
        // Mode 10
        0b11110 => {
            // Partition indices: 46 bits
            // Partition: 5 bits
            // Color Endpoints: 72 bits (6666, 6666, 6666)
            r[0] |= bstream.read_bits_i32(6); // rw[5:0]
            g[3] |= bstream.read_bit_i32() << 4; // gz[4]
            b[3] |= bstream.read_bit_i32(); // bz[0]
            b[3] |= bstream.read_bit_i32() << 1; // bz[1]
            b[2] |= bstream.read_bit_i32() << 4; // by[4]
            g[0] |= bstream.read_bits_i32(6); // gw[5:0]
            g[2] |= bstream.read_bit_i32() << 5; // gy[5]
            b[2] |= bstream.read_bit_i32() << 5; // by[5]
            b[3] |= bstream.read_bit_i32() << 2; // bz[2]
            g[2] |= bstream.read_bit_i32() << 4; // gy[4]
            b[0] |= bstream.read_bits_i32(6); // bw[5:0]
            g[3] |= bstream.read_bit_i32() << 5; // gz[5]
            b[3] |= bstream.read_bit_i32() << 3; // bz[3]
            b[3] |= bstream.read_bit_i32() << 5; // bz[5]
            b[3] |= bstream.read_bit_i32() << 4; // bz[4]
            r[1] |= bstream.read_bits_i32(6); // rx[5:0]
            g[2] |= bstream.read_bits_i32(4); // gy[3:0]
            g[1] |= bstream.read_bits_i32(6); // gx[5:0]
            g[3] |= bstream.read_bits_i32(4); // gz[3:0]
            b[1] |= bstream.read_bits_i32(6); // bx[5:0]
            b[2] |= bstream.read_bits_i32(4); // by[3:0]
            r[2] |= bstream.read_bits_i32(6); // ry[5:0]
            r[3] |= bstream.read_bits_i32(6); // rz[5:0]
            partition = bstream.read_bits_i32(5); // d[4:0]
            mode = 9;
        }
        // Mode 11
        0b00011 => {
            // Partition indices: 63 bits
            // Partition: 0 bits
            // Color Endpoints: 60 bits (10.10, 10.10, 10.10)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(10); // rx[9:0]
            g[1] |= bstream.read_bits_i32(10); // gx[9:0]
            b[1] |= bstream.read_bits_i32(10); // bx[9:0]
            mode = 10;
        }
        // Mode 12
        0b00111 => {
            // Partition indices: 63 bits
            // Partition: 0 bits
            // Color Endpoints: 60 bits (11.9, 11.9, 11.9)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(9); // rx[8:0]
            r[0] |= bstream.read_bit_i32() << 10; // rw[10]
            g[1] |= bstream.read_bits_i32(9); // gx[8:0]
            g[0] |= bstream.read_bit_i32() << 10; // gw[10]
            b[1] |= bstream.read_bits_i32(9); // bx[8:0]
            b[0] |= bstream.read_bit_i32() << 10; // bw[10]
            mode = 11;
        }
        // Mode 13
        0b01011 => {
            // Partition indices: 63 bits
            // Partition: 0 bits
            // Color Endpoints: 60 bits (12.8, 12.8, 12.8)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(8); // rx[7:0]
            r[0] |= bstream.read_bits_reversed(2) << 10; // rx[10:11]
            g[1] |= bstream.read_bits_i32(8); // gx[7:0]
            g[0] |= bstream.read_bits_reversed(2) << 10; // gx[10:11]
            b[1] |= bstream.read_bits_i32(8); // bx[7:0]
            b[0] |= bstream.read_bits_reversed(2) << 10; // bx[10:11]
            mode = 12;
        }
        // Mode 14
        0b01111 => {
            // Partition indices: 63 bits
            // Partition: 0 bits
            // Color Endpoints: 60 bits (16.4, 16.4, 16.4)
            r[0] |= bstream.read_bits_i32(10); // rw[9:0]
            g[0] |= bstream.read_bits_i32(10); // gw[9:0]
            b[0] |= bstream.read_bits_i32(10); // bw[9:0]
            r[1] |= bstream.read_bits_i32(4); // rx[3:0]
            r[0] |= bstream.read_bits_reversed(6) << 10; // rw[10:15]
            g[1] |= bstream.read_bits_i32(4); // gx[3:0]
            g[0] |= bstream.read_bits_reversed(6) << 10; // gw[10:15]
            b[1] |= bstream.read_bits_i32(4); // bx[3:0]
            b[0] |= bstream.read_bits_reversed(6) << 10; // bw[10:15]
            mode = 13;
        }
        _ => {
            // Modes 10011, 10111, 11011, and 11111 (not shown) are reserved.
            // Do not use these in your encoder. If the hardware is passed blocks
            // with one of these modes specified, the resulting decompressed block
            // must contain all zeroes in all channels except for the alpha channel.
            for i in 0..4 {
                let start = i * destination_pitch;
                let end = start + 4 * 3;
                decompressed_block[start..end].fill(f16::ZERO);
            }

            return;
        }
    }

    let num_partitions = if mode >= 10 { 0 } else { 1 };

    let actual_bits0_mode = ACTUAL_BITS_COUNT[0][mode as usize] as i32;
    if is_signed {
        r[0] = extend_sign(r[0], actual_bits0_mode);
        g[0] = extend_sign(g[0], actual_bits0_mode);
        b[0] = extend_sign(b[0], actual_bits0_mode);
    }

    // Mode 11 (like Mode 10) does not use delta compression,
    // and instead stores both color endpoints explicitly.
    if mode != 9 && mode != 10 || is_signed {
        for i in 1..(num_partitions + 1) * 2 {
            r[i] = extend_sign(r[i], ACTUAL_BITS_COUNT[1][mode as usize] as i32);
            g[i] = extend_sign(g[i], ACTUAL_BITS_COUNT[2][mode as usize] as i32);
            b[i] = extend_sign(b[i], ACTUAL_BITS_COUNT[3][mode as usize] as i32);
        }
    }

    if mode != 9 && mode != 10 {
        for i in 1..(num_partitions + 1) * 2 {
            r[i] = transform_inverse(r[i], r[0], actual_bits0_mode, is_signed);
            g[i] = transform_inverse(g[i], g[0], actual_bits0_mode, is_signed);
            b[i] = transform_inverse(b[i], b[0], actual_bits0_mode, is_signed);
        }
    }

    for i in 0..(num_partitions + 1) * 2 {
        r[i] = unquantize(r[i], actual_bits0_mode, is_signed);
        g[i] = unquantize(g[i], actual_bits0_mode, is_signed);
        b[i] = unquantize(b[i], actual_bits0_mode, is_signed);
    }

    let weights = if mode >= 10 { WEIGHT4 } else { WEIGHT3 };

    for i in 0..4 {
        for j in 0..4 {
            let mut partition_set = if mode >= 10 {
                if i | j == 0 {
                    128i32
                } else {
                    0i32
                }
            } else {
                PARTITION_SETS[partition as usize][i][j] as i32
            };

            let mut index_bits = if mode >= 10 { 4 } else { 3 };

            // fix-up index is specified with one less bit
            // The fix-up index for subset 0 is always index 0
            if (partition_set & 0x80) != 0 {
                index_bits -= 1;
            }
            partition_set &= 0x01;

            let index = bstream.read_bits_i32(index_bits);

            let ep_i = (partition_set * 2) as usize;
            let out = i * destination_pitch + j * 3;

            decompressed_block[out] = f16::from_bits(finish_unquantize(
                interpolate(r[ep_i], r[ep_i + 1], weights, index),
                is_signed,
            ));
            decompressed_block[out + 1] = f16::from_bits(finish_unquantize(
                interpolate(g[ep_i], g[ep_i + 1], weights, index),
                is_signed,
            ));
            decompressed_block[out + 2] = f16::from_bits(finish_unquantize(
                interpolate(b[ep_i], b[ep_i + 1], weights, index),
                is_signed,
            ));
        }
    }
}

/// Decodes a BC6H block by reading 16 bytes from `compressed_block` and writing the RGB32F data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc6h")]
#[inline(always)]
pub fn decode_block_bc6h_float(
    compressed_block: &[u8],
    decompressed_block: &mut [f32],
    destination_pitch: usize,
    is_signed: bool,
) {
    let mut block = [half::f16::ZERO; 48];
    decode_block_bc6h(compressed_block, &mut block, 12, is_signed);

    let mut decompressed = decompressed_block;

    for i in 0..4 {
        for j in 0..4 {
            let offset = i * 12 + j * 3;
            let pixel_offset = j * 3;

            decompressed[pixel_offset] = block[offset].to_f32();
            decompressed[pixel_offset + 1] = block[offset + 1].to_f32();
            decompressed[pixel_offset + 2] = block[offset + 2].to_f32();
        }
        decompressed = &mut decompressed[destination_pitch..];
    }
}

/// Decodes a BC7 block by reading 16 bytes from `compressed_block` and writing the RGBA8 data into `decompressed_block` with `destination_pitch` many bytes per output row.
#[cfg(feature = "bc7")]
#[allow(clippy::needless_range_loop)]
pub fn decode_block_bc7(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    static ACTUAL_BITS_COUNT: &[[u8; 8]; 2] = &[
        [4, 6, 5, 7, 5, 7, 7, 5], // RGBA
        [0, 0, 0, 0, 6, 8, 7, 5], // Alpha
    ];

    // There are 64 possible partition sets for a two-region tile.
    // Each 4x4 block represents a single shape.
    // Here also every fix-up index has MSB bit set.
    static PARTITION_SETS: &[[[[u8; 4]; 4]; 64]; 2] = &[
        [
            // Partition table for 2-subset BPTC
            [[128, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 129]], //  0
            [[128, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 129]], //  1
            [[128, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 129]], //  2
            [[128, 0, 0, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  3
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 1, 129]], //  4
            [[128, 0, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  5
            [[128, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  6
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  7
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 129]], //  8
            [[128, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], //  9
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 129]], // 10
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 129]], // 11
            [[128, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 12
            [[128, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 129]], // 13
            [[128, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 14
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 129]], // 15
            [[128, 0, 0, 0], [1, 0, 0, 0], [1, 1, 1, 0], [1, 1, 1, 129]], // 16
            [[128, 1, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 17
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 1, 0]], // 18
            [[128, 1, 129, 1], [0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 19
            [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 20
            [[128, 0, 0, 0], [1, 0, 0, 0], [129, 1, 0, 0], [1, 1, 1, 0]], // 21
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 22
            [[128, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 0, 129]], // 23
            [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 24
            [[128, 0, 0, 0], [1, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 25
            [[128, 1, 129, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0]], // 26
            [[128, 0, 129, 1], [0, 1, 1, 0], [0, 1, 1, 0], [1, 1, 0, 0]], // 27
            [[128, 0, 0, 1], [0, 1, 1, 1], [129, 1, 1, 0], [1, 0, 0, 0]], // 28
            [[128, 0, 0, 0], [1, 1, 1, 1], [129, 1, 1, 1], [0, 0, 0, 0]], // 29
            [[128, 1, 129, 1], [0, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0]], // 30
            [[128, 0, 129, 1], [1, 0, 0, 1], [1, 0, 0, 1], [1, 1, 0, 0]], // 31
            [[128, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 129]], // 32
            [[128, 0, 0, 0], [1, 1, 1, 1], [0, 0, 0, 0], [1, 1, 1, 129]], // 33
            [[128, 1, 0, 1], [1, 0, 129, 0], [0, 1, 0, 1], [1, 0, 1, 0]], // 34
            [[128, 0, 1, 1], [0, 0, 1, 1], [129, 1, 0, 0], [1, 1, 0, 0]], // 35
            [[128, 0, 129, 1], [1, 1, 0, 0], [0, 0, 1, 1], [1, 1, 0, 0]], // 36
            [[128, 1, 0, 1], [0, 1, 0, 1], [129, 0, 1, 0], [1, 0, 1, 0]], // 37
            [[128, 1, 1, 0], [1, 0, 0, 1], [0, 1, 1, 0], [1, 0, 0, 129]], // 38
            [[128, 1, 0, 1], [1, 0, 1, 0], [1, 0, 1, 0], [0, 1, 0, 129]], // 39
            [[128, 1, 129, 1], [0, 0, 1, 1], [1, 1, 0, 0], [1, 1, 1, 0]], // 40
            [[128, 0, 0, 1], [0, 0, 1, 1], [129, 1, 0, 0], [1, 0, 0, 0]], // 41
            [[128, 0, 129, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 1, 0, 0]], // 42
            [[128, 0, 129, 1], [1, 0, 1, 1], [1, 1, 0, 1], [1, 1, 0, 0]], // 43
            [[128, 1, 129, 0], [1, 0, 0, 1], [1, 0, 0, 1], [0, 1, 1, 0]], // 44
            [[128, 0, 1, 1], [1, 1, 0, 0], [1, 1, 0, 0], [0, 0, 1, 129]], // 45
            [[128, 1, 1, 0], [0, 1, 1, 0], [1, 0, 0, 1], [1, 0, 0, 129]], // 46
            [[128, 0, 0, 0], [0, 1, 129, 0], [0, 1, 1, 0], [0, 0, 0, 0]], // 47
            [[128, 1, 0, 0], [1, 1, 129, 0], [0, 1, 0, 0], [0, 0, 0, 0]], // 48
            [[128, 0, 129, 0], [0, 1, 1, 1], [0, 0, 1, 0], [0, 0, 0, 0]], // 49
            [[128, 0, 0, 0], [0, 0, 129, 0], [0, 1, 1, 1], [0, 0, 1, 0]], // 50
            [[128, 0, 0, 0], [0, 1, 0, 0], [129, 1, 1, 0], [0, 1, 0, 0]], // 51
            [[128, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 1], [0, 0, 1, 129]], // 52
            [[128, 0, 1, 1], [0, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 129]], // 53
            [[128, 1, 129, 0], [0, 0, 1, 1], [1, 0, 0, 1], [1, 1, 0, 0]], // 54
            [[128, 0, 129, 1], [1, 0, 0, 1], [1, 1, 0, 0], [0, 1, 1, 0]], // 55
            [[128, 1, 1, 0], [1, 1, 0, 0], [1, 1, 0, 0], [1, 0, 0, 129]], // 56
            [[128, 1, 1, 0], [0, 0, 1, 1], [0, 0, 1, 1], [1, 0, 0, 129]], // 57
            [[128, 1, 1, 1], [1, 1, 1, 0], [1, 0, 0, 0], [0, 0, 0, 129]], // 58
            [[128, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0], [0, 1, 1, 129]], // 59
            [[128, 0, 0, 0], [1, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 129]], // 60
            [[128, 0, 129, 1], [0, 0, 1, 1], [1, 1, 1, 1], [0, 0, 0, 0]], // 61
            [[128, 0, 129, 0], [0, 0, 1, 0], [1, 1, 1, 0], [1, 1, 1, 0]], // 62
            [[128, 1, 0, 0], [0, 1, 0, 0], [0, 1, 1, 1], [0, 1, 1, 129]], // 63
        ],
        [
            // Partition table for 3-subset BPTC
            [[128, 0, 1, 129], [0, 0, 1, 1], [0, 2, 2, 1], [2, 2, 2, 130]], //  0
            [[128, 0, 0, 129], [0, 0, 1, 1], [130, 2, 1, 1], [2, 2, 2, 1]], //  1
            [[128, 0, 0, 0], [2, 0, 0, 1], [130, 2, 1, 1], [2, 2, 1, 129]], //  2
            [[128, 2, 2, 130], [0, 0, 2, 2], [0, 0, 1, 1], [0, 1, 1, 129]], //  3
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 1, 2, 2], [1, 1, 2, 130]], //  4
            [[128, 0, 1, 129], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 2, 130]], //  5
            [[128, 0, 2, 130], [0, 0, 2, 2], [1, 1, 1, 1], [1, 1, 1, 129]], //  6
            [[128, 0, 1, 1], [0, 0, 1, 1], [130, 2, 1, 1], [2, 2, 1, 129]], //  7
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 1, 1, 1], [2, 2, 2, 130]], //  8
            [[128, 0, 0, 0], [1, 1, 1, 1], [129, 1, 1, 1], [2, 2, 2, 130]], //  9
            [[128, 0, 0, 0], [1, 1, 129, 1], [2, 2, 2, 2], [2, 2, 2, 130]], // 10
            [[128, 0, 1, 2], [0, 0, 129, 2], [0, 0, 1, 2], [0, 0, 1, 130]], // 11
            [[128, 1, 1, 2], [0, 1, 129, 2], [0, 1, 1, 2], [0, 1, 1, 130]], // 12
            [[128, 1, 2, 2], [0, 129, 2, 2], [0, 1, 2, 2], [0, 1, 2, 130]], // 13
            [[128, 0, 1, 129], [0, 1, 1, 2], [1, 1, 2, 2], [1, 2, 2, 130]], // 14
            [[128, 0, 1, 129], [2, 0, 0, 1], [130, 2, 0, 0], [2, 2, 2, 0]], // 15
            [[128, 0, 0, 129], [0, 0, 1, 1], [0, 1, 1, 2], [1, 1, 2, 130]], // 16
            [[128, 1, 1, 129], [0, 0, 1, 1], [130, 0, 0, 1], [2, 2, 0, 0]], // 17
            [[128, 0, 0, 0], [1, 1, 2, 2], [129, 1, 2, 2], [1, 1, 2, 130]], // 18
            [[128, 0, 2, 130], [0, 0, 2, 2], [0, 0, 2, 2], [1, 1, 1, 129]], // 19
            [[128, 1, 1, 129], [0, 1, 1, 1], [0, 2, 2, 2], [0, 2, 2, 130]], // 20
            [[128, 0, 0, 129], [0, 0, 0, 1], [130, 2, 2, 1], [2, 2, 2, 1]], // 21
            [[128, 0, 0, 0], [0, 0, 129, 1], [0, 1, 2, 2], [0, 1, 2, 130]], // 22
            [[128, 0, 0, 0], [1, 1, 0, 0], [130, 2, 129, 0], [2, 2, 1, 0]], // 23
            [[128, 1, 2, 130], [0, 129, 2, 2], [0, 0, 1, 1], [0, 0, 0, 0]], // 24
            [[128, 0, 1, 2], [0, 0, 1, 2], [129, 1, 2, 2], [2, 2, 2, 130]], // 25
            [[128, 1, 1, 0], [1, 2, 130, 1], [129, 2, 2, 1], [0, 1, 1, 0]], // 26
            [[128, 0, 0, 0], [0, 1, 129, 0], [1, 2, 130, 1], [1, 2, 2, 1]], // 27
            [[128, 0, 2, 2], [1, 1, 0, 2], [129, 1, 0, 2], [0, 0, 2, 130]], // 28
            [[128, 1, 1, 0], [0, 129, 1, 0], [2, 0, 0, 2], [2, 2, 2, 130]], // 29
            [[128, 0, 1, 1], [0, 1, 2, 2], [0, 1, 130, 2], [0, 0, 1, 129]], // 30
            [[128, 0, 0, 0], [2, 0, 0, 0], [130, 2, 1, 1], [2, 2, 2, 129]], // 31
            [[128, 0, 0, 0], [0, 0, 0, 2], [129, 1, 2, 2], [1, 2, 2, 130]], // 32
            [[128, 2, 2, 130], [0, 0, 2, 2], [0, 0, 1, 2], [0, 0, 1, 129]], // 33
            [[128, 0, 1, 129], [0, 0, 1, 2], [0, 0, 2, 2], [0, 2, 2, 130]], // 34
            [[128, 1, 2, 0], [0, 129, 2, 0], [0, 1, 130, 0], [0, 1, 2, 0]], // 35
            [[128, 0, 0, 0], [1, 1, 129, 1], [2, 2, 130, 2], [0, 0, 0, 0]], // 36
            [[128, 1, 2, 0], [1, 2, 0, 1], [130, 0, 129, 2], [0, 1, 2, 0]], // 37
            [[128, 1, 2, 0], [2, 0, 1, 2], [129, 130, 0, 1], [0, 1, 2, 0]], // 38
            [[128, 0, 1, 1], [2, 2, 0, 0], [1, 1, 130, 2], [0, 0, 1, 129]], // 39
            [[128, 0, 1, 1], [1, 1, 130, 2], [2, 2, 0, 0], [0, 0, 1, 129]], // 40
            [[128, 1, 0, 129], [0, 1, 0, 1], [2, 2, 2, 2], [2, 2, 2, 130]], // 41
            [[128, 0, 0, 0], [0, 0, 0, 0], [130, 1, 2, 1], [2, 1, 2, 129]], // 42
            [[128, 0, 2, 2], [1, 129, 2, 2], [0, 0, 2, 2], [1, 1, 2, 130]], // 43
            [[128, 0, 2, 130], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 1, 129]], // 44
            [[128, 2, 2, 0], [1, 2, 130, 1], [0, 2, 2, 0], [1, 2, 2, 129]], // 45
            [[128, 1, 0, 1], [2, 2, 130, 2], [2, 2, 2, 2], [0, 1, 0, 129]], // 46
            [[128, 0, 0, 0], [2, 1, 2, 1], [130, 1, 2, 1], [2, 1, 2, 129]], // 47
            [[128, 1, 0, 129], [0, 1, 0, 1], [0, 1, 0, 1], [2, 2, 2, 130]], // 48
            [[128, 2, 2, 130], [0, 1, 1, 1], [0, 2, 2, 2], [0, 1, 1, 129]], // 49
            [[128, 0, 0, 2], [1, 129, 1, 2], [0, 0, 0, 2], [1, 1, 1, 130]], // 50
            [[128, 0, 0, 0], [2, 129, 1, 2], [2, 1, 1, 2], [2, 1, 1, 130]], // 51
            [[128, 2, 2, 2], [0, 129, 1, 1], [0, 1, 1, 1], [0, 2, 2, 130]], // 52
            [[128, 0, 0, 2], [1, 1, 1, 2], [129, 1, 1, 2], [0, 0, 0, 130]], // 53
            [[128, 1, 1, 0], [0, 129, 1, 0], [0, 1, 1, 0], [2, 2, 2, 130]], // 54
            [[128, 0, 0, 0], [0, 0, 0, 0], [2, 1, 129, 2], [2, 1, 1, 130]], // 55
            [[128, 1, 1, 0], [0, 129, 1, 0], [2, 2, 2, 2], [2, 2, 2, 130]], // 56
            [[128, 0, 2, 2], [0, 0, 1, 1], [0, 0, 129, 1], [0, 0, 2, 130]], // 57
            [[128, 0, 2, 2], [1, 1, 2, 2], [129, 1, 2, 2], [0, 0, 2, 130]], // 58
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [2, 129, 1, 130]], // 59
            [[128, 0, 0, 130], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 129]], // 60
            [[128, 2, 2, 2], [1, 2, 2, 2], [0, 2, 2, 2], [129, 2, 2, 130]], // 61
            [[128, 1, 0, 129], [2, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 130]], // 62
            [[128, 1, 1, 129], [2, 0, 1, 1], [130, 2, 0, 1], [2, 2, 2, 0]], // 63
        ],
    ];

    static WEIGHT2: &[i32] = &[0, 21, 43, 64];
    static WEIGHT3: &[i32] = &[0, 9, 18, 27, 37, 46, 55, 64];
    static WEIGHT4: &[i32] = &[0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

    const MODE_HAS_P_BITS: u8 = 0b11001011;

    let mut bstream = BitStream::new(compressed_block);

    // Find mode
    let mut mode = 0;
    while mode < 8 && bstream.read_bit() == 0 {
        mode += 1;
    }

    // Unexpected mode, clear the block (transparent black)
    if mode >= 8 {
        for i in 0..4 {
            for j in 0..4 {
                let offset = i * destination_pitch + j * 4;
                decompressed_block[offset..offset + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        return;
    }

    let mut partition = 0;
    let mut num_partitions = 1;
    let mut rotation = 0;
    let mut index_selection_bit = 0;

    if mode == 0 || mode == 1 || mode == 2 || mode == 3 || mode == 7 {
        num_partitions = if mode == 0 || mode == 2 { 3 } else { 2 };
        partition = bstream.read_bits(if mode == 0 { 4 } else { 6 }) as usize;
    }

    let num_endpoints = num_partitions * 2;

    if mode == 4 || mode == 5 {
        rotation = bstream.read_bits(2);
        if mode == 4 {
            index_selection_bit = bstream.read_bit();
        }
    }

    // Extract endpoints
    let mut endpoints = [[0i32; 4]; 6];

    // RGB
    for i in 0..3 {
        for j in 0..num_endpoints {
            endpoints[j][i] = bstream.read_bits(ACTUAL_BITS_COUNT[0][mode as usize] as u32) as i32;
        }
    }

    // Alpha (if any)
    if ACTUAL_BITS_COUNT[1][mode as usize] > 0 {
        for j in 0..num_endpoints {
            endpoints[j][3] = bstream.read_bits(ACTUAL_BITS_COUNT[1][mode as usize] as u32) as i32;
        }
    }

    // Fully decode endpoints
    // Handle modes that have P-bits
    if mode == 0 || mode == 1 || mode == 3 || mode == 6 || mode == 7 {
        // Component-wise left-shift
        for endpoint in endpoints.iter_mut().take(num_endpoints) {
            for component in endpoint.iter_mut() {
                *component <<= 1;
            }
        }

        // If P-bit is shared
        if mode == 1 {
            let i = bstream.read_bit() as i32;
            let j = bstream.read_bit() as i32;

            // RGB component-wise insert pbits
            for k in 0..3 {
                endpoints[0][k] |= i;
                endpoints[1][k] |= i;
                endpoints[2][k] |= j;
                endpoints[3][k] |= j;
            }
        } else if MODE_HAS_P_BITS & (1 << mode) != 0 {
            // Unique P-bit per endpoint
            for endpoint in endpoints.iter_mut().take(num_endpoints) {
                let j = bstream.read_bit() as i32;
                for component in endpoint.iter_mut() {
                    *component |= j;
                }
            }
        }
    }

    // Fully decode endpoints
    // Component-wise precision adjustment
    for i in 0..num_endpoints {
        // Get color components precision including pbit
        let j = ACTUAL_BITS_COUNT[0][mode as usize] + ((MODE_HAS_P_BITS >> mode) & 1);

        // RGB components
        for k in 0..3 {
            // Left shift endpoint components so that their MSB lies in bit 7
            endpoints[i][k] <<= 8 - j;
            // Replicate each component's MSB into the LSBs revealed by the left-shift operation
            endpoints[i][k] |= endpoints[i][k] >> j as i32;
        }

        // Get alpha component precision including pbit
        let j = ACTUAL_BITS_COUNT[1][mode as usize] + ((MODE_HAS_P_BITS >> mode) & 1);

        // Alpha component
        endpoints[i][3] <<= 8 - j;
        endpoints[i][3] |= endpoints[i][3] >> j as i32;
    }

    // If this mode does not explicitly define the alpha component, set alpha to 255 (1.0)
    if ACTUAL_BITS_COUNT[1][mode as usize] == 0 {
        for endpoint in endpoints.iter_mut().take(num_endpoints) {
            endpoint[3] = 0xFF;
        }
    }

    // Determine weights tables
    let index_bits = match mode {
        0 | 1 => 3,
        6 => 4,
        _ => 2,
    };

    let index_bits2 = match mode {
        4 => 3,
        5 => 2,
        _ => 0,
    };

    let weights = match index_bits {
        2 => WEIGHT2,
        3 => WEIGHT3,
        _ => WEIGHT4,
    };

    let weights2 = match index_bits2 {
        2 => WEIGHT2,
        _ => WEIGHT3,
    };

    // Collect indices in two passes
    // Pass #1: collecting color indices
    let mut indices = [[0i32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let partition_set = if num_partitions == 1 {
                if i | j == 0 {
                    128
                } else {
                    0
                }
            } else {
                PARTITION_SETS[num_partitions - 2][partition][i][j]
            };

            let mut idx_bits = match mode {
                0 | 1 => 3,
                6 => 4,
                _ => 2,
            };

            // Fix-up index is specified with one less bit
            // The fix-up index for subset 0 is always index 0
            if partition_set & 0x80 != 0 {
                idx_bits -= 1;
            }

            indices[i][j] = bstream.read_bits(idx_bits) as i32;
        }
    }

    // Pass #2: reading alpha indices (if any) and interpolating & rotating
    for i in 0..4 {
        for j in 0..4 {
            let partition_set = if num_partitions == 1 {
                if i | j == 0 {
                    128
                } else {
                    0
                }
            } else {
                PARTITION_SETS[num_partitions - 2][partition][i][j]
            };
            let partition_set = (partition_set & 0x03) as usize;

            let index = indices[i][j];

            let (mut r, mut g, mut b, mut a) = if index_bits2 == 0 {
                // No secondary index bits
                (
                    interpolate(
                        endpoints[partition_set * 2][0],
                        endpoints[partition_set * 2 + 1][0],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][1],
                        endpoints[partition_set * 2 + 1][1],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][2],
                        endpoints[partition_set * 2 + 1][2],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][3],
                        endpoints[partition_set * 2 + 1][3],
                        weights,
                        index,
                    ),
                )
            } else {
                let index2 = bstream.read_bits(if i | j == 0 {
                    index_bits2 - 1
                } else {
                    index_bits2
                }) as i32;

                if index_selection_bit == 0 {
                    (
                        interpolate(
                            endpoints[partition_set * 2][0],
                            endpoints[partition_set * 2 + 1][0],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][1],
                            endpoints[partition_set * 2 + 1][1],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][2],
                            endpoints[partition_set * 2 + 1][2],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][3],
                            endpoints[partition_set * 2 + 1][3],
                            weights2,
                            index2,
                        ),
                    )
                } else {
                    (
                        interpolate(
                            endpoints[partition_set * 2][0],
                            endpoints[partition_set * 2 + 1][0],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][1],
                            endpoints[partition_set * 2 + 1][1],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][2],
                            endpoints[partition_set * 2 + 1][2],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][3],
                            endpoints[partition_set * 2 + 1][3],
                            weights,
                            index,
                        ),
                    )
                }
            };

            // Handle rotation
            match rotation {
                1 => std::mem::swap(&mut a, &mut r), // 01 – Block format is Scalar(R) Vector(AGB) - swap A and R
                2 => std::mem::swap(&mut a, &mut g), // 10 – Block format is Scalar(G) Vector(RAB) - swap A and G
                3 => std::mem::swap(&mut a, &mut b), // 11 - Block format is Scalar(B) Vector(RGA) - swap A and B
                _ => {}
            }

            let offset = i * destination_pitch + j * 4;
            decompressed_block[offset] = r as u8;
            decompressed_block[offset + 1] = g as u8;
            decompressed_block[offset + 2] = b as u8;
            decompressed_block[offset + 3] = a as u8;
        }
    }
}

#[cfg(any(feature = "bc6h", feature = "bc7"))]
#[inline]
fn interpolate(a: i32, b: i32, weights: &[i32], index: i32) -> i32 {
    (a * (64 - weights[index as usize]) + b * weights[index as usize] + 32) >> 6
}

#[cfg(feature = "bc6h")]
#[inline]
fn extend_sign(val: i32, bits: i32) -> i32 {
    // http://graphics.stanford.edu/~seander/bithacks.html#VariableSignExtend
    (val << (32 - bits)) >> (32 - bits)
}

#[cfg(feature = "bc6h")]
#[inline]
fn transform_inverse(val: i32, a0: i32, bits: i32, is_signed: bool) -> i32 {
    // If the precision of A0 is "p" bits, then the transform algorithm is:
    // B0 = (B0 + A0) & ((1 << p) - 1)
    let transformed = (val + a0) & ((1 << bits) - 1);
    if is_signed {
        extend_sign(transformed, bits)
    } else {
        transformed
    }
}

#[cfg(feature = "bc6h")]
#[inline]
fn unquantize(val: i32, bits: i32, is_signed: bool) -> i32 {
    if !is_signed {
        if bits >= 15 {
            val
        } else if val == 0 {
            0
        } else if val == ((1 << bits) - 1) {
            0xFFFF
        } else {
            ((val << 16) + 0x8000) >> bits
        }
    } else if bits >= 16 {
        val
    } else {
        let (s, v) = if val < 0 { (true, -val) } else { (false, val) };

        let unq = if v == 0 {
            0
        } else if v >= ((1 << (bits - 1)) - 1) {
            0x7FFF
        } else {
            ((v << 15) + 0x4000) >> (bits - 1)
        };

        if s {
            -unq
        } else {
            unq
        }
    }
}

#[cfg(feature = "bc6h")]
#[inline]
fn finish_unquantize(val: i32, is_signed: bool) -> u16 {
    if !is_signed {
        // Scale the magnitude by 31 / 64
        ((val * 31) >> 6) as u16
    } else {
        // Scale the magnitude by 31 / 32
        let scaled = if val < 0 {
            -(((-val) * 31) >> 5)
        } else {
            (val * 31) >> 5
        };

        let (sign_bit, magnitude) = if scaled < 0 {
            (0x8000, -scaled)
        } else {
            (0, scaled)
        };

        (sign_bit | magnitude) as u16
    }
}

/// Internal bitstream helper for reading bits from compressed data
#[cfg(any(feature = "bc6h", feature = "bc7"))]
#[derive(Debug, Clone, Copy)]
struct BitStream {
    low: u64,
    high: u64,
}

#[cfg(any(feature = "bc6h", feature = "bc7"))]
impl BitStream {
    /// Create a new bitstream from raw data.
    #[inline]
    fn new(data: &[u8]) -> Self {
        let low = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let high = u64::from_le_bytes(data[8..16].try_into().unwrap());
        Self { low, high }
    }

    #[cfg(feature = "bc7")]
    #[inline]
    fn read_bit(&mut self) -> u32 {
        self.read_bits(1)
    }

    #[cfg(feature = "bc6h")]
    #[inline]
    fn read_bit_i32(&mut self) -> i32 {
        self.read_bits(1) as i32
    }

    #[inline]
    pub fn read_bits(&mut self, num_bits: u32) -> u32 {
        let mask = (1u64 << num_bits) - 1;
        // Read the low N bits.
        let bits = (self.low & mask) as u32;
        self.low >>= num_bits;

        // Put the low N bits of "high" into the high 64-N bits of "low".
        self.low |= (self.high & mask) << (64 - num_bits);
        self.high >>= num_bits;

        bits
    }

    #[cfg(feature = "bc6h")]
    #[inline]
    pub fn read_bits_i32(&mut self, num_bits: u32) -> i32 {
        self.read_bits(num_bits) as i32
    }

    #[cfg(feature = "bc6h")]
    #[inline]
    fn read_bits_reversed(&mut self, num_bits: u32) -> i32 {
        let mut bits = self.read_bits_i32(num_bits);
        // Reverse the bits.
        let mut result = 0;

        (0..num_bits).for_each(|_| {
            result <<= 1;
            result |= bits & 1;
            bits >>= 1;
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_block(
        decode_block: fn(&[u8], &mut [u8], usize),
        pitch: usize,
        compressed_block: &[u8],
        expected_output: &[u8],
        name: &str,
    ) {
        let mut decoded = [0u8; 64];
        decode_block(compressed_block, &mut decoded, pitch);

        for y in 0..4 {
            let start = y * pitch;
            let end = start + pitch;
            assert_eq!(
                &decoded[start..end],
                &expected_output[start..end],
                "{name}: Mismatch at row {y}",
            );
        }
    }

    #[test]
    fn test_bc1_block_black() {
        let compressed_block = [0u8; 8];
        let expected_output = [
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
        ];
        test_block(
            decode_block_bc1,
            16,
            &compressed_block,
            &expected_output,
            "Black block",
        );
    }

    #[test]
    fn test_bc1_block_red() {
        let compressed_block = [0x00, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
        ];
        test_block(
            decode_block_bc1,
            16,
            &compressed_block,
            &expected_output,
            "Red block",
        );
    }

    #[test]
    fn test_bc1_block_gradient() {
        let compressed_block = [0x00, 0xF8, 0xE0, 0x07, 0x55, 0x55, 0x55, 0x55];
        let expected_output = [
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
        ];
        test_block(
            decode_block_bc1,
            16,
            &compressed_block,
            &expected_output,
            "Gradient block",
        );
    }

    #[test]
    fn test_bc2_alpha_gradient() {
        let compressed_block = [
            0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x11, 0xFF, 0x0, 0x0, 0x22, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0x44, 0xFF, 0x0, 0x0, 0x55, 0xFF, 0x0, 0x0, 0x66, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x88, 0xFF, 0x0, 0x0, 0x99, 0xFF, 0x0, 0x0, 0xAA, 0xFF, 0x0, 0x0, 0xBB,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0xDD, 0xFF, 0x0, 0x0, 0xEE, 0xFF, 0x0, 0x0, 0xFF,
        ];
        test_block(
            decode_block_bc2,
            16,
            &compressed_block,
            &expected_output,
            "Alpha gradient",
        );
    }

    #[test]
    fn test_bc2_alpha_half_transparent() {
        let compressed_block = [
            0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
        ];
        test_block(
            decode_block_bc2,
            16,
            &compressed_block,
            &expected_output,
            "Half transparent",
        );
    }

    #[test]
    fn test_bc3_solid_black() {
        let compressed_block = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc3,
            16,
            &compressed_block,
            &expected_output,
            "Solid black with full alpha",
        );
    }

    #[test]
    fn test_bc3_transparent_red() {
        let compressed_block = [
            0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc3,
            16,
            &compressed_block,
            &expected_output,
            "Transparent red",
        );
    }

    #[test]
    fn test_bc3_alpha_gradient() {
        let compressed_block = [
            0x00, 0xFF, 0xFF, 0xFF, 0x55, 0x55, 0x55, 0x55, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0x66, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
        ];
        test_block(
            decode_block_bc3,
            16,
            &compressed_block,
            &expected_output,
            "Red with alpha gradient",
        );
    }

    #[test]
    fn test_bc3_color_alpha_gradient() {
        let compressed_block = [
            0x00, 0xFF, 0xFF, 0xFF, 0x55, 0x55, 0x55, 0x55, 0x00, 0xF8, 0xE0, 0x07, 0x55, 0x55,
            0x55, 0x55,
        ];
        let expected_output = [
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0x66, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
            0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
            0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
        ];
        test_block(
            decode_block_bc3,
            16,
            &compressed_block,
            &expected_output,
            "Color and alpha gradients",
        );
    }

    #[test]
    fn test_bc3_semi_transparent() {
        let compressed_block = [
            0x80, 0x80, 0xFF, 0xFF, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
            0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
            0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
        ];
        test_block(
            decode_block_bc3,
            16,
            &compressed_block,
            &expected_output,
            "Semi-transparent red",
        );
    }

    #[test]
    fn test_bc4_gradient() {
        let compressed_block = [0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
        let expected_output = [
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc4,
            4,
            &compressed_block,
            &expected_output,
            "BC4 gradient",
        );
    }

    #[test]
    fn test_bc4_interpolated() {
        let compressed_block = [0x00, 0xFF, 0x92, 0x24, 0x49, 0x92, 0x00, 0x00];
        let expected_output = [
            0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc4,
            4,
            &compressed_block,
            &expected_output,
            "BC4 interpolated",
        );
    }

    #[test]
    fn test_bc5_gradient() {
        let compressed_block = [
            0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x24, 0xFF, 0x24, 0xFF, 0x24, 0xFF, 0x24, 0xFF, 0x24, 0xFF, 0x0, 0x0, 0xFF, 0x0,
            0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc5,
            8,
            &compressed_block,
            &expected_output,
            "BC5 gradient",
        );
    }

    #[test]
    fn test_bc5_interpolated() {
        let compressed_block = [
            0x00, 0xFF, 0x92, 0x24, 0x49, 0x92, 0x00, 0x00, 0xFF, 0x00, 0x92, 0x24, 0x49, 0x92,
            0x00, 0x00,
        ];
        let expected_output = [
            0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA,
            0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x33, 0xDA, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0,
            0xFF, 0x0, 0xFF, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0,
        ];
        test_block(
            decode_block_bc5,
            8,
            &compressed_block,
            &expected_output,
            "BC5 interpolated",
        );
    }

    #[cfg(feature = "bc6h")]
    #[test]
    fn test_bc6h_block_0() {
        use half::f16;

        let compressed_block = [
            0x40, 0xAF, 0xF6, 0x0B, 0xFD, 0x2E, 0xFF, 0xFF, 0x11, 0x71, 0x10, 0xA1, 0x21, 0xF2,
            0x33, 0x73,
        ];
        let expected_output = [
            f16::from_bits(0x5BAB),
            f16::from_bits(0x84B9),
            f16::from_bits(0xDBE9),
            f16::from_bits(0x5BA2),
            f16::from_bits(0x84F6),
            f16::from_bits(0xDBF1),
            f16::from_bits(0x5B99),
            f16::from_bits(0x8533),
            f16::from_bits(0xDBFA),
            f16::from_bits(0x5D9B),
            f16::from_bits(0x8307),
            f16::from_bits(0xD847),
            f16::from_bits(0x5B7E),
            f16::from_bits(0x85F0),
            f16::from_bits(0xDC15),
            f16::from_bits(0x5BA2),
            f16::from_bits(0x84F6),
            f16::from_bits(0xDBF1),
            f16::from_bits(0x5CC3),
            f16::from_bits(0x81E8),
            f16::from_bits(0xD8D6),
            f16::from_bits(0x5D9B),
            f16::from_bits(0x8307),
            f16::from_bits(0xD847),
            f16::from_bits(0x5BA2),
            f16::from_bits(0x84F6),
            f16::from_bits(0xDBF1),
            f16::from_bits(0x5B6D),
            f16::from_bits(0x866B),
            f16::from_bits(0xDC27),
            f16::from_bits(0x5C27),
            f16::from_bits(0x8117),
            f16::from_bits(0xD93F),
            f16::from_bits(0x5CC3),
            f16::from_bits(0x81E8),
            f16::from_bits(0xD8D6),
            f16::from_bits(0x5BA2),
            f16::from_bits(0x84F6),
            f16::from_bits(0xDBF1),
            f16::from_bits(0x5CFE),
            f16::from_bits(0x8235),
            f16::from_bits(0xD8AF),
            f16::from_bits(0x5C5B),
            f16::from_bits(0x815C),
            f16::from_bits(0xD91C),
            f16::from_bits(0x5D66),
            f16::from_bits(0x82C1),
            f16::from_bits(0xD869),
        ];

        let mut decoded = [f16::ZERO; 48];
        decode_block_bc6h(&compressed_block, &mut decoded, 12, true);

        assert_eq!(&decoded[..], &expected_output[..], "BC6H block mismatch");
    }

    #[test]
    #[rustfmt::skip]
    fn test_bc6h_block_0_float() {
        let compressed_block = [
            0x40, 0xAF, 0xF6, 0x0B, 0xFD, 0x2E, 0xFF, 0xFF, 0x11, 0x71, 0x10, 0xA1, 0x21, 0xF2,
            0x33, 0x73,
        ];

        let expected_output: [f32; 48] = [
            245.375, -0.000072062016, -253.125, 244.25, -0.0000756979, -254.125, 243.125, -0.00007933378, -255.25, 358.75, -0.0000461936, -136.875, 239.75, -0.00009059906, -261.25, 244.25,
            -0.0000756979, -254.125, 304.75, -0.000029087067, -154.75, 358.75, -0.0000461936, -136.875, 244.25, -0.0000756979, -254.125, 237.625, -0.00009793043, -265.75, 265.75, -0.000016629696,
            -167.875, 304.75, -0.000029087067, -154.75, 244.25, -0.0000756979, -254.125, 319.5, -0.000033676624, -149.875, 278.75, -0.000020742416, -163.5, 345.5, -0.000042021275, -141.125
        ];

        let mut decoded = [0.0_f32; 48];
        decode_block_bc6h_float(&compressed_block, &mut decoded, 12, true);

        assert_eq!(&decoded[..], &expected_output[..], "BC6H block mismatch");
    }

    #[test]
    fn test_bc7_block_0() {
        let compressed_block = [
            0x40, 0xAF, 0xF6, 0xB, 0xFD, 0x2E, 0xFF, 0xFF, 0x11, 0x71, 0x10, 0xA1, 0x21, 0xF2,
            0x33, 0x73,
        ];
        let expected_output = [
            0xBD, 0xBF, 0xBF, 0xFF, 0xBD, 0xBD, 0xBD, 0xFF, 0xBD, 0xBF, 0xBF, 0xFF, 0xBD, 0xBD,
            0xBD, 0xFF, 0xBD, 0xBD, 0xBD, 0xFF, 0xBC, 0xBB, 0xB9, 0xFF, 0xBB, 0xB9, 0xB7, 0xFF,
            0xBB, 0xB9, 0xB7, 0xFF, 0xBB, 0xB9, 0xB7, 0xFF, 0xB9, 0xB1, 0xAC, 0xFF, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc7,
            8,
            &compressed_block,
            &expected_output,
            "BC7 block 0",
        );
    }

    #[test]
    fn test_bc7_block_1() {
        let compressed_block = [
            0xC0, 0x8C, 0xEF, 0xA2, 0xBB, 0xDC, 0xFE, 0x7F, 0x6C, 0x55, 0x6A, 0x34, 0x4F, 0x0,
            0x5D, 0x0,
        ];
        let expected_output = [
            0x50, 0x4A, 0x48, 0xFE, 0x50, 0x4A, 0x48, 0xFE, 0x64, 0x5D, 0x59, 0xFE, 0x50, 0x4A,
            0x48, 0xFE, 0x7C, 0x74, 0x6E, 0xFE, 0x46, 0x41, 0x3F, 0xFE, 0x72, 0x6A, 0x65, 0xFE,
            0x4A, 0x45, 0x43, 0xFE, 0x32, 0x2E, 0x2E, 0xFE, 0x32, 0x2E, 0x2E, 0xFE, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc7,
            8,
            &compressed_block,
            &expected_output,
            "BC7 block 1",
        );
    }
}
