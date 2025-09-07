use bytemuck::cast_slice_mut;

use crate::ASTCSettings;

pub(crate) const ASTC_MAX_RANKED_MODES: u32 = 64;

const RANGE_TABLE: [[i32; 3]; 21] = [
    //2^ 3^ 5^
    [1, 0, 0], // 0..1
    [0, 1, 0], // 0..2
    [2, 0, 0], // 0..3
    [0, 0, 1], // 0..4
    [1, 1, 0], // 0..5
    [3, 0, 0], // 0..7
    [1, 0, 1], // 0..9
    [2, 1, 0], // 0..11
    [4, 0, 0], // 0..15
    [2, 0, 1], // 0..19
    [3, 1, 0], // 0..23
    [5, 0, 0], // 0..31
    [3, 0, 1], // 0..39
    [4, 1, 0], // 0..47
    [6, 0, 0], // 0..63
    [4, 0, 1], // 0..79
    [5, 1, 0], // 0..95
    [7, 0, 0], // 0..127
    [5, 0, 1], // 0..159
    [6, 1, 0], // 0..191
    [8, 0, 0], // 0..255
];

fn get_bit(input: u32, a: i32) -> u32 {
    get_field(input, a, a)
}

fn get_field(input: u32, a: i32, b: i32) -> u32 {
    debug_assert!(a >= b);
    (input >> b) & ((1 << (a - b + 1)) - 1)
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
    pixels: [f32; 256],

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
            pixels: [0.0; 256],
            pca_error: [[0.0; 5]; 2],
            alpha_error: [[0.0; 5]; 2],
            sq_norm: [[0.0; 5]; 2],
            scale_error: [[0.0; 7]; 7],
            best_scores: [0.0; 64],
            best_modes: [0; 64],
            settings,
        }
    }

    pub(crate) fn rank(
        &self,
        rgba_data: &[u8],
        xx: usize,
        yy: usize,
        mode_buffer: &mut [u32; ASTC_MAX_RANKED_MODES as usize],
    ) {
        todo!()
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
    ) {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pack_block_4x4_0() {
        const INPUT_BLOCK: AstcBlock = AstcBlock {
            width: 4,
            height: 4,
            dual_plane: 0,
            weight_range: 8,
            weights: [
                1, 8, 14, 13, 3, 9, 9, 8, 0, 7, 7, 2, 1, 2, 5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 3,
            channels: 0,
            color_endpoint_modes: [8, 0, 0, 0],
            endpoint_range: 19,
            endpoints: [
                115, 107, 48, 178, 32, 96, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        };
        const EXPECTED_OUTPUT: [u32; 4] = [0xD6E70242, 0x3020B260, 0xEE484AA, 0x817BC991];

        let mut data = [0; 4];
        INPUT_BLOCK.pack(&mut data);

        assert_eq!(data, EXPECTED_OUTPUT)
    }

    #[test]
    fn pack_block_4x4_1() {
        const INPUT_BLOCK: AstcBlock = AstcBlock {
            width: 4,
            height: 4,
            dual_plane: 0,
            weight_range: 7,
            weights: [
                7, 9, 10, 4, 1, 5, 3, 8, 1, 1, 11, 10, 5, 5, 5, 7, 4, 3, 2, 3, 4, 1, 3, 1, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 64,
                0, 0, 0, 21, 0, 0, 0, 21, 0, 0, 0,
            ],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 3,
            channels: 21,
            color_endpoint_modes: [8, 21, 0, 21],
            endpoint_range: 20,
            endpoints: [
                226, 202, 117, 106, 58, 54, 127, 127, 0, 0, 0, 0, 21, 0, 0, 0, 21, 0,
            ],
        };

        const EXPECTED_OUTPUT: [u32; 4] = [0x95C50251, 0x6C74D4EB, 0x4D5B5780, 0xEB452F84];

        let mut data = [0; 4];
        INPUT_BLOCK.pack(&mut data);

        assert_eq!(data, EXPECTED_OUTPUT)
    }

    #[test]
    fn pack_block_4x4_2() {
        const INPUT_BLOCK: AstcBlock = AstcBlock {
            width: 3,
            height: 3,
            dual_plane: 1,
            weight_range: 5,
            weights: [
                4, 4, 0, 1, 0, 0, 7, 5, 3, 7, 4, 6, 7, 5, 2, 2, 3, 5, 2, 4, 2, 4, 1, 2, 16, 0, 0,
                0, 16, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 21, 0, 0, 0, 64,
                0, 0, 0, 21, 0, 0, 0, 21, 0, 0, 0,
            ],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 3,
            channels: 21,
            color_endpoint_modes: [8, 21, 0, 21],
            endpoint_range: 20,
            endpoints: [
                239, 222, 121, 115, 57, 55, 255, 0, 0, 0, 0, 0, 21, 0, 0, 0, 21, 0,
            ],
        };

        const EXPECTED_OUTPUT: [u32; 4] = [0xBDDF05BF, 0x6E72E6F3, 0xBF52D400, 0x24403DDC];

        let mut data = [0; 4];
        INPUT_BLOCK.pack(&mut data);

        assert_eq!(data, EXPECTED_OUTPUT)
    }

    #[test]
    fn pack_block_4x4_3() {
        const INPUT_BLOCK: AstcBlock = AstcBlock {
            width: 3,
            height: 4,
            dual_plane: 1,
            weight_range: 4,
            weights: [
                1, 1, 3, 5, 3, 5, 3, 1, 5, 3, 5, 3, 3, 1, 4, 4, 4, 1, 5, 5, 0, 0, 0, 4, 38, 0, 0,
                0, 0, 0, 0, 0, 26, 0, 0, 0, 38, 0, 0, 0, 26, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 3,
            channels: 0,
            color_endpoint_modes: [8, 0, 0, 0],
            endpoint_range: 19,
            endpoints: [
                147, 155, 186, 118, 160, 28, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        };

        const EXPECTED_OUTPUT: [u32; 4] = [0xB72705CF, 0xE60F675, 0xF85F6002, 0x93BDD5EE];

        let mut data = [0; 4];
        INPUT_BLOCK.pack(&mut data);

        assert_eq!(data, EXPECTED_OUTPUT)
    }

    #[test]
    fn pack_block_4x4_4() {
        const INPUT_BLOCK: AstcBlock = AstcBlock {
            width: 4,
            height: 4,
            dual_plane: 0,
            weight_range: 6,
            weights: [
                0, 8, 2, 6, 9, 9, 9, 4, 1, 7, 8, 8, 8, 7, 9, 3, 5, 0, 4, 4, 4, 1, 4, 1, 26, 0, 0,
                0, 64, 0, 0, 0, 0, 0, 0, 0, 51, 0, 0, 0, 38, 0, 0, 0, 64, 0, 0, 0, 64, 0, 0, 0, 64,
                0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0,
            ],
            color_component_selector: 0,
            partitions: 1,
            partition_id: 0,
            color_endpoint_pairs: 3,
            channels: 64,
            color_endpoint_modes: [8, 0, 43, 21],
            endpoint_range: 20,
            endpoints: [
                148, 157, 90, 92, 59, 58, 191, 191, 43, 0, 0, 0, 64, 0, 0, 0, 21, 0,
            ],
        };

        const EXPECTED_OUTPUT: [u32; 4] = [0x3B290241, 0x7476B8B5, 0xDA3FB000, 0x509FE933];

        let mut data = [0; 4];
        INPUT_BLOCK.pack(&mut data);

        assert_eq!(data, EXPECTED_OUTPUT)
    }
}
