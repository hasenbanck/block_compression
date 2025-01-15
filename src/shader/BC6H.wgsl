// Copyright (c) 2025, Nils Hasenbanck
// Copyright (c) 2016-2024, Intel Corporation
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of
// the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

struct Uniforms {
    width: u32,
    height: u32,
    blocks_offset: u32,
}

struct Settings {
    slow_mode: u32,
    fast_mode: u32,
    refine_iterations_1p: i32,
    refine_iterations_2p: i32,
    fast_skip_threshold: i32,
}

struct State {
    data: array<u32, 5>,
    best_err: f32,

    rgb_bounds: array<f32, 6>,
    max_span: f32,
    max_span_idx: u32,

    mode: u32,
    epb: u32,
    qbounds: array<i32, 8>,
}

// TODO: NHA I don't know if this will work with the decoding logic, which seems to work on the raw F16 values!
@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> block_buffer: array<u32>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;
@group(0) @binding(3) var<storage, read> settings: Settings;

fn get_skips(part_id: i32) -> vec3<u32> {
    const skip_table = array<u32, 128>(
        0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u,
        0xf0u, 0x20u, 0x80u, 0x20u, 0x20u, 0x80u, 0x80u, 0xf0u, 0x20u, 0x80u, 0x20u, 0x20u, 0x80u, 0x80u, 0x20u, 0x20u,
        0xf0u, 0xf0u, 0x60u, 0x80u, 0x20u, 0x80u, 0xf0u, 0xf0u, 0x20u, 0x80u, 0x20u, 0x20u, 0x20u, 0xf0u, 0xf0u, 0x60u,
        0x60u, 0x20u, 0x60u, 0x80u, 0xf0u, 0xf0u, 0x20u, 0x20u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0xf0u, 0x20u, 0x20u, 0xf0u,
        0x3fu, 0x38u, 0xf8u, 0xf3u, 0x8fu, 0x3fu, 0xf3u, 0xf8u, 0x8fu, 0x8fu, 0x6fu, 0x6fu, 0x6fu, 0x5fu, 0x3fu, 0x38u,
        0x3fu, 0x38u, 0x8fu, 0xf3u, 0x3fu, 0x38u, 0x6fu, 0xa8u, 0x53u, 0x8fu, 0x86u, 0x6au, 0x8fu, 0x5fu, 0xfau, 0xf8u,
		0x8fu, 0xf3u, 0x3fu, 0x5au, 0x6au, 0xa8u, 0x89u, 0xfau, 0xf6u, 0x3fu, 0xf8u, 0x5fu, 0xf3u, 0xf6u, 0xf6u, 0xf8u,
        0x3fu, 0xf3u, 0x5fu, 0x5fu, 0x5fu, 0x8fu, 0x5fu, 0xafu, 0x5fu, 0xafu, 0x8fu, 0xdfu, 0xf3u, 0xcfu, 0x3fu, 0x38u,
    );

    let skip_packed = skip_table[part_id];

    return vec3<u32>(0u, skip_packed >> 4u, skip_packed & 15u);
}

fn put_bits(state: ptr<function, State>, pos: ptr<function, u32>, bits: u32, v: u32) {
    (*state).data[(*pos) / 32u] |= v << ((*pos) % 32u);
    if ((*pos) % 32u + bits > 32u) {
        (*state).data[(*pos) / 32u + 1u] |= v >> (32u - (*pos) % 32u);
    }
    *pos += bits;
}

fn data_shl_1bit_from(state: ptr<function, State>, from_bits: u32) {
    if (from_bits < 96u) {
        let shifted = ((*state).data[2] >> 1u) | ((*state).data[3] << 31u);
        let mask = ((1u << (from_bits - 64u)) - 1u) >> 1u;
        (*state).data[2] = (mask & (*state).data[2]) | (~mask & shifted);
        (*state).data[3] = ((*state).data[3] >> 1u) | ((*state).data[4] << 31u);
        (*state).data[4] = (*state).data[4] >> 1u;
    } else if (from_bits < 128u) {
        let shifted = ((*state).data[3] >> 1u) | ((*state).data[4] << 31u);
        let mask = ((1u << (from_bits - 96u)) - 1u) >> 1u;
        (*state).data[3] = (mask & (*state).data[3]) | (~mask & shifted);
        (*state).data[4] = (*state).data[4] >> 1u;
    }
}

fn bc7_code_qblock(state: ptr<function, State>, qpos: ptr<function, u32>, qblock: vec2<u32>, bits: u32, flips: u32) {
    let levels = 1u << bits;
    var flips_shifted = flips;

    for (var k1 = 0u; k1 < 2u; k1++) {
        var qbits_shifted = qblock[k1];
        for (var k2 = 0u; k2 < 8u; k2++) {
            var q = qbits_shifted & 15u;
            if ((flips_shifted & 1u) > 0u) {
                q = (levels - 1u) - q;
            }

            if (k1 == 0u && k2 == 0u) {
                put_bits(state, qpos, bits - 1u, q);
            } else {
                put_bits(state, qpos, bits, q);
            }
            qbits_shifted >>= 4u;
            flips_shifted >>= 1u;
        }
    }
}

fn bc7_code_apply_swap_mode456(qep: ptr<function, array<i32, 24>>, channels: u32, qblock: ptr<function, vec2<u32>>, bits: u32) {
    let levels = 1u << bits;

    if (((*qblock)[0] & 15u) >= levels / 2u) {
        for (var p = 0u; p < channels; p++) {
            let temp = (*qep)[p];
            (*qep)[p] = (*qep)[channels + p];
            (*qep)[channels + p] = temp;
        }

        for (var k = 0u; k < 2u; k++) {
            (*qblock)[k] = (0x11111111u * (levels - 1u)) - (*qblock)[k];
        }
    }
}

fn bc7_code_adjust_skip_mode01237(state: ptr<function, State>, mode: u32, part_id: i32) {
    let pairs = select(2u, 3u, mode == 0u || mode == 2u);
    let bits = select(2u, 3u, mode == 0u || mode == 1u);

    var skips = get_skips(part_id);

    if (pairs > 2u && skips[1] < skips[2]) {
        let t = skips[1];
        skips[1] = skips[2];
        skips[2] = t;
    }

    for (var j = 1u; j < pairs; j++) {
        let k = skips[j];
        data_shl_1bit_from(state, 128u + (pairs - 1u) - (15u - k) * bits);
    }
}

@compute
@workgroup_size(8, 8)
fn compress_bc6h(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;

    var state: State;

    // TODO: NHA implement BC6H
}
