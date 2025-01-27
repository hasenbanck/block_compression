mod constants;
#[cfg(test)]
mod test;

use std::f32::consts::PI;

use bytemuck::{cast_slice, cast_slice_mut};
pub(crate) use constants::ASTC_MAX_RANKED_MODES;

use self::constants::{
    ASTC_PACKED_MODES_COUNT, FILTERBANK, FILTER_DATA, PACKED_MODES, RANGE_TABLE,
};
use crate::ASTCSettings;

const STRIDE: usize = 8;
const PITCH: usize = 64;

fn sq(v: f32) -> f32 {
    v * v
}

fn dot3(a: &[f32], b: &[f32]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dot4(a: &[f32], b: &[f32]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

fn get_bit(input: u32, a: i32) -> u32 {
    get_field(input, a, a)
}

fn get_bits(value: u32, from: i32, to: i32) -> u32 {
    (value >> from) & (1u32.wrapping_shl((to + 1 - from) as u32).wrapping_sub(1))
}

fn get_field(input: u32, a: i32, b: i32) -> u32 {
    debug_assert!(a >= b);
    (input >> b) & ((1 << (a - b + 1)) - 1)
}

struct PixelSet {
    pixels: [f32; 256],
    block_width: i32,
    block_height: i32,
}

impl PixelSet {
    fn rotate_plane(&mut self, p: usize) {
        for y in 0..self.block_height as usize {
            for x in 0..self.block_width as usize {
                let mut r = get_pixel(&self.pixels, 0, x, y);
                let mut g = get_pixel(&self.pixels, 1, x, y);
                let mut b = get_pixel(&self.pixels, 2, x, y);
                let mut a = get_pixel(&self.pixels, 3, x, y);

                match p {
                    0 => std::mem::swap(&mut a, &mut r),
                    1 => std::mem::swap(&mut a, &mut g),
                    2 => std::mem::swap(&mut a, &mut b),
                    _ => {}
                }

                set_pixel(&mut self.pixels, 0, x, y, r);
                set_pixel(&mut self.pixels, 1, x, y, g);
                set_pixel(&mut self.pixels, 2, x, y, b);
                set_pixel(&mut self.pixels, 3, x, y, a);
            }
        }
    }

    fn dct_4(values: &mut [f32], stride: usize) {
        const SCALE: [f32; 2] = [0.5, 0.707106769];
        const C: [f32; 5] = [1.0, 0.923879533, 0.707106769, 0.382683432, 0.0];

        let mut data = [0.0f32; 4];
        for i in 0..2 {
            let a = values[stride * i];
            let b = values[stride * (3 - i)];
            data[i] = a + b;
            data[2 + i] = a - b;
        }

        for i in 0..4 {
            let mut acc = 0.0;
            let input = &data[(i % 2) * 2..];

            for j in 0..2 {
                let mut e = (2 * j + 1) * i;
                e %= 4 * 4;
                let mut w = 1.0;

                if e > 8 {
                    e = 16 - e;
                }
                if e > 4 {
                    w = -1.0;
                    e = 8 - e;
                }

                w *= C[e];
                acc += w * input[j];
            }

            values[stride * i] = acc * SCALE[usize::from(i > 0)];
        }
    }

    fn dct_6(values: &mut [f32], stride: usize) {
        const SCALE: [f32; 2] = [0.408248290, 0.577350269];
        const C: [f32; 7] = [
            1.0,
            0.965925813,
            0.866025388,
            0.707106769,
            0.500000000,
            0.258819044,
            0.0,
        ];

        let mut data = [0.0f32; 6];
        for i in 0..3 {
            let a = values[stride * i];
            let b = values[stride * (5 - i)];
            data[i] = a + b;
            data[3 + i] = a - b;
        }

        for i in 0..6 {
            let mut acc = 0.0;
            let input = &data[(i % 2) * 3..];

            for j in 0..3 {
                let mut e = (2 * j + 1) * i;
                e %= 4 * 6;
                let mut w = 1.0;

                if e > 12 {
                    e = 24 - e;
                }
                if e > 6 {
                    w = -1.0;
                    e = 12 - e;
                }

                w *= C[e];
                acc += w * input[j];
            }

            values[stride * i] = acc * SCALE[usize::from(i > 0)];
        }
    }

    fn dct_n(values: &mut [f32], stride: usize, n: usize) {
        debug_assert!(n <= 16);

        let mut c = [0.0f32; 17]; // 16 + 1
        for i in 0..=n {
            c[i] = ((i as f32) / (4.0 * n as f32) * 2.0 * PI).cos();
        }

        let scale = [1.0 / (1.0 * n as f32).sqrt(), 1.0 / (n as f32 / 2.0).sqrt()];

        let mut data = [0.0f32; 16];
        for i in 0..n {
            data[i] = values[stride * i];
        }

        for i in 0..n {
            let mut acc = 0.0;
            for j in 0..n {
                let mut e = (2 * j + 1) * i;
                e %= 4 * n;
                let mut w = 1.0;

                if e > 2 * n {
                    e = 4 * n - e;
                }
                if e > n {
                    w = -1.0;
                    e = 2 * n - e;
                }

                debug_assert!(e <= n);
                w *= c[e];
                acc += w * data[j];
            }

            values[stride * i] = acc * scale[usize::from(i > 0)];
        }
    }

    fn dct(values: &mut [f32], stride: usize, n: usize) {
        match n {
            8 => Self::dct_n(values, stride, 8),
            6 => Self::dct_6(values, stride),
            5 => Self::dct_n(values, stride, 5),
            4 => Self::dct_4(values, stride),
            _ => panic!("Unsupported DCT size"),
        }
    }

    fn compute_dct_inplace(&mut self, channels: i32) {
        for p in 0..channels as usize {
            // Process rows
            for y in 0..self.block_height as usize {
                let start = PITCH * p + y * STRIDE;
                let slice = &mut self.pixels[start..];
                Self::dct(slice, 1, self.block_width as usize);
            }

            // Process columns
            for x in 0..self.block_width as usize {
                let start = PITCH * p + x;
                let slice = &mut self.pixels[start..];
                Self::dct(slice, STRIDE, self.block_height as usize);
            }
        }
    }

    fn compute_moments(&self, stats: &mut [f32; 15], channels: i32) {
        for y in 0..self.block_height as usize {
            for x in 0..self.block_width as usize {
                let mut rgba = [0.0f32; 4];
                for p in 0..channels as usize {
                    rgba[p] = get_pixel(&self.pixels, p, x, y);
                }

                stats[10] += rgba[0];
                stats[11] += rgba[1];
                stats[12] += rgba[2];

                stats[0] += rgba[0] * rgba[0];
                stats[1] += rgba[0] * rgba[1];
                stats[2] += rgba[0] * rgba[2];

                stats[4] += rgba[1] * rgba[1];
                stats[5] += rgba[1] * rgba[2];

                stats[7] += rgba[2] * rgba[2];

                if channels == 4 {
                    stats[13] += rgba[3];

                    stats[3] += rgba[0] * rgba[3];
                    stats[6] += rgba[1] * rgba[3];
                    stats[8] += rgba[2] * rgba[3];
                    stats[9] += rgba[3] * rgba[3];
                }
            }
        }

        stats[14] += (self.block_height * self.block_width) as f32;
    }

    fn covar_from_stats(covar: &mut [f32; 10], stats: &[f32; 15], channels: i32) {
        covar[0] = stats[0] - stats[10] * stats[10] / stats[14];
        covar[1] = stats[1] - stats[10] * stats[11] / stats[14];
        covar[2] = stats[2] - stats[10] * stats[12] / stats[14];

        covar[4] = stats[4] - stats[11] * stats[11] / stats[14];
        covar[5] = stats[5] - stats[11] * stats[12] / stats[14];

        covar[7] = stats[7] - stats[12] * stats[12] / stats[14];

        if channels == 4 {
            covar[3] = stats[3] - stats[10] * stats[13] / stats[14];
            covar[6] = stats[6] - stats[11] * stats[13] / stats[14];
            covar[8] = stats[8] - stats[12] * stats[13] / stats[14];
            covar[9] = stats[9] - stats[13] * stats[13] / stats[14];
        }
    }

    fn compute_covar_dc(
        &self,
        covar: &mut [f32; 10],
        dc: &mut [f32; 4],
        zero_based: bool,
        channels: i32,
    ) {
        let mut stats = [0.0f32; 15];
        self.compute_moments(&mut stats, channels);

        if zero_based {
            for p in 0..4 {
                stats[10 + p] = 0.0;
            }
        }

        Self::covar_from_stats(covar, &stats, channels);
        for p in 0..channels as usize {
            dc[p] = stats[10 + p] / stats[14];
        }
    }

    fn ssymv3(a: &mut [f32; 4], covar: &[f32; 10], b: &[f32; 4]) {
        a[0] = covar[0] * b[0] + covar[1] * b[1] + covar[2] * b[2];
        a[1] = covar[1] * b[0] + covar[4] * b[1] + covar[5] * b[2];
        a[2] = covar[2] * b[0] + covar[5] * b[1] + covar[7] * b[2];
    }

    fn ssymv4(a: &mut [f32; 4], covar: &[f32; 10], b: &[f32; 4]) {
        a[0] = covar[0] * b[0] + covar[1] * b[1] + covar[2] * b[2] + covar[3] * b[3];
        a[1] = covar[1] * b[0] + covar[4] * b[1] + covar[5] * b[2] + covar[6] * b[3];
        a[2] = covar[2] * b[0] + covar[5] * b[1] + covar[7] * b[2] + covar[8] * b[3];
        a[3] = covar[3] * b[0] + covar[6] * b[1] + covar[8] * b[2] + covar[9] * b[3];
    }

    fn compute_axis(axis: &mut [f32; 4], covar: &[f32; 10], power_iterations: i32, channels: i32) {
        let mut vec = [1.0f32; 4];

        for i in 0..power_iterations {
            if channels == 3 {
                Self::ssymv3(axis, covar, &vec);
            }
            if channels == 4 {
                Self::ssymv4(axis, covar, &vec);
            }

            vec[..(channels as usize)].copy_from_slice(&axis[..(channels as usize)]);

            if i % 2 == 1 {
                // renormalize every other iteration
                let mut norm_sq = 0.0f32;
                for p in 0..channels as usize {
                    norm_sq += axis[p] * axis[p];
                }

                let rnorm = 1.0 / norm_sq.sqrt();
                for p in 0..channels as usize {
                    vec[p] *= rnorm;
                }
            }
        }

        axis[..(channels as usize)].copy_from_slice(&vec[..(channels as usize)]);
    }
}

fn get_pixel(pixels: &[f32], p: usize, x: usize, y: usize) -> f32 {
    pixels[PITCH * p + STRIDE * y + x]
}

fn set_pixel(pixels: &mut [f32], p: usize, x: usize, y: usize, value: f32) {
    pixels[PITCH * p + STRIDE * y + x] = value;
}

struct AstcBlock {
    width: i32,
    height: i32,
    dual_plane: u8,
    weight_range: i32,
    weights: [u8; 64],
    color_component_selector: i32,
    partitions: i32,
    partition_id: i32,
    color_endpoint_pairs: i32,
    channels: i32,
    color_endpoint_modes: [i32; 4],
    endpoint_range: i32,
    endpoints: [u8; 18],
}

impl AstcBlock {
    fn from_mode_ranker(mode_ranker: &ModeRankerASTC) -> Self {
        Self {
            width: mode_ranker.settings.block_width as i32,
            height: mode_ranker.settings.block_height as i32,
            dual_plane: 0,
            weight_range: 0,
            weights: [0; 64],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 1,
            channels: mode_ranker.settings.channels as i32,
            color_endpoint_modes: [0; 4],
            endpoint_range: 0,
            endpoints: [0; 18],
        }
    }

    fn from_mode_parameters(packed_mode: u32, channels: u32) -> Self {
        let mut block = Self {
            width: 2 + get_bits(packed_mode, 13, 15) as i32,
            height: 2 + get_bits(packed_mode, 16, 18) as i32,
            dual_plane: get_bits(packed_mode, 19, 19) as u8,
            weight_range: get_bits(packed_mode, 0, 3) as i32,
            weights: [0; 64],
            color_component_selector: get_bits(packed_mode, 4, 5) as i32,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 0,
            channels: channels as i32,
            color_endpoint_modes: [0; 4],
            endpoint_range: get_bits(packed_mode, 8, 12) as i32,
            endpoints: [0; 18],
        };

        block.color_endpoint_modes[0] = (get_bits(packed_mode, 6, 7) * 2 + 6) as i32;
        block.color_endpoint_pairs = 1 + (block.color_endpoint_modes[0] / 4);

        block
    }

    fn can_store(value: i32, bits: i32) -> bool {
        if value < 0 {
            return false;
        }
        if value >= 1 << bits {
            return false;
        }
        true
    }

    pub(crate) fn pack_block_mode(&self) -> i32 {
        let mut block_mode = 0;
        let d = self.dual_plane;
        let h = (self.weight_range >= 6) as i32;
        let dh = (d as i32) * 2 + h;
        let mut r = self.weight_range + 2 - if h > 0 { 6 } else { 0 };
        r = r / 2 + (r % 2) * 4;

        if Self::can_store(self.width - 4, 2) && Self::can_store(self.height - 2, 2) {
            let b = self.width - 4;
            let a = self.height - 2;
            block_mode = (dh << 9) | (b << 7) | (a << 5) | ((r & 4) << 2) | (r & 3);
        }

        if Self::can_store(self.width - 8, 2) && Self::can_store(self.height - 2, 2) {
            let b = self.width - 8;
            let a = self.height - 2;
            block_mode = (dh << 9) | (b << 7) | (a << 5) | ((r & 4) << 2) | 4 | (r & 3);
        }

        if Self::can_store(self.width - 2, 2) && Self::can_store(self.height - 8, 2) {
            let a = self.width - 2;
            let b = self.height - 8;
            block_mode = (dh << 9) | (b << 7) | (a << 5) | ((r & 4) << 2) | 8 | (r & 3);
        }

        if Self::can_store(self.width - 2, 2) && Self::can_store(self.height - 6, 1) {
            let a = self.width - 2;
            let b = self.height - 6;
            block_mode = (dh << 9) | (b << 7) | (a << 5) | ((r & 4) << 2) | 12 | (r & 3);
        }

        if Self::can_store(self.width - 2, 1) && Self::can_store(self.height - 2, 2) {
            let b = self.width;
            let a = self.height - 2;
            block_mode = (dh << 9) | (b << 7) | (a << 5) | ((r & 4) << 2) | 12 | (r & 3);
        }

        if dh == 0 && Self::can_store(self.width - 6, 2) && Self::can_store(self.height - 6, 2) {
            let a = self.width - 6;
            let b = self.height - 6;
            block_mode = (b << 9) | 256 | (a << 5) | (r << 2);
        }

        block_mode
    }

    fn sequence_bits(count: i32, range: i32) -> i32 {
        let mut bits = count * RANGE_TABLE[range as usize][0];
        bits += (count * RANGE_TABLE[range as usize][1] * 8 + 4) / 5;
        bits += (count * RANGE_TABLE[range as usize][2] * 7 + 2) / 3;
        bits
    }

    fn set_bits(data: &mut [u32; 4], pos: &mut i32, bits: i32, value: u32) {
        debug_assert!(bits <= 25);
        let word_idx = (*pos / 8) as usize;
        let shift = *pos % 8;

        let bytes: &mut [u8] = cast_slice_mut(data);

        let mut word = u32::from_le_bytes([
            bytes[word_idx],
            bytes[word_idx + 1],
            bytes[word_idx + 2],
            bytes[word_idx + 3],
        ]);

        word |= value << shift;

        let new_bytes: [u8; 4] = word.to_le_bytes();
        bytes[word_idx..word_idx + 4].copy_from_slice(&new_bytes);

        *pos += bits;
    }

    fn pack_five_trits(data: &mut [u32; 4], sequence: &[i32; 5], pos: &mut i32, n: i32) {
        let mut t = [0i32; 5];
        let mut m = [0i32; 5];

        for i in 0..5 {
            t[i] = sequence[i] >> n;
            m[i] = sequence[i] - (t[i] << n);
        }

        let c = if t[1] == 2 && t[2] == 2 {
            3 * 4 + t[0]
        } else if t[2] == 2 {
            t[1] * 16 + t[0] * 4 + 3
        } else {
            t[2] * 16 + t[1] * 4 + t[0]
        };

        let big_t = if t[3] == 2 && t[4] == 2 {
            get_field(c as u32, 4, 2) * 32 + 7 * 4 + get_field(c as u32, 1, 0)
        } else {
            let mut temp = get_field(c as u32, 4, 0);
            if t[4] == 2 {
                temp += (t[3] * 128 + 3 * 32) as u32;
            } else {
                temp += (t[4] * 128 + t[3] * 32) as u32;
            }
            temp
        };

        let mut pack1: u32 = 0;
        pack1 |= m[0] as u32;
        pack1 |= get_field(big_t, 1, 0) << n;
        pack1 |= (m[1] as u32) << (2 + n);

        let mut pack2: u32 = 0;
        pack2 |= get_field(big_t, 3, 2);
        pack2 |= (m[2] as u32) << 2;
        pack2 |= get_field(big_t, 4, 4) << (2 + n);
        pack2 |= (m[3] as u32) << (3 + n);
        pack2 |= get_field(big_t, 6, 5) << (3 + n * 2);
        pack2 |= (m[4] as u32) << (5 + n * 2);
        pack2 |= get_field(big_t, 7, 7) << (5 + n * 3);

        Self::set_bits(data, pos, 2 + n * 2, pack1);
        Self::set_bits(data, pos, 6 + n * 3, pack2);
    }

    fn pack_three_quint(data: &mut [u32; 4], sequence: &[i32; 3], pos: &mut i32, n: i32) {
        let mut q = [0i32; 3];
        let mut m = [0i32; 3];

        for i in 0..3 {
            q[i] = sequence[i] >> n;
            m[i] = sequence[i] - (q[i] << n);
        }

        let big_q = if q[0] == 4 && q[1] == 4 {
            get_field(q[2] as u32, 1, 0) * 8 + 3 * 2 + get_bit(q[2] as u32, 2)
        } else {
            let c = if q[1] == 4 {
                (q[0] << 3) + 5
            } else {
                (q[1] << 3) + q[0]
            };

            if q[2] == 4 {
                get_field(!c as u32, 2, 1) * 32
                    + get_field(c as u32, 4, 3) * 8
                    + 3 * 2
                    + get_bit(c as u32, 0)
            } else {
                (q[2] as u32) * 32 + get_field(c as u32, 4, 0)
            }
        };

        let mut pack: u32 = 0;
        pack |= m[0] as u32;
        pack |= get_field(big_q, 2, 0) << n;
        pack |= (m[1] as u32) << (3 + n);
        pack |= get_field(big_q, 4, 3) << (3 + n * 2);
        pack |= (m[2] as u32) << (5 + n * 2);
        pack |= get_field(big_q, 6, 5) << (5 + n * 3);

        Self::set_bits(data, pos, 7 + n * 3, pack);
    }

    fn pack_integer_sequence(
        output_data: &mut [u32; 4],
        sequence: &[u8],
        pos: i32,
        count: i32,
        range: i32,
    ) {
        let n = RANGE_TABLE[range as usize][0];
        let bits = Self::sequence_bits(count, range);
        let pos0 = pos;

        let mut data = [0u32; 4];
        let mut current_pos = pos;

        if RANGE_TABLE[range as usize][1] == 1 {
            for j in 0..((count + 4) / 5) {
                let mut temp = [0i32; 5];
                for i in 0..i32::min(count - j * 5, 5) {
                    temp[i as usize] = sequence[(j * 5 + i) as usize] as i32;
                }
                Self::pack_five_trits(&mut data, &temp, &mut current_pos, n);
            }
        } else if RANGE_TABLE[range as usize][2] == 1 {
            for j in 0..((count + 2) / 3) {
                let mut temp = [0i32; 3];
                for i in 0..i32::min(count - j * 3, 3) {
                    temp[i as usize] = sequence[(j * 3 + i) as usize] as i32;
                }
                Self::pack_three_quint(&mut data, &temp, &mut current_pos, n);
            }
        } else {
            for i in 0..count {
                Self::set_bits(&mut data, &mut current_pos, n, sequence[i as usize] as u32);
            }
        }

        if pos0 + bits < 96 {
            data[3] = 0;
        }
        if pos0 + bits < 64 {
            data[2] = 0;
        }
        if pos0 + bits < 32 {
            data[1] = 0;
        }
        data[((pos0 + bits) / 32) as usize] &= (1 << ((pos0 + bits) % 32)) - 1;

        for k in 0..4 {
            output_data[k] |= data[k];
        }
    }

    pub(crate) fn pack(&self, data: &mut [u32; 4]) {
        *data = [0; 4];

        let mut pos = 0;
        Self::set_bits(data, &mut pos, 11, self.pack_block_mode() as u32);

        let num_weights = self.width * self.height * (if self.dual_plane != 0 { 2 } else { 1 });
        let weight_bits = Self::sequence_bits(num_weights, self.weight_range);
        let mut extra_bits = 0;

        debug_assert!(num_weights <= 64);
        debug_assert!((24..=96).contains(&weight_bits));

        Self::set_bits(data, &mut pos, 2, (self.partitions - 1) as u32);

        if self.partitions > 1 {
            Self::set_bits(data, &mut pos, 10, self.partition_id as u32);

            let mut min_cem = 16;
            let mut max_cem = 0;
            for j in 0..self.partitions {
                min_cem = i32::min(min_cem, self.color_endpoint_modes[j as usize]);
                max_cem = i32::max(max_cem, self.color_endpoint_modes[j as usize]);
            }
            debug_assert!(max_cem / 4 <= min_cem / 4 + 1);

            let mut cem = self.color_endpoint_modes[0] << 2;
            if max_cem != min_cem {
                cem = i32::min(3, min_cem / 4 + 1);
                for j in 0..self.partitions {
                    let c = self.color_endpoint_modes[j as usize] / 4 - ((cem & 3) - 1);
                    let m = self.color_endpoint_modes[j as usize] % 4;
                    debug_assert!(c == 0 || c == 1);
                    cem |= c << (2 + j);
                    cem |= m << (2 + self.partitions + 2 * j);
                }
                extra_bits = 3 * self.partitions - 4;
                let mut pos2 = 128 - weight_bits - extra_bits;
                Self::set_bits(data, &mut pos2, extra_bits, (cem >> 6) as u32);
            }

            Self::set_bits(data, &mut pos, 6, (cem & 63) as u32);
        } else {
            Self::set_bits(data, &mut pos, 4, self.color_endpoint_modes[0] as u32);
        }

        if self.dual_plane != 0 {
            debug_assert!(self.partitions < 4);
            extra_bits += 2;
            let mut pos2 = 128 - weight_bits - extra_bits;
            Self::set_bits(data, &mut pos2, 2, self.color_component_selector as u32);
        }

        let mut num_cem_pairs = 0;
        for j in 0..self.partitions {
            num_cem_pairs += 1 + self.color_endpoint_modes[j as usize] / 4;
        }

        #[cfg(debug_assertions)]
        {
            debug_assert!(num_cem_pairs <= 9);

            let config_bits = pos + extra_bits;
            let remaining_bits = 128 - config_bits - weight_bits;

            let mut endpoint_range = self.endpoint_range;
            for range in (1..=20).rev() {
                let bits = Self::sequence_bits(2 * num_cem_pairs, range);
                if bits <= remaining_bits {
                    endpoint_range = range;
                    break;
                }
            }
            debug_assert!(endpoint_range >= 4);
            debug_assert_eq!(self.endpoint_range, endpoint_range);
        }

        Self::pack_integer_sequence(
            data,
            &self.endpoints,
            pos,
            2 * num_cem_pairs,
            self.endpoint_range,
        );

        let mut rdata = [0u32; 4];
        Self::pack_integer_sequence(&mut rdata, &self.weights, 0, num_weights, self.weight_range);

        for i in 0..4 {
            data[i] |= rdata[3 - i].reverse_bits();
        }
    }
}

pub(crate) struct ModeRankerASTC<'a> {
    pca_error: [[f32; 5]; 2],
    alpha_error: [[f32; 5]; 2],
    sq_norm: [[f32; 5]; 2],
    scale_error: [[f32; 7]; 7], // 2x2 to 8x8

    best_scores: [f32; 64],
    best_modes: [u32; 64],

    settings: &'a ASTCSettings,
}

impl<'a> ModeRankerASTC<'a> {
    pub(crate) fn new(settings: &'a ASTCSettings) -> Self {
        Self {
            pca_error: [[0.0; 5]; 2],
            alpha_error: [[0.0; 5]; 2],
            sq_norm: [[0.0; 5]; 2],
            scale_error: [[0.0; 7]; 7],
            best_scores: [0.0; 64],
            best_modes: [0; 64],
            settings,
        }
    }

    fn load_block_interleaved(
        &self,
        rgba_data: &[u8],
        xx: usize,
        yy: usize,
        stride: usize,
        pixels: &mut [f32; 256],
    ) {
        let width = self.settings.block_width as usize;
        let height = self.settings.block_height as usize;

        let rgba: &[u32] = cast_slice(rgba_data);

        for y in 0..height {
            for x in 0..width {
                let src_idx = ((yy * height + y) * stride + (xx * width + x) * 4) / 4;
                let rgba_val = rgba[src_idx];

                pixels[y * STRIDE + x] = ((rgba_val >> 0) & 0xFF) as f32; // R
                pixels[PITCH + y * STRIDE + x] = ((rgba_val >> 8) & 0xFF) as f32; // G
                pixels[2 * PITCH + y * STRIDE + x] = ((rgba_val >> 16) & 0xFF) as f32; // B
                pixels[3 * PITCH + y * STRIDE + x] = ((rgba_val >> 24) & 0xFF) as f32;
                // A
            }
        }
    }

    fn clear_alpha(&self, pixels: &mut [f32; 256]) {
        let width = self.settings.block_width as usize;
        let height = self.settings.block_height as usize;

        for y in 0..height {
            for x in 0..width {
                pixels[3 * 64 + y * 8 + x] = 255.0;
            }
        }
    }

    fn compute_pca_endpoints(ep: &mut [f32; 8], block: &PixelSet, zero_based: bool, channels: i32) {
        let mut dc = [0.0f32; 4];
        let mut cov = [0.0f32; 10];

        block.compute_covar_dc(&mut cov, &mut dc, zero_based, channels);

        const POWER_ITERATIONS: i32 = 10;

        // TODO: Try to use f32::EPSILON later
        let eps = 0.001f32.powi(2) * 1000.0;
        cov[0] += eps;
        cov[4] += eps;
        cov[7] += eps;
        cov[9] += eps;

        let mut dir = [0.0f32; 4];
        PixelSet::compute_axis(&mut dir, &cov, POWER_ITERATIONS, channels);

        // TODO: Try to use f32::INFINITY later
        let mut ext = [1000.0f32, -1000.0f32];

        for y in 0..block.block_height as usize {
            for x in 0..block.block_width as usize {
                let mut proj = 0.0f32;
                for p in 0..channels as usize {
                    proj += (get_pixel(&block.pixels, p, x, y) - dc[p]) * dir[p];
                }

                ext[0] = ext[0].min(proj);
                ext[1] = ext[1].max(proj);
            }
        }

        if ext[1] - 1.0 < ext[0] {
            ext[1] += 0.5;
            ext[0] -= 0.5;
        }

        for i in 0..2 {
            for p in 0..channels {
                ep[p as usize * 2 + i] = dc[p as usize] + dir[p as usize] * ext[i];
            }
        }
    }

    fn compute_metrics(&mut self, pixels: &[f32; 256]) {
        // The ISPC code did copy the pixel by hand, but we can just use copy
        // here instead, since the operations are byte for byte the same.
        let mut pset = PixelSet {
            pixels: *pixels,
            block_width: self.settings.block_width as i32,
            block_height: self.settings.block_height as i32,
        };

        for i in 0..2 {
            let zero_based = i == 1;
            let mut endpoints = [0.0f32; 8];
            Self::compute_pca_endpoints(&mut endpoints, &pset, zero_based, 4);

            let mut base = [0.0f32; 4];
            let mut dir = [0.0f32; 4];
            for p in 0..4 {
                dir[p] = endpoints[p * 2 + 1] - endpoints[p * 2];
                base[p] = endpoints[p * 2];
            }
            let sq_norm = dot4(&dir, &dir) + 0.00001;

            let mut pca_error = 0.0;
            let mut alpha_error = 0.0;
            let mut pca_alpha_error = 0.0;

            for y in 0..self.settings.block_height as usize {
                for x in 0..self.settings.block_width as usize {
                    let mut pixel = [0.0f32; 4];
                    for p in 0..4 {
                        pixel[p] = get_pixel(&pset.pixels, p, x, y) - base[p];
                    }
                    let proj = dot4(&pixel, &dir) / sq_norm;

                    for p in 0..3 {
                        pca_error +=
                            sq(get_pixel(&pset.pixels, p, x, y) - (proj * dir[p] + base[p]));
                    }
                    pca_alpha_error +=
                        sq(get_pixel(&pset.pixels, 3, x, y) - (proj * dir[3] + base[3]));
                    alpha_error += sq(get_pixel(&pset.pixels, 3, x, y) - 255.0);
                }
            }

            self.pca_error[i][0] = pca_error + pca_alpha_error;
            self.alpha_error[i][0] = alpha_error - pca_alpha_error;
            self.sq_norm[i][0] = sq_norm;
        }

        for i in 0..2 {
            for c in 1..5usize {
                pset.rotate_plane(c - 1);

                let zero_based = i == 1;
                let mut endpoints = [0.0f32; 8];
                Self::compute_pca_endpoints(&mut endpoints, &pset, zero_based, 3);

                let mut base = [0.0f32; 3];
                let mut dir = [0.0f32; 3];
                for p in 0..3 {
                    dir[p] = endpoints[p * 2 + 1] - endpoints[p * 2];
                    base[p] = endpoints[p * 2];
                }
                let sq_norm = dot3(&dir, &dir) + 0.00001;

                let mut pca_error = 0.0;
                let mut alpha_error = 0.0;
                let mut pca_alpha_error = 0.0;
                let mut ext = [1000.0f32, -1000.0f32];

                for y in 0..self.settings.block_height as usize {
                    for x in 0..self.settings.block_width as usize {
                        let mut pixel = [0.0f32; 3];
                        for p in 0..3 {
                            pixel[p] = get_pixel(&pset.pixels, p, x, y) - base[p];
                        }
                        let proj = dot3(&pixel, &dir) / sq_norm;

                        for p in 0..3 {
                            if p == c - 1 {
                                pca_alpha_error +=
                                    sq(get_pixel(&pset.pixels, p, x, y)
                                        - (proj * dir[p] + base[p]));
                                alpha_error += sq(get_pixel(&pset.pixels, p, x, y) - 255.0);
                            } else {
                                pca_error +=
                                    sq(get_pixel(&pset.pixels, p, x, y)
                                        - (proj * dir[p] + base[p]));
                            }
                        }

                        let value = get_pixel(&pset.pixels, 3, x, y);
                        ext[0] = ext[0].min(value);
                        ext[1] = ext[1].max(value);
                    }
                }

                self.pca_error[i][c] = pca_error + pca_alpha_error;
                self.alpha_error[i][c] = alpha_error - pca_alpha_error;
                self.sq_norm[i][c] = sq_norm + sq(ext[1] - ext[0]);

                // Rotate back
                pset.rotate_plane(c - 1);
            }
        }

        pset.compute_dct_inplace(4);

        for h in 2..=self.settings.block_height as usize {
            for w in 2..=self.settings.block_width as usize {
                let mut sq_sum = 0.0;

                for y in 0..self.settings.block_height as usize {
                    for x in 0..self.settings.block_width as usize {
                        if y < h && x < w {
                            continue;
                        }

                        for p in 0..4 {
                            sq_sum += sq(pset.pixels[PITCH * p + STRIDE * y + x]);
                        }
                    }
                }

                self.scale_error[h - 2][w - 2] = sq_sum;
            }
        }
    }

    fn get_sq_rcp_levels(range: i32) -> f32 {
        static TABLE: &[f32; 21] = &[
            1.000000, 0.250000, 0.111111, 0.062500, 0.040000, 0.020408, 0.012346, 0.008264,
            0.004444, 0.002770, 0.001890, 0.001041, 0.000657, 0.000453, 0.000252, 0.000160,
            0.000111, 0.000062, 0.000040, 0.000027, 0.000015,
        ];

        TABLE[range as usize]
    }

    fn estimate_error(&self, block: &AstcBlock) -> f32 {
        let c = if block.dual_plane != 0 {
            1 + block.color_component_selector as usize
        } else {
            0
        };

        let scale_error = self.scale_error[(block.height - 2) as usize][(block.width - 2) as usize];

        let zero_based = (block.color_endpoint_modes[0] % 4) == 2;
        let zero_idx = zero_based as usize;

        let mut pca_error = self.pca_error[zero_idx][c];
        let sq_norm = self.sq_norm[zero_idx][c];

        if block.color_endpoint_modes[0] <= 8 {
            pca_error += self.alpha_error[zero_idx][c];
        }

        let sq_rcp_w_levels = Self::get_sq_rcp_levels(block.weight_range);
        let sq_rcp_ep_levels = Self::get_sq_rcp_levels(block.endpoint_range);

        let mut quant_error = 0.0;
        quant_error += 2.0 * sq_norm * sq_rcp_w_levels;
        quant_error += 9000.0
            * (self.settings.block_height * self.settings.block_width) as f32
            * sq_rcp_ep_levels;

        scale_error + pca_error + quant_error
    }

    fn insert_element(&mut self, mut error: f32, mut packed_mode: u32, threshold_error: &mut f32) {
        let mut max_error = 0.0f32;

        for k in 0..self.settings.fast_skip_threshold as usize {
            if self.best_scores[k] > error {
                std::mem::swap(&mut self.best_scores[k], &mut error);
                std::mem::swap(&mut self.best_modes[k], &mut packed_mode);
            }

            max_error = max_error.max(self.best_scores[k]);
        }

        *threshold_error = max_error;
    }

    pub(crate) fn rank(
        &mut self,
        rgba_data: &[u8],
        xx: usize,
        yy: usize,
        stride: usize,
        mode_buffer: &mut [u32; ASTC_MAX_RANKED_MODES as usize],
        pixels: &mut [f32; 256],
    ) {
        self.load_block_interleaved(rgba_data, xx, yy, stride, pixels);

        if self.settings.channels == 3 {
            self.clear_alpha(pixels);
        }

        self.compute_metrics(pixels);

        let mut threshold_error = 0.0f32;
        let mut count = -1;

        for &packed_mode in PACKED_MODES.iter() {
            // TODO: NHA The original code did use another "astc_mode" struct and not a block. Important?!
            let block = AstcBlock::from_mode_parameters(packed_mode, self.settings.channels);

            if block.height > self.settings.block_height as i32
                || block.width > self.settings.block_width as i32
                || (self.settings.channels == 3 && block.color_endpoint_modes[0] > 8)
            {
                continue;
            }

            let error = self.estimate_error(&block);
            count += 1;

            if count < self.settings.fast_skip_threshold as i32 {
                self.best_modes[count as usize] = packed_mode;
                self.best_scores[count as usize] = error;
                threshold_error = f32::max(threshold_error, error);
            } else if error < threshold_error {
                self.insert_element(error, packed_mode, &mut threshold_error);
            }
        }

        debug_assert!(count >= 0);

        mode_buffer[..(self.settings.fast_skip_threshold as usize)]
            .copy_from_slice(&self.best_modes[..(self.settings.fast_skip_threshold as usize)]);
    }
}

pub(crate) struct BlockCompressorASTC<'a> {
    width: u32,
    height: u32,
    dual_plane: u32,
    partitions: u32,
    color_endpoint_pairs: u32,
    channels: u32,
    settings: &'a ASTCSettings,
}

impl<'a> BlockCompressorASTC<'a> {
    pub(crate) fn new(mode: u32, settings: &'a ASTCSettings) -> Self {
        let width = 2 + get_field(mode, 15, 13); // 2..8
        let height = 2 + get_field(mode, 18, 16); // 2..8
        let dual_plane = get_field(mode, 19, 19); // 0 or 1
        let color_endpoint_modes0 = get_field(mode, 7, 6) * 2 + 6; // 6, 8, 10 or 12
        let color_endpoint_pairs = 1 + (color_endpoint_modes0 / 4);
        let channels = if color_endpoint_modes0 > 8 { 4 } else { 3 };

        Self {
            width,
            height,
            dual_plane,
            partitions: 1,
            color_endpoint_pairs,
            channels,
            settings,
        }
    }

    pub(crate) fn compress(
        &self,
        rgba_data: &[u8],
        blocks_buffer: &mut [u8],
        xx: usize,
        yy: usize,
        stride: usize,
        best_score: &mut f32,
        pixels: &[f32; 256],
    ) {
        todo!()
    }
}
