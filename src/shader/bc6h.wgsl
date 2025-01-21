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
    refine_iterations_1p: u32,
    refine_iterations_2p: u32,
    fast_skip_threshold: u32,
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

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> block_buffer: array<u32>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;
@group(0) @binding(3) var<storage, read> settings: Settings;

fn sq(x: f32) -> f32 {
    return x * x;
}

fn rsqrt(x: f32) -> f32 {
    return 1.0 / sqrt(x);
}

fn f32_to_f16_bits(f: f32) -> u32 {
    let u = bitcast<u32>(quantizeToF16(f));
    let sign = (u >> 31) & 0x1u;
    let exp = ((u >> 23) & 0xFFu) - 127u + 15u;
    let frac = (u >> 13) & 0x3FFu;
    return (sign << 15) | (exp << 10) | frac;
}

fn load_block_interleaved_16bit(block: ptr<function, array<f32, 64>>, xx: u32, yy: u32) {
    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            let pixel_x = xx * 4u + x;
            let pixel_y = yy * 4u + y;
            let rgba = textureLoad(source_texture, vec2<u32>(pixel_x, pixel_y), 0);

            (*block)[16u * 0u + y * 4u + x] = f32(f32_to_f16_bits(rgba.r) & 0xFFFF);
            (*block)[16u * 1u + y * 4u + x] = f32(f32_to_f16_bits(rgba.g) & 0xFFFF);
            (*block)[16u * 2u + y * 4u + x] = f32(f32_to_f16_bits(rgba.b) & 0xFFFF);
            (*block)[16u * 3u + y * 4u + x] = 0.0;
        }
    }
}

fn store_data(state: ptr<function, State>, block_width: u32, xx: u32, yy: u32) {
    let offset = uniforms.blocks_offset + (yy * block_width * 4u + xx * 4u);

    block_buffer[offset + 0] = (*state).data[0];
    block_buffer[offset + 1] = (*state).data[1];
    block_buffer[offset + 2] = (*state).data[2];
    block_buffer[offset + 3] = (*state).data[3];
}

fn get_unquant_value(bits: u32, index: i32) -> i32 {
    switch (bits) {
        case 2u: {
            const table = array<i32, 16>(0, 21, 43, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
            return table[index];
        }
        case 3u: {
            const table = array<i32, 16>(0, 9, 18, 27, 37, 46, 55, 64, 0, 0, 0, 0, 0, 0, 0, 0);
            return table[index];
        }
        default: {
            const table = array<i32, 16>(0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64);
            return table[index];
        }
    }
}

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

fn get_pattern(part_id: i32) -> u32 {
    const pattern_table = array<u32, 128>(
        0x50505050u, 0x40404040u, 0x54545454u, 0x54505040u, 0x50404000u, 0x55545450u, 0x55545040u, 0x54504000u,
		0x50400000u, 0x55555450u, 0x55544000u, 0x54400000u, 0x55555440u, 0x55550000u, 0x55555500u, 0x55000000u,
		0x55150100u, 0x00004054u, 0x15010000u, 0x00405054u, 0x00004050u, 0x15050100u, 0x05010000u, 0x40505054u,
		0x00404050u, 0x05010100u, 0x14141414u, 0x05141450u, 0x01155440u, 0x00555500u, 0x15014054u, 0x05414150u,
		0x44444444u, 0x55005500u, 0x11441144u, 0x05055050u, 0x05500550u, 0x11114444u, 0x41144114u, 0x44111144u,
		0x15055054u, 0x01055040u, 0x05041050u, 0x05455150u, 0x14414114u, 0x50050550u, 0x41411414u, 0x00141400u,
		0x00041504u, 0x00105410u, 0x10541000u, 0x04150400u, 0x50410514u, 0x41051450u, 0x05415014u, 0x14054150u,
		0x41050514u, 0x41505014u, 0x40011554u, 0x54150140u, 0x50505500u, 0x00555050u, 0x15151010u, 0x54540404u,
		0xAA685050u, 0x6A5A5040u, 0x5A5A4200u, 0x5450A0A8u, 0xA5A50000u, 0xA0A05050u, 0x5555A0A0u, 0x5A5A5050u,
		0xAA550000u, 0xAA555500u, 0xAAAA5500u, 0x90909090u, 0x94949494u, 0xA4A4A4A4u, 0xA9A59450u, 0x2A0A4250u,
		0xA5945040u, 0x0A425054u, 0xA5A5A500u, 0x55A0A0A0u, 0xA8A85454u, 0x6A6A4040u, 0xA4A45000u, 0x1A1A0500u,
		0x0050A4A4u, 0xAAA59090u, 0x14696914u, 0x69691400u, 0xA08585A0u, 0xAA821414u, 0x50A4A450u, 0x6A5A0200u,
		0xA9A58000u, 0x5090A0A8u, 0xA8A09050u, 0x24242424u, 0x00AA5500u, 0x24924924u, 0x24499224u, 0x50A50A50u,
		0x500AA550u, 0xAAAA4444u, 0x66660000u, 0xA5A0A5A0u, 0x50A050A0u, 0x69286928u, 0x44AAAA44u, 0x66666600u,
		0xAA444444u, 0x54A854A8u, 0x95809580u, 0x96969600u, 0xA85454A8u, 0x80959580u, 0xAA141414u, 0x96960000u,
		0xAAAA1414u, 0xA05050A0u, 0xA0A5A5A0u, 0x96000000u, 0x40804080u, 0xA9A8A9A8u, 0xAAAAAA44u, 0x2A4A5254u,
    );

    return pattern_table[part_id];
}

fn get_pattern_mask(part_id: i32, j: u32) -> u32 {
    const pattern_mask_table = array<u32, 128>(
		0xCCCC3333u, 0x88887777u, 0xEEEE1111u, 0xECC81337u, 0xC880377Fu, 0xFEEC0113u, 0xFEC80137u, 0xEC80137Fu,
		0xC80037FFu, 0xFFEC0013u, 0xFE80017Fu, 0xE80017FFu, 0xFFE80017u, 0xFF0000FFu, 0xFFF0000Fu, 0xF0000FFFu,
		0xF71008EFu, 0x008EFF71u, 0x71008EFFu, 0x08CEF731u, 0x008CFF73u, 0x73108CEFu, 0x3100CEFFu, 0x8CCE7331u,
		0x088CF773u, 0x3110CEEFu, 0x66669999u, 0x366CC993u, 0x17E8E817u, 0x0FF0F00Fu, 0x718E8E71u, 0x399CC663u,
		0xAAAA5555u, 0xF0F00F0Fu, 0x5A5AA5A5u, 0x33CCCC33u, 0x3C3CC3C3u, 0x55AAAA55u, 0x96966969u, 0xA55A5AA5u,
		0x73CE8C31u, 0x13C8EC37u, 0x324CCDB3u, 0x3BDCC423u, 0x69969669u, 0xC33C3CC3u, 0x99666699u, 0x0660F99Fu,
		0x0272FD8Du, 0x04E4FB1Bu, 0x4E40B1BFu, 0x2720D8DFu, 0xC93636C9u, 0x936C6C93u, 0x39C6C639u, 0x639C9C63u,
		0x93366CC9u, 0x9CC66339u, 0x817E7E81u, 0xE71818E7u, 0xCCF0330Fu, 0x0FCCF033u, 0x774488BBu, 0xEE2211DDu,
		0x08CC0133u, 0x8CC80037u, 0xCC80006Fu, 0xEC001331u, 0x330000FFu, 0x00CC3333u, 0xFF000033u, 0xCCCC0033u,
		0x0F0000FFu, 0x0FF0000Fu, 0x00F0000Fu, 0x44443333u, 0x66661111u, 0x22221111u, 0x136C0013u, 0x008C8C63u,
		0x36C80137u, 0x08CEC631u, 0x3330000Fu, 0xF0000333u, 0x00EE1111u, 0x88880077u, 0x22C0113Fu, 0x443088CFu,
		0x0C22F311u, 0x03440033u, 0x69969009u, 0x9960009Fu, 0x03303443u, 0x00660699u, 0xC22C3113u, 0x8C0000EFu,
		0x1300007Fu, 0xC4003331u, 0x004C1333u, 0x22229999u, 0x00F0F00Fu, 0x24929249u, 0x29429429u, 0xC30C30C3u,
		0xC03C3C03u, 0x00AA0055u, 0xAA0000FFu, 0x30300303u, 0xC0C03333u, 0x90900909u, 0xA00A5005u, 0xAAA0000Fu,
		0x0AAA0555u, 0xE0E01111u, 0x70700707u, 0x6660000Fu, 0x0EE01111u, 0x07707007u, 0x06660999u, 0x660000FFu,
		0x00660099u, 0x0CC03333u, 0x03303003u, 0x60000FFFu, 0x80807777u, 0x10100101u, 0x000A0005u, 0x08CE8421u,
    );

    let mask_packed = pattern_mask_table[part_id];
    let mask0 = mask_packed & 0xFFFFu;
    let mask1 = mask_packed >> 16u;

    return select(select(mask1, mask0, j == 0), ~mask0 & ~mask1, j == 2);
}

fn get_mode_prefix(mode: u32) -> u32 {
    const mode_prefix_table = array<u32, 14>(
        0u, 1u, 2u, 6u, 10u, 14u, 18u, 22u, 26u, 30u, 3u, 7u, 11u, 15u
    );
    return mode_prefix_table[mode];
}

fn get_span(mode: u32) -> f32 {
    const span_table = array<f32, 14>(
        0.9 * f32(0xFFFF) /  64.0,  // (0) 4 / 10
        0.9 * f32(0xFFFF) /   4.0,  // (1) 5 / 7
        0.8 * f32(0xFFFF) / 256.0,  // (2) 3 / 11
        -1.0, -1.0,
        0.9 * f32(0xFFFF) /  32.0,  // (5) 4 / 9
        0.9 * f32(0xFFFF) /  16.0,  // (6) 4 / 8
        -1.0, -1.0,
        f32(0xFFFF),                // (9) absolute
        f32(0xFFFF),                // (10) absolute
        0.95 * f32(0xFFFF) / 8.0,   // (11) 8 / 11
        0.95 * f32(0xFFFF) / 32.0,  // (12) 7 / 12
        6.0,                        // (13) 3 / 16
    );
    return span_table[mode];
}

fn get_mode_bits(mode: u32) -> u32 {
    const mode_bits_table = array<u32, 14>(
        10u, 7u, 11u, 0u, 0u,
        9u, 8u, 0u, 0u, 6u,
        10u, 11u, 12u, 16u,
    );
    return mode_bits_table[mode];
}

fn ep_quant_bc6h_8(state: ptr<function, State>, ep: ptr<function, array<f32, 8>>, bits: u32, pairs: u32) {
    let levels = 1u << bits;

    for (var i = 0u; i < 8u * pairs; i++) {
        let v = i32(((*ep)[i] / (256.0 * 256.0 - 1.0) * f32(levels - 1u) + 0.5));
        (*state).qbounds[i] = clamp(v, 0, i32(levels - 1u));
    }
}

fn compute_qbounds_core(state: ptr<function, State>, rgb_span: vec3<f32>) {
    var bounds: array<f32, 8>;

    for (var p = 0u; p < 3u; p++) {
        let middle = ((*state).rgb_bounds[p] + (*state).rgb_bounds[3u + p]) / 2.0;
        bounds[p] = middle - rgb_span[p] / 2.0;
        bounds[4u + p] = middle + rgb_span[p] / 2.0;
    }

    ep_quant_bc6h_8(state, &bounds, (*state).epb, 1);
}

fn compute_qbounds(state: ptr<function, State>, span: f32) {
    compute_qbounds_core(state, vec3<f32>(span, span, span));
}

fn compute_qbounds2(state: ptr<function, State>, span: f32, max_span_idx: u32) {
    var rgb_span = vec3<f32>(span, span, span);
    if (max_span_idx < 3u) {
        rgb_span[max_span_idx] *= 2.0;
    }
    compute_qbounds_core(state, rgb_span);
}

fn partial_sort_list(list: ptr<function, array<i32, 32>>, length: u32, partial_count: u32) {
    for (var k = 0u; k < partial_count; k++) {
        var best_idx = i32(k);
        var best_value = (*list)[k];

        for (var i = k + 1u; i < length; i++) {
            if (best_value > (*list)[i]) {
                best_value = (*list)[i];
                best_idx = i32(i);
            }
        }

        let temp = (*list)[k];
        (*list)[k] = best_value;
        (*list)[best_idx] = temp;
    }
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

fn opt_endpoints(ep: ptr<function, array<f32, 24>>, offset: u32, block: ptr<function, array<f32, 64>>, bits: u32, qblock: vec2<u32>, mask: u32, channels: u32) {
    let levels = i32(1u << bits);

    var Atb1: vec4<f32>;
    var sum_q = 0.0;
    var sum_qq = 0.0;
    var sum: array<f32, 5>;

    var mask_shifted = mask << 1u;
    for (var k1 = 0u; k1 < 2u; k1++) {
        var qbits_shifted = qblock[k1];
        for (var k2 = 0u; k2 < 8u; k2++) {
            let k = k1 * 8u + k2;
            let q = f32(qbits_shifted & 15u);
            qbits_shifted >>= 4u;

            mask_shifted >>= 1u;
            if ((mask_shifted & 1u) == 0u) {
                continue;
            }

            let x = f32(levels - 1) - q;
            let y = q;

            sum_q += q;
            sum_qq += q * q;

            sum[4] += 1.0;
            for (var p = 0u; p < channels; p++) {
                sum[p] += (*block)[k + p * 16u];
                Atb1[p] += x * (*block)[k + p * 16u];
            }
        }
    }

    var Atb2: vec4<f32>;
    for (var p = 0u; p < channels; p++) {
        Atb2[p] = f32(levels - 1) * sum[p] - Atb1[p];
    }

    let Cxx = sum[4] * sq(f32(levels - 1)) - 2.0 * f32(levels - 1) * sum_q + sum_qq;
    let Cyy = sum_qq;
    let Cxy = f32(levels - 1) * sum_q - sum_qq;
    let scale = f32(levels - 1) / (Cxx * Cyy - Cxy * Cxy);

    for (var p = 0u; p < channels; p++) {
        (*ep)[offset + 0u + p] = (Atb1[p] * Cyy - Atb2[p] * Cxy) * scale;
        (*ep)[offset + 4u + p] = (Atb2[p] * Cxx - Atb1[p] * Cxy) * scale;
    }

    if (abs(Cxx * Cyy - Cxy * Cxy) < 0.001) {
        // flatten
        for (var p = 0u; p < channels; p++) {
            (*ep)[offset + 0u + p] = sum[p] / sum[4];
            (*ep)[offset + 4u + p] = (*ep)[offset + 0u + p];
        }
    }
}

// Principal Component Analysis (PCA) bound
fn get_pca_bound(covar: array<f32, 10>, channels: u32) -> f32 {
    const power_iterations = 4u; // Quite approximative, but enough for bounding

    var covar_scaled = covar;
    let inv_var = 1.0 / (256.0 * 256.0);
    for (var k = 0u; k < 10u; k++) {
        covar_scaled[k] *= inv_var;
    }

    let eps = sq(0.001);
    covar_scaled[0] += eps;
    covar_scaled[4] += eps;
    covar_scaled[7] += eps;

    var axis: vec4<f32>;
    compute_axis(&axis, &covar_scaled, power_iterations, channels);

    var a_vec: vec4<f32>;
    if (channels == 3u) {
        ssymv3(&a_vec, &covar_scaled, &axis);
    } else if (channels == 4u) {
        ssymv4(&a_vec, &covar_scaled, &axis);
    }

    var sq_sum = 0.0;
    for (var p = 0u; p < channels; p++) {
        sq_sum += sq(a_vec[p]);
    }
    let lambda = sqrt(sq_sum);

    var bound = covar_scaled[0] + covar_scaled[4] + covar_scaled[7];
    if (channels == 4u) {
        bound += covar_scaled[9];
    }
    bound -= lambda;
    bound = max(bound, 0.0);

    return bound;
}

fn ssymv3(a: ptr<function, vec4<f32>>, covar: ptr<function, array<f32, 10>>, b: ptr<function, vec4<f32>>) {
    (*a)[0] = (*covar)[0] * (*b)[0] + (*covar)[1] * (*b)[1] + (*covar)[2] * (*b)[2];
    (*a)[1] = (*covar)[1] * (*b)[0] + (*covar)[4] * (*b)[1] + (*covar)[5] * (*b)[2];
    (*a)[2] = (*covar)[2] * (*b)[0] + (*covar)[5] * (*b)[1] + (*covar)[7] * (*b)[2];
}

fn ssymv4(a: ptr<function, vec4<f32>>, covar: ptr<function, array<f32, 10>>, b: ptr<function, vec4<f32>>) {
    (*a)[0] = (*covar)[0] * (*b)[0] + (*covar)[1] * (*b)[1] + (*covar)[2] * (*b)[2] + (*covar)[3] * (*b)[3];
    (*a)[1] = (*covar)[1] * (*b)[0] + (*covar)[4] * (*b)[1] + (*covar)[5] * (*b)[2] + (*covar)[6] * (*b)[3];
    (*a)[2] = (*covar)[2] * (*b)[0] + (*covar)[5] * (*b)[1] + (*covar)[7] * (*b)[2] + (*covar)[8] * (*b)[3];
    (*a)[3] = (*covar)[3] * (*b)[0] + (*covar)[6] * (*b)[1] + (*covar)[8] * (*b)[2] + (*covar)[9] * (*b)[3];
}

fn compute_axis(axis: ptr<function, vec4<f32>>, covar: ptr<function, array<f32, 10>>, power_iterations: u32, channels: u32) {
    var a_vec = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    for (var i = 0u; i < power_iterations; i++) {
        if (channels == 3u) {
            ssymv3(axis, covar, &a_vec);
        } else if (channels == 4u) {
            ssymv4(axis, covar, &a_vec);
        }

        for (var p = 0u; p < channels; p++) {
            a_vec[p] = (*axis)[p];
        }

        // Renormalize every other iteration
        if (i % 2u == 1u) {
            var norm_sq = 0.0;
            for (var p = 0u; p < channels; p++) {
                norm_sq += (*axis)[p] * (*axis)[p];
            }

            let rnorm = rsqrt(norm_sq);
            for (var p = 0u; p < channels; p++) {
                a_vec[p] *= rnorm;
            }
        }
    }

    for (var p = 0u; p < channels; p++) {
        (*axis)[p] = a_vec[p];
    }
}

fn compute_stats_masked(stats: ptr<function, array<f32, 15>>, block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) {
    var mask_shifted = mask << 1u;
    for (var k = 0u; k < 16u; k++) {
        mask_shifted = mask_shifted >> 1u;
        let flag = f32(mask_shifted & 1u);

        var rgba: vec4<f32>;
        for (var p = 0u; p < channels; p++) {
            rgba[p] = (*block)[k + p * 16u] * flag;
        }
        (*stats)[14] += flag;

        (*stats)[10] += rgba[0];
        (*stats)[11] += rgba[1];
        (*stats)[12] += rgba[2];

        (*stats)[0] += rgba[0] * rgba[0];
        (*stats)[1] += rgba[0] * rgba[1];
        (*stats)[2] += rgba[0] * rgba[2];

        (*stats)[4] += rgba[1] * rgba[1];
        (*stats)[5] += rgba[1] * rgba[2];

        (*stats)[7] += rgba[2] * rgba[2];

        if (channels == 4u) {
            (*stats)[13] += rgba[3];
            (*stats)[3] += rgba[0] * rgba[3];
            (*stats)[6] += rgba[1] * rgba[3];
            (*stats)[8] += rgba[2] * rgba[3];
            (*stats)[9] += rgba[3] * rgba[3];
        }
    }
}

fn covar_from_stats(covar: ptr<function, array<f32, 10>>, stats: array<f32, 15>, channels: u32) {
    (*covar)[0] = stats[0] - stats[10] * stats[10] / stats[14];
    (*covar)[1] = stats[1] - stats[10] * stats[11] / stats[14];
    (*covar)[2] = stats[2] - stats[10] * stats[12] / stats[14];

    (*covar)[4] = stats[4] - stats[11] * stats[11] / stats[14];
    (*covar)[5] = stats[5] - stats[11] * stats[12] / stats[14];

    (*covar)[7] = stats[7] - stats[12] * stats[12] / stats[14];

    if (channels == 4u) {
        (*covar)[3] = stats[3] - stats[10] * stats[13] / stats[14];
        (*covar)[6] = stats[6] - stats[11] * stats[13] / stats[14];
        (*covar)[8] = stats[8] - stats[12] * stats[13] / stats[14];
        (*covar)[9] = stats[9] - stats[13] * stats[13] / stats[14];
    }
}

fn compute_covar_dc_masked(covar: ptr<function, array<f32, 10>>, dc: ptr<function, vec4<f32>>, block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) {
    var stats: array<f32, 15>;
    compute_stats_masked(&stats, block, mask, channels);

    // Calculate dc values from stats
    for (var p = 0u; p < channels; p++) {
        (*dc)[p] = stats[10u + p] / stats[14];
    }

    covar_from_stats(covar, stats, channels);
}

fn block_pca_axis(axis: ptr<function, vec4<f32>>, dc: ptr<function, vec4<f32>>, block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) {
    const power_iterations = 8u; // 4 not enough for HQ

    var covar: array<f32, 10>;
    compute_covar_dc_masked(&covar, dc, block, mask, channels);

    const inv_var = 1.0 / (256.0 * 256.0);
    for (var k = 0u; k < 10u; k++) {
        covar[k] *= inv_var;
    }

    let eps = sq(0.001);
    covar[0] += eps;
    covar[4] += eps;
    covar[7] += eps;
    covar[9] += eps;

    compute_axis(axis, &covar, power_iterations, channels);
}

fn block_pca_bound(block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) -> f32 {
    var stats: array<f32, 15>;
    compute_stats_masked(&stats, block, mask, channels);

    var covar: array<f32, 10>;
    covar_from_stats(&covar, stats, channels);

    return get_pca_bound(covar, channels);
}

fn block_pca_bound_split(block: ptr<function, array<f32, 64>>, mask: u32, full_stats: array<f32, 15>, channels: u32) -> f32 {
    var stats: array<f32, 15>;
    compute_stats_masked(&stats, block, mask, channels);

    var covar1: array<f32, 10>;
    covar_from_stats(&covar1, stats, channels);

    for (var i = 0u; i < 15u; i++) {
        stats[i] = full_stats[i] - stats[i];
    }

    var covar2: array<f32, 10>;
    covar_from_stats(&covar2, stats, channels);

    var bound = 0.0;
    bound += get_pca_bound(covar1, channels);
    bound += get_pca_bound(covar2, channels);

    return sqrt(bound) * 256.0;
}

fn unpack_to_uf16(v: u32, bits: u32) -> u32 {
    if (bits >= 15u) {
        return v;
    }
    if (v == 0u) {
        return 0u;
    }
    if (v == (1u << bits) - 1u) {
        return 0xFFFFu;
    }

    return (v * 2u + 1u) << (15u - bits);
}

fn ep_quant_bc6h(qep: ptr<function, array<i32, 24>>, ep:  ptr<function, array<f32, 24>>, bits: u32, pairs: u32) {
    let levels = 1u << bits;

    for (var i = 0u; i < 8u * pairs; i++) {
        let v = i32(((*ep)[i] / (256.0 * 256.0 - 1.0) * f32(levels - 1u) + 0.5));
        (*qep)[i] = clamp(v, 0, i32(levels - 1u));
    }
}

fn ep_dequant_bc6h(ep: ptr<function, array<f32, 24>>, qep: ptr<function, array<i32, 24>>, bits: u32, pairs: u32) {
    for (var i = 0u; i < 8u * pairs; i++) {
        (*ep)[i] = f32(unpack_to_uf16(u32((*qep)[i]), bits));
    }
}

fn ep_quant_dequant_bc6h(state: ptr<function, State>, qep: ptr<function, array<i32, 24>>, ep: ptr<function, array<f32, 24>>, pairs: u32) {
    let bits = (*state).epb;
    ep_quant_bc6h(qep, ep, bits, pairs);

    for (var i = 0u; i < 2u * pairs; i++) {
        for (var p = 0u; p < 3u; p++) {
            (*qep)[i * 4u + p] = clamp((*qep)[i * 4u + p], (*state).qbounds[p], (*state).qbounds[4u + p]);
        }
    }

    ep_dequant_bc6h(ep, qep, bits, pairs);
}

fn block_quant(qblock: ptr<function, vec2<u32>>, block: ptr<function, array<f32, 64>>, bits: u32, ep: ptr<function, array<f32, 24>>, pattern: u32, channels: u32) -> f32 {
    var total_err = 0.0;
    let levels = 1u << bits;

    (*qblock)[0] = 0u;
    (*qblock)[1] = 0u;

    var pattern_shifted = pattern;
    for (var k = 0u; k < 16u; k++) {
        let j = pattern_shifted & 3u;
        pattern_shifted = pattern_shifted >> 2u;

        var proj = 0.0;
        var div = 0.0;
        for (var p = 0u; p < channels; p++) {
            let ep_a = (*ep)[8u * j + 0u + p];
            let ep_b = (*ep)[8u * j + 4u + p];
            proj += ((*block)[k + p * 16u] - ep_a) * (ep_b - ep_a);
            div += sq(ep_b - ep_a);
        }

        proj = proj / div;

        let q1 = i32(proj * f32(levels) + 0.5);
        let q1_clamped = clamp(q1, 1, i32(levels) - 1);

        var err0 = 0.0;
        var err1 = 0.0;
        let w0 = get_unquant_value(bits, q1_clamped - 1);
        let w1 = get_unquant_value(bits, q1_clamped);

        for (var p = 0u; p < channels; p++) {
            let ep_a = (*ep)[8u * j + 0u + p];
            let ep_b = (*ep)[8u * j + 4u + p];
            let dec_v0 = f32(i32(((64.0 - f32(w0)) * ep_a + f32(w0) * ep_b + 32.0) / 64.0));
            let dec_v1 = f32(i32(((64.0 - f32(w1)) * ep_a + f32(w1) * ep_b + 32.0) / 64.0));
            err0 += sq(dec_v0 - (*block)[k + p * 16u]);
            err1 += sq(dec_v1 - (*block)[k + p * 16u]);
        }

        var best_err = err1;
        var best_q = q1_clamped;
        if (err0 < err1) {
            best_err = err0;
            best_q = q1_clamped - 1;
        }

        (*qblock)[k / 8u] |= u32(best_q) << (4u * (k % 8u));
        total_err += best_err;
    }

    return total_err;
}

fn block_segment_core(ep: ptr<function, array<f32, 24>>, offset: u32, block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) {
    var axis: vec4<f32>;
    var dc: vec4<f32>;
    block_pca_axis(&axis, &dc, block, mask, channels);

    var ext = vec2<f32>(3.40282347e38, -3.40282347e38);

    // Find min/max
    var mask_shifted = mask << 1u;
    for (var k = 0u; k < 16u; k++) {
        mask_shifted = mask_shifted >> 1u;
        if ((mask_shifted & 1u) == 0u) {
            continue;
        }

        var dot = 0.0;
        for (var p = 0u; p < channels; p++) {
            dot += axis[p] * ((*block)[16u * p + k] - dc[p]);
        }

        ext[0] = min(ext[0], dot);
        ext[1] = max(ext[1], dot);
    }

    // Create some distance if the endpoints collapse
    if (ext[1] - ext[0] < 1.0) {
        ext[0] -= 0.5;
        ext[1] += 0.5;
    }

    for (var i = 0u; i < 2u; i++) {
        for (var p = 0u; p < channels; p++) {
            (*ep)[offset + 4u * i + p] = ext[i] * axis[p] + dc[p];
        }
    }
}

fn block_segment(ep: ptr<function, array<f32, 24>>, offset: u32, block: ptr<function, array<f32, 64>>, mask: u32, channels: u32) {
    block_segment_core(ep, offset, block, mask, channels);

    for (var i = 0u; i < 2u; i++) {
        for (var p = 0u; p < channels; p++) {
            (*ep)[offset + 4u * i + p] = clamp((*ep)[offset + 4u * i + p], 0.0, 255.0);
        }
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

fn bc7_code_apply_swap_mode01237(qep: ptr<function, array<i32, 24>>, qblock: vec2<u32>, mode: u32, part_id: i32) -> u32 {
    let bits = select(2u, 3u, mode == 0u || mode == 1u);
    let pairs = select(2u, 3u, mode == 0u || mode == 2u);

    var flips = 0u;
    let levels = 1u << bits;

    let skips = get_skips(part_id);

    for (var j = 0u; j < pairs; j++) {
        let k0 = skips[j];
        // Extract 4 bits from qblock at position k0
        let q = (qblock[k0 >> 3u] << (28u - (k0 & 7u) * 4u)) >> 28u;

        if (q >= levels / 2u) {
            for (var p = 0u; p < 4u; p++) {
                let temp = (*qep)[8u * j + p];
                (*qep)[8u * j + p] = (*qep)[8u * j + 4u + p];
                (*qep)[8u * j + 4u + p] = temp;
            }

            let pmask = get_pattern_mask(part_id, j);
            flips |= u32(pmask);
        }
    }

    return flips;
}

fn bc6h_code_2p(state: ptr<function, State>, qep: ptr<function, array<i32, 24>>, qblock: vec2<u32>, part_id: i32, mode: u32) {
    let bits = 3u;
    let pairs = 2u;
    let channels = 3u;

    let flips = bc7_code_apply_swap_mode01237(qep, qblock, 1u, part_id);

    for (var k = 0u; k < 5u; k++) {
        (*state).data[k] = 0u;
    }
    var pos = 0u;

    var packed: vec4<u32>;
    bc6h_pack(&packed, qep, mode);

    // Mode
    put_bits(state, &pos, 5u, packed[0]);

    // Endpoints
    put_bits(state, &pos, 30u, packed[1]);
    put_bits(state, &pos, 30u, packed[2]);
    put_bits(state, &pos, 12u, packed[3]);

    // Partition
    put_bits(state, &pos, 5u, u32(part_id));

    // Quantized values
    bc7_code_qblock(state, &pos, qblock, bits, flips);
    bc7_code_adjust_skip_mode01237(state, 1u, part_id);
}

fn bc6h_code_1p(state: ptr<function, State>, qep: ptr<function, array<i32, 24>>, qblock: ptr<function, vec2<u32>>, mode: u32) {
    bc7_code_apply_swap_mode456(qep, 4u, qblock, 4u);

    for (var k = 0u; k < 5u; k++) {
        (*state).data[k] = 0u;
    }
    var pos = 0u;

    var packed: vec4<u32>;
    bc6h_pack(&packed, qep, mode);

    // Mode
    put_bits(state, &pos, 5u, packed[0]);

    // Endpoints
    put_bits(state, &pos, 30u, packed[1]);
    put_bits(state, &pos, 30u, packed[2]);

    // Quantized values
    bc7_code_qblock(state, &pos, *qblock, 4u, 0u);
}

fn bc6h_enc_2p(state: ptr<function, State>, block: ptr<function, array<f32, 64>>) {
    var full_stats: array<f32, 15>;
    compute_stats_masked(&full_stats, block, 0xFFFFFFFFu, 3u);

    var part_list: array<i32, 32>;
    for (var part = 0u; part < 32u; part++) {
        let mask = get_pattern_mask(i32(part), 0u);
        let bound12 = block_pca_bound_split(block, mask, full_stats, 3u);
        let bound = i32(bound12);
        part_list[part] = i32(part) + bound * 64;
    }

    partial_sort_list(&part_list, 32, settings.fast_skip_threshold);
    bc6h_enc_2p_list(state, block, &part_list, settings.fast_skip_threshold);
}

fn bc6h_enc_2p_part_fast(state: ptr<function, State>, block: ptr<function, array<f32, 64>>, qep: ptr<function, array<i32, 24>>, qblock: ptr<function, vec2<u32>>, part_id: i32) -> f32 {
    let pattern = get_pattern(part_id);
    let bits = 3u;
    let pairs = 2u;
    let channels = 3u;

    var ep: array<f32, 24>;
    for (var j = 0u; j < pairs; j++) {
        let mask = get_pattern_mask(part_id, j);
        block_segment_core(&ep, j * 8u, block, mask, channels);
    }

    ep_quant_dequant_bc6h(state, qep, &ep, 2u);

    return block_quant(qblock, block, bits, &ep, pattern, channels);
}

fn bc6h_enc_2p_list(state: ptr<function, State>, block: ptr<function, array<f32, 64>>, part_list: ptr<function, array<i32, 32>>, part_count: u32) {
    if (part_count == 0u) {
        return;
    }

    let bits = 3u;
    let pairs = 2u;
    let channels = 3u;

    var best_qep: array<i32, 24>;
    var best_qblock: vec2<u32>;
    var best_part_id = -1;
    var best_err = 3.40282347e38;

    for (var part = 0u; part < part_count; part++) {
        let part_id = (*part_list)[part] & 31;

        var qep: array<i32, 24>;
        var qblock: vec2<u32>;
        let err = bc6h_enc_2p_part_fast(state, block, &qep, &qblock, part_id);

        if (err < best_err) {
            for (var i = 0u; i < 8u * pairs; i++) {
                best_qep[i] = qep[i];
            }
            for (var k = 0u; k < 2u; k++) {
                best_qblock[k] = qblock[k];
            }
            best_part_id = part_id;
            best_err = err;
        }
    }

    // Refine
    for (var i = 0u; i < settings.refine_iterations_2p; i++) {
        var ep: array<f32, 24>;
        for (var j = 0u; j < pairs; j++) {
            let mask = get_pattern_mask(best_part_id, j);
            opt_endpoints(&ep, j * 8u, block, bits, best_qblock, mask, channels);
        }

        var qep: array<i32, 24>;
        var qblock: vec2<u32>;
        ep_quant_dequant_bc6h(state, &qep, &ep, 2u);

        let pattern = get_pattern(best_part_id);
        let err = block_quant(&qblock, block, bits, &ep, pattern, channels);

        if (err < best_err) {
            for (var i = 0u; i < 8u * pairs; i++) {
                best_qep[i] = qep[i];
            }
            for (var k = 0u; k < 2u; k++) {
                best_qblock[k] = qblock[k];
            }
            best_err = err;
        }
    }

    if (best_err < (*state).best_err) {
        (*state).best_err = best_err;
        bc6h_code_2p(state, &best_qep, best_qblock, best_part_id, (*state).mode);
    }
}

fn bc6h_enc_1p(state: ptr<function, State>, block: ptr<function, array<f32, 64>>) {
    var ep: array<f32, 24>;
    block_segment_core(&ep, 0, block, 0xFFFFFFFFu, 3u);

    var qep: array<i32, 24>;
    ep_quant_dequant_bc6h(state, &qep, &ep, 1u);

    var qblock: vec2<u32>;
    var err = block_quant(&qblock, block, 4u, &ep, 0u, 3u);

    // Refine
    let refineIterations = settings.refine_iterations_1p;
    for (var i = 0u; i < refineIterations; i++) {
        opt_endpoints(&ep, 0, block, 4, qblock, 0xFFFFFFFFu, 3u);
        ep_quant_dequant_bc6h(state, &qep, &ep, 1u);
        err = block_quant(&qblock, block, 4u, &ep, 0u, 3u);
    }

    if (err < (*state).best_err) {
        (*state).best_err = err;
        bc6h_code_1p(state, &qep, &qblock, (*state).mode);
    }
}

fn bc6h_test_mode(state: ptr<function, State>, block: ptr<function, array<f32, 64>>, mode: u32, enc: bool, margin: f32) {
    let mode_bits = get_mode_bits(mode);
    let span = get_span(mode);
    let max_span = (*state).max_span;
    let max_span_idx = (*state).max_span_idx;

    if (max_span * margin > span) {
        return;
    }

    if (mode >= 10u) {
        (*state).epb = mode_bits;
        (*state).mode = mode;
        compute_qbounds(state, span);
        if (enc) {
            bc6h_enc_1p(state, block);
        }
    } else if (mode <= 1u || mode == 5u || mode == 9u) {
        (*state).epb = mode_bits;
        (*state).mode = mode;
        compute_qbounds(state, span);
        if (enc) {
            bc6h_enc_2p(state, block);
        }
    } else {
        (*state).epb = mode_bits;
        (*state).mode = mode + max_span_idx;
        compute_qbounds2(state, span, max_span_idx);
        if (enc) {
            bc6h_enc_2p(state, block);
        }
    }
}

fn bit_at(v: i32, pos: u32) -> u32 {
    return u32((v >> pos) & 1);
}

fn reverse_bits(v: u32, bits: u32) -> u32 {
    if bits == 2u {
        return (v >> 1u) + (v & 1u) * 2u;
    }

    if bits == 6u {
        var vv = (v & 0x5555u) * 2u + ((v >> 1u) & 0x5555u);
        return (vv >> 4u) + ((vv >> 2u) & 3u) * 4u + (vv & 3u) * 16u;
    }

    // Should never happen
    return 0u;
}

fn bc6h_pack(packed: ptr<function, vec4<u32>>, qep: ptr<function, array<i32, 24>>, mode: u32) {
    if mode == 0u {
        var pred_qep: array<i32, 16>;
        for (var p = 0u; p < 3u; p++) {
            pred_qep[p] = (*qep)[p];
            pred_qep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 31;
            pred_qep[8u + p] = ((*qep)[8u + p] - (*qep)[p]) & 31;
            pred_qep[12u + p] = ((*qep)[12u + p] - (*qep)[p]) & 31;
        }

        var pqep: array<u32, 10>;

        pqep[4] = u32(pred_qep[4]) + (u32(pred_qep[8 + 1] & 15) * 64u);
        pqep[5] = u32(pred_qep[5]) + (u32(pred_qep[12 + 1] & 15) * 64u);
        pqep[6] = u32(pred_qep[6]) + (u32(pred_qep[8 + 2] & 15) * 64u);

        pqep[4] += bit_at(pred_qep[12 + 1], 4u) << 5u;
        pqep[5] += bit_at(pred_qep[12 + 2], 0u) << 5u;
        pqep[6] += bit_at(pred_qep[12 + 2], 1u) << 5u;

        pqep[8] = u32(pred_qep[8]) + bit_at(pred_qep[12 + 2], 2u) * 32u;
        pqep[9] = u32(pred_qep[12]) + bit_at(pred_qep[12 + 2], 3u) * 32u;

        (*packed)[0] = get_mode_prefix(0);
        (*packed)[0] += bit_at(pred_qep[8 + 1], 4u) << 2u;
        (*packed)[0] += bit_at(pred_qep[8 + 2], 4u) << 3u;
        (*packed)[0] += bit_at(pred_qep[12 + 2], 4u) << 4u;

        (*packed)[1] = (u32(pred_qep[2]) << 20u) + (u32(pred_qep[1]) << 10u) + u32(pred_qep[0]);
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (pqep[9] << 6u) + pqep[8];
    }
    else if mode == 1u {
        var pred_qep: array<i32, 16>;
        for (var p = 0u; p < 3u; p++) {
            pred_qep[p] = (*qep)[p];
            pred_qep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 63;
            pred_qep[8u + p] = ((*qep)[8u + p] - (*qep)[p]) & 63;
            pred_qep[12u + p] = ((*qep)[12u + p] - (*qep)[p]) & 63;
        }

        var pqep: array<u32, 8>;

        pqep[0] = u32(pred_qep[0]);
        pqep[0] += bit_at(pred_qep[12 + 2], 0u) << 7u;
        pqep[0] += bit_at(pred_qep[12 + 2], 1u) << 8u;
        pqep[0] += bit_at(pred_qep[8 + 2], 4u) << 9u;

        pqep[1] = u32(pred_qep[1]);
        pqep[1] += bit_at(pred_qep[8 + 2], 5u) << 7u;
        pqep[1] += bit_at(pred_qep[12 + 2], 2u) << 8u;
        pqep[1] += bit_at(pred_qep[8 + 1], 4u) << 9u;

        pqep[2] = u32(pred_qep[2]);
        pqep[2] += bit_at(pred_qep[12 + 2], 3u) << 7u;
        pqep[2] += bit_at(pred_qep[12 + 2], 5u) << 8u;
        pqep[2] += bit_at(pred_qep[12 + 2], 4u) << 9u;

        pqep[4] = u32(pred_qep[4]) + (u32(pred_qep[8 + 1] & 15) * 64u);
        pqep[5] = u32(pred_qep[5]) + (u32(pred_qep[12 + 1] & 15) * 64u);
        pqep[6] = u32(pred_qep[6]) + (u32(pred_qep[8 + 2] & 15) * 64u);

        (*packed)[0] = get_mode_prefix(1);
        (*packed)[0] += bit_at(pred_qep[8 + 1], 5u) << 2u;
        (*packed)[0] += bit_at(pred_qep[12 + 1], 4u) << 3u;
        (*packed)[0] += bit_at(pred_qep[12 + 1], 5u) << 4u;

        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (u32(pred_qep[12]) << 6u) + u32(pred_qep[8]);
    }
    else if (mode == 2u || mode == 3u || mode == 4u) {
        var dqep: array<i32, 16>;
        for (var p = 0u; p < 3u; p++) {
            let mask = select(15, 31, p == mode - 2u);
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & mask;
            dqep[8u + p] = ((*qep)[8u + p] - (*qep)[p]) & mask;
            dqep[12u + p] = ((*qep)[12u + p] - (*qep)[p]) & mask;
        }

        var pqep: array<u32, 10>;

        pqep[0] = u32(dqep[0] & 1023);
        pqep[1] = u32(dqep[1] & 1023);
        pqep[2] = u32(dqep[2] & 1023);

        pqep[4] = u32(dqep[4]) + (u32(dqep[8 + 1] & 15) * 64u);
        pqep[5] = u32(dqep[5]) + (u32(dqep[12 + 1] & 15) * 64u);
        pqep[6] = u32(dqep[6]) + (u32(dqep[8 + 2] & 15) * 64u);

        pqep[8] = u32(dqep[8]);
        pqep[9] = u32(dqep[12]);

        if (mode == 2u) {
            (*packed)[0] = get_mode_prefix(2u);

            pqep[5] += bit_at(dqep[0 + 1], 10u) << 4u;
            pqep[6] += bit_at(dqep[0 + 2], 10u) << 4u;

            pqep[4] += bit_at(dqep[0 + 0], 10u) << 5u;
            pqep[5] += bit_at(dqep[12 + 2], 0u) << 5u;
            pqep[6] += bit_at(dqep[12 + 2], 1u) << 5u;
            pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
            pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;
        } else if (mode == 3u) {
            (*packed)[0] = get_mode_prefix(3u);

            pqep[4] += bit_at(dqep[0 + 0], 10u) << 4u;
            pqep[6] += bit_at(dqep[0 + 2], 10u) << 4u;
            pqep[8] += bit_at(dqep[12 + 2], 0u) << 4u;
            pqep[9] += bit_at(dqep[8 + 1], 4u) << 4u;

            pqep[4] += bit_at(dqep[12 + 1], 4u) << 5u;
            pqep[5] += bit_at(dqep[0 + 1], 10u) << 5u;
            pqep[6] += bit_at(dqep[12 + 2], 1u) << 5u;
            pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
            pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;
        } else if (mode == 4u) {
            (*packed)[0] = get_mode_prefix(4u);

            pqep[4] += bit_at(dqep[0 + 0], 10u) << 4u;
            pqep[5] += bit_at(dqep[0 + 1], 10u) << 4u;
            pqep[8] += bit_at(dqep[12 + 2], 1u) << 4u;
            pqep[9] += bit_at(dqep[12 + 2], 4u) << 4u;

            pqep[4] += bit_at(dqep[8 + 2], 4u) << 5u;
            pqep[5] += bit_at(dqep[12 + 2], 0u) << 5u;
            pqep[6] += bit_at(dqep[0 + 2], 10u) << 5u;
            pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
            pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;
        }

        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (pqep[9] << 6u) + pqep[8];
    }
    else if (mode == 5u) {
        var dqep: array<i32, 16>;
        for (var p = 0u; p < 3u; p++) {
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 31;
            dqep[8u + p] = ((*qep)[8u + p] - (*qep)[p]) & 31;
            dqep[12u + p] = ((*qep)[12u + p] - (*qep)[p]) & 31;
        }

        var pqep: array<u32, 10>;

        pqep[0] = u32(dqep[0]);
        pqep[1] = u32(dqep[1]);
        pqep[2] = u32(dqep[2]);
        pqep[4] = u32(dqep[4]) + u32(dqep[8 + 1] & 15) * 64u;
        pqep[5] = u32(dqep[5]) + u32(dqep[12 + 1] & 15) * 64u;
        pqep[6] = u32(dqep[6]) + u32(dqep[8 + 2] & 15) * 64u;
        pqep[8] = u32(dqep[8]);
        pqep[9] = u32(dqep[12]);

        pqep[0] += bit_at(dqep[8 + 2], 4u) << 9u;
        pqep[1] += bit_at(dqep[8 + 1], 4u) << 9u;
        pqep[2] += bit_at(dqep[12 + 2], 4u) << 9u;

        pqep[4] += bit_at(dqep[12 + 1], 4u) << 5u;
        pqep[5] += bit_at(dqep[12 + 2], 0u) << 5u;
        pqep[6] += bit_at(dqep[12 + 2], 1u) << 5u;

        pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
        pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;

        (*packed)[0] = get_mode_prefix(5u);

        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (pqep[9] << 6u) + pqep[8];
    }
    else if (mode == 6u || mode == 7u || mode == 8u) {
        var dqep: array<i32, 16>;
        for (var p = 0u; p < 3u; p++) {
            let mask = select(31, 63, p == mode - 6u);
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & mask;
            dqep[8u + p] = ((*qep)[8u + p] - (*qep)[p]) & mask;
            dqep[12u + p] = ((*qep)[12u + p] - (*qep)[p]) & mask;
        }

        var pqep: array<u32, 10>;

        pqep[0] = u32(dqep[0]);
        pqep[0] += bit_at(dqep[8 + 2], 4u) << 9u;

        pqep[1] = u32(dqep[1]);
        pqep[1] += bit_at(dqep[8 + 1], 4u) << 9u;

        pqep[2] = u32(dqep[2]);
        pqep[2] += bit_at(dqep[12 + 2], 4u) << 9u;

        pqep[4] = u32(dqep[4]) + u32(dqep[8 + 1] & 15) * 64u;
        pqep[5] = u32(dqep[5]) + u32(dqep[12 + 1] & 15) * 64u;
        pqep[6] = u32(dqep[6]) + u32(dqep[8 + 2] & 15) * 64u;

        pqep[8] = u32(dqep[8]);
        pqep[9] = u32(dqep[12]);

        if (mode == 6u) {
            (*packed)[0] = get_mode_prefix(6u);

            pqep[0] += bit_at(dqep[12 + 1], 4u) << 8u;
            pqep[1] += bit_at(dqep[12 + 2], 2u) << 8u;
            pqep[2] += bit_at(dqep[12 + 2], 3u) << 8u;
            pqep[5] += bit_at(dqep[12 + 2], 0u) << 5u;
            pqep[6] += bit_at(dqep[12 + 2], 1u) << 5u;
        }
        else if (mode == 7u) {
            (*packed)[0] = get_mode_prefix(7u);

            pqep[0] += bit_at(dqep[12 + 2], 0u) << 8u;
            pqep[1] += bit_at(dqep[8 + 1], 5u) << 8u;
            pqep[2] += bit_at(dqep[12 + 1], 5u) << 8u;
            pqep[4] += bit_at(dqep[12 + 1], 4u) << 5u;
            pqep[6] += bit_at(dqep[12 + 2], 1u) << 5u;
            pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
            pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;
        }
        else if (mode == 8u) {
            (*packed)[0] = get_mode_prefix(8u);

            pqep[0] += bit_at(dqep[12 + 2], 1u) << 8u;
            pqep[1] += bit_at(dqep[8 + 2], 5u) << 8u;
            pqep[2] += bit_at(dqep[12 + 2], 5u) << 8u;
            pqep[4] += bit_at(dqep[12 + 1], 4u) << 5u;
            pqep[5] += bit_at(dqep[12 + 2], 0u) << 5u;
            pqep[8] += bit_at(dqep[12 + 2], 2u) << 5u;
            pqep[9] += bit_at(dqep[12 + 2], 3u) << 5u;
        }

        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (pqep[9] << 6u) + pqep[8];
    }
    else if (mode == 9u) {
        var pqep: array<u32, 10>;

        pqep[0] = u32((*qep)[0]);
        pqep[0] += bit_at((*qep)[12 + 1], 4u) << 6u;
        pqep[0] += bit_at((*qep)[12 + 2], 0u) << 7u;
        pqep[0] += bit_at((*qep)[12 + 2], 1u) << 8u;
        pqep[0] += bit_at((*qep)[8 + 2], 4u) << 9u;

        pqep[1] = u32((*qep)[1]);
        pqep[1] += bit_at((*qep)[8 + 1], 5u) << 6u;
        pqep[1] += bit_at((*qep)[8 + 2], 5u) << 7u;
        pqep[1] += bit_at((*qep)[12 + 2], 2u) << 8u;
        pqep[1] += bit_at((*qep)[8 + 1], 4u) << 9u;

        pqep[2] = u32((*qep)[2]);
        pqep[2] += bit_at((*qep)[12 + 1], 5u) << 6u;
        pqep[2] += bit_at((*qep)[12 + 2], 3u) << 7u;
        pqep[2] += bit_at((*qep)[12 + 2], 5u) << 8u;
        pqep[2] += bit_at((*qep)[12 + 2], 4u) << 9u;

        pqep[4] = u32((*qep)[4]) + u32((*qep)[8 + 1] & 15) * 64u;
        pqep[5] = u32((*qep)[5]) + u32((*qep)[12 + 1] & 15) * 64u;
        pqep[6] = u32((*qep)[6]) + u32((*qep)[8 + 2] & 15) * 64u;

        (*packed)[0] = get_mode_prefix(9u);
        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
        (*packed)[3] = (u32((*qep)[12]) << 6u) + u32((*qep)[8]);
    }
    else if (mode == 10u) {
        (*packed)[0] = get_mode_prefix(10u);
        (*packed)[1] = (u32((*qep)[2]) << 20u) + (u32((*qep)[1]) << 10u) + u32((*qep)[0]);
        (*packed)[2] = (u32((*qep)[6]) << 20u) + (u32((*qep)[5]) << 10u) + u32((*qep)[4]);
    }
    else if (mode == 11u) {
        var dqep: array<i32, 8>;
        for (var p = 0u; p < 3u; p++) {
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 511;
        }

        var pqep: array<u32, 8>;

        pqep[0] = u32(dqep[0] & 1023);
        pqep[1] = u32(dqep[1] & 1023);
        pqep[2] = u32(dqep[2] & 1023);

        pqep[4] = u32(dqep[4]) + u32(dqep[0] >> 10) * 512u;
        pqep[5] = u32(dqep[5]) + u32(dqep[1] >> 10) * 512u;
        pqep[6] = u32(dqep[6]) + u32(dqep[2] >> 10) * 512u;

        (*packed)[0] = get_mode_prefix(11u);
        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
    }
    else if (mode == 12u) {
        var dqep: array<i32, 8>;
        for (var p = 0u; p < 3u; p++) {
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 255;
        }

        var pqep: array<u32, 8>;

        pqep[0] = u32(dqep[0] & 1023);
        pqep[1] = u32(dqep[1] & 1023);
        pqep[2] = u32(dqep[2] & 1023);

        pqep[4] = u32(dqep[4]) + reverse_bits(u32(dqep[0] >> 10), 2u) * 256u;
        pqep[5] = u32(dqep[5]) + reverse_bits(u32(dqep[1] >> 10), 2u) * 256u;
        pqep[6] = u32(dqep[6]) + reverse_bits(u32(dqep[2] >> 10), 2u) * 256u;

        (*packed)[0] = get_mode_prefix(12u);
        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
    }
    else if (mode == 13u) {
        var dqep: array<i32, 8>;
        for (var p = 0u; p < 3u; p++) {
            dqep[p] = (*qep)[p];
            dqep[4u + p] = ((*qep)[4u + p] - (*qep)[p]) & 15;
        }

        var pqep: array<u32, 8>;

        pqep[0] = u32(dqep[0] & 1023);
        pqep[1] = u32(dqep[1] & 1023);
        pqep[2] = u32(dqep[2] & 1023);

        pqep[4] = u32(dqep[4]) + reverse_bits(u32(dqep[0] >> 10), 6u) * 16u;
        pqep[5] = u32(dqep[5]) + reverse_bits(u32(dqep[1] >> 10), 6u) * 16u;
        pqep[6] = u32(dqep[6]) + reverse_bits(u32(dqep[2] >> 10), 6u) * 16u;

        (*packed)[0] = get_mode_prefix(13u);
        (*packed)[1] = (pqep[2] << 20u) + (pqep[1] << 10u) + pqep[0];
        (*packed)[2] = (pqep[6] << 20u) + (pqep[5] << 10u) + pqep[4];
    }
}

fn bc6h_setup(state: ptr<function, State>, block: ptr<function, array<f32, 64>>) {
    for (var p = 0u; p < 3u; p++) {
        (*state).rgb_bounds[p] = f32(0xFFFF);
        (*state).rgb_bounds[3u + p] = 0.0;
    }

    // Find min/max bounds
    for (var p = 0u; p < 3u; p++) {
        for (var k = 0u; k < 16u; k++) {
            let value = ((*block)[p * 16u + k] / 31.0) * 64.0;
            (*block)[p * 16u + k] = value;
            (*state).rgb_bounds[p] = min((*state).rgb_bounds[p], value);
            (*state).rgb_bounds[3u + p] = max((*state).rgb_bounds[3u + p], value);
        }
    }

    (*state).max_span = 0.0;
    (*state).max_span_idx = 0u;

    for (var p = 0u; p < 3u; p++) {
        let span = (*state).rgb_bounds[3u + p] - (*state).rgb_bounds[p];
        if (span > (*state).max_span) {
            (*state).max_span_idx = p;
            (*state).max_span = span;
        }
    }
}

fn compress_bc6h_core(state: ptr<function, State>, block: ptr<function, array<f32, 64>>) {
    bc6h_setup(state, block);

    if (settings.slow_mode != 0u) {
        bc6h_test_mode(state, block, 0u, true, 0.0);
        bc6h_test_mode(state, block, 1u, true, 0.0);
        bc6h_test_mode(state, block, 2u, true, 0.0);
        bc6h_test_mode(state, block, 5u, true, 0.0);
        bc6h_test_mode(state, block, 6u, true, 0.0);
        bc6h_test_mode(state, block, 9u, true, 0.0);
        bc6h_test_mode(state, block, 10u, true, 0.0);
        bc6h_test_mode(state, block, 11u, true, 0.0);
        bc6h_test_mode(state, block, 12u, true, 0.0);
        bc6h_test_mode(state, block, 13u, true, 0.0);
    } else {
        if (settings.fast_skip_threshold > 0u) {
            bc6h_test_mode(state, block, 9u, false, 0.0);

            if (settings.fast_mode != 0u) {
                bc6h_test_mode(state, block, 1u, false, 1.0);
            }

            bc6h_test_mode(state, block, 6u, false, 1.0 / 1.2);
            bc6h_test_mode(state, block, 5u, false, 1.0 / 1.2);
            bc6h_test_mode(state, block, 0u, false, 1.0 / 1.2);
            bc6h_test_mode(state, block, 2u, false, 1.0);
            bc6h_enc_2p(state, block);

            if (settings.fast_mode == 0u) {
                bc6h_test_mode(state, block, 1u, true, 0.0);
            }
        }

        bc6h_test_mode(state, block, 10u, false, 0.0);
        bc6h_test_mode(state, block, 11u, false, 1.0);
        bc6h_test_mode(state, block, 12u, false, 1.0);
        bc6h_test_mode(state, block, 13u, false, 1.0);
        bc6h_enc_1p(state, block);
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

    load_block_interleaved_16bit(&block, xx, yy);

    var state: State;
    state.best_err = 3.40282347e38;

    compress_bc6h_core(&state, &block);

    store_data(&state, block_width, xx, yy);
}
