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

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> block_buffer: array<u32>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

fn sq(x: f32) -> f32 {
    return x * x;
}

fn rsqrt(x: f32) -> f32 {
    return 1.0 / sqrt(x);
}

fn rcp(x: f32) -> f32 {
    return 1.0 / x;
}

fn load_block_interleaved_rgba(block: ptr<function, array<f32, 64>>, xx: u32, yy: u32) {
    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            let pixel_x = xx * 4u + x;
            let pixel_y = yy * 4u + y;
            let rgba = textureLoad(source_texture, vec2<u32>(pixel_x, pixel_y), 0);

            (*block)[16u * 0u + y * 4u + x] = rgba.r * 255.0;
            (*block)[16u * 1u + y * 4u + x] = rgba.g * 255.0;
            (*block)[16u * 2u + y * 4u + x] = rgba.b * 255.0;
            (*block)[16u * 3u + y * 4u + x] = rgba.a * 255.0;
        }
    }
}

fn load_block_r_8bit(block: ptr<function, array<f32, 64>>, xx: u32, yy: u32) {
    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            let pixel_x = xx * 4u + x;
            let pixel_y = yy * 4u + y;
            let red = textureLoad(source_texture, vec2<u32>(pixel_x, pixel_y), 0).r;

            (*block)[48u + y * 4u + x] = red * 255.0;
        }
    }
}

fn load_block_g_8bit(block: ptr<function, array<f32, 64>>, xx: u32, yy: u32) {
    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            let pixel_x = xx * 4u + x;
            let pixel_y = yy * 4u + y;
            let green = textureLoad(source_texture, vec2<u32>(pixel_x, pixel_y), 0).g;

            (*block)[48u + y * 4u + x] = green  * 255.0;
        }
    }
}

fn load_block_alpha_4bit(xx: u32, yy: u32) -> vec2<u32> {
    var alpha_bits: vec2<u32>;

    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            let pixel_x = xx * 4u + x;
            let pixel_y = yy * 4u + y;
            let alpha = textureLoad(source_texture, vec2<u32>(pixel_x, pixel_y), 0).a;

            // Convert alpha to 4 bits (0-15)
            let alpha4 = u32(alpha * 15.0);
            let bit_position = y * 16u + x * 4u;

            if (bit_position < 32u) {
                alpha_bits[0] |= (alpha4 << bit_position);
            } else {
                alpha_bits[1] |= (alpha4 << (bit_position - 32u));
            }
        }
    }

    return alpha_bits;
}

fn store_data_2(block_width: u32, xx: u32, yy: u32, data: vec2<u32>) {
    let offset = uniforms.blocks_offset + (yy * block_width * 2u + xx * 2u);

    block_buffer[offset + 0] = data[0];
    block_buffer[offset + 1] = data[1];
}

fn store_data_4(block_width: u32, xx: u32, yy: u32, data: vec4<u32>) {
    let offset = uniforms.blocks_offset + (yy * block_width * 4u + xx * 4u);

    block_buffer[offset + 0] = data[0];
    block_buffer[offset + 1] = data[1];
    block_buffer[offset + 2] = data[2];
    block_buffer[offset + 3] = data[3];
}

fn compute_covar_dc(
    covar: ptr<function, array<f32, 6>>,
    dc: ptr<function, vec3<f32>>,
    block: ptr<function, array<f32, 64>>,
) {
    for (var p = 0u; p < 3u; p++) {
        var acc = 0.0;
        for (var k = 0u; k < 16u; k++) {
            acc += (*block)[k + p * 16u];
        }
        (*dc)[p] = acc / 16.0;
    }

    var covar0 = 0.0;
    var covar1 = 0.0;
    var covar2 = 0.0;
    var covar3 = 0.0;
    var covar4 = 0.0;
    var covar5 = 0.0;

    for (var k = 0u; k < 16u; k++) {
        let rgb0 = (*block)[k + 0u * 16u] - (*dc)[0];
        let rgb1 = (*block)[k + 1u * 16u] - (*dc)[1];
        let rgb2 = (*block)[k + 2u * 16u] - (*dc)[2];

        covar0 += rgb0 * rgb0;
        covar1 += rgb0 * rgb1;
        covar2 += rgb0 * rgb2;
        covar3 += rgb1 * rgb1;
        covar4 += rgb1 * rgb2;
        covar5 += rgb2 * rgb2;
    }

    (*covar)[0] = covar0;
    (*covar)[1] = covar1;
    (*covar)[2] = covar2;
    (*covar)[3] = covar3;
    (*covar)[4] = covar4;
    (*covar)[5] = covar5;
}

fn ssymv(result: ptr<function, vec3<f32>>, covar: ptr<function, array<f32, 6>>, a_vector: ptr<function, vec3<f32>>) {
    (*result)[0] = (*covar)[0] * (*a_vector)[0] + (*covar)[1] * (*a_vector)[1] + (*covar)[2] * (*a_vector)[2];
    (*result)[1] = (*covar)[1] * (*a_vector)[0] + (*covar)[3] * (*a_vector)[1] + (*covar)[4] * (*a_vector)[2];
    (*result)[2] = (*covar)[2] * (*a_vector)[0] + (*covar)[4] * (*a_vector)[1] + (*covar)[5] * (*a_vector)[2];
}

fn compute_axis3(axis: ptr<function, vec3<f32>>, covar: ptr<function, array<f32, 6>>, powerIterations: i32) {
    var a_vector = vec3<f32>(1.0, 1.0, 1.0);

    for (var i = 0; i < powerIterations; i++) {
        ssymv(axis, covar, &a_vector);

        for (var p = 0u; p < 3u; p++) {
            a_vector[p] = (*axis)[p];
        }

        if (i % 2 == 1) {
            var norm_sq = 0.0;
            for (var p = 0u; p < 3u; p++) {
                norm_sq += (*axis)[p] * (*axis)[p];
            }

            let rnorm = rsqrt(norm_sq);
            for (var p = 0u; p < 3u; p++) {
                a_vector[p] *= rnorm;
            }
        }
    }

    for (var p = 0u; p < 3u; p++) {
        (*axis)[p] = a_vector[p];
    }
}

fn pick_endpoints(
    c0: ptr<function, vec3<f32>>,
    c1: ptr<function, vec3<f32>>,
    block: ptr<function, array<f32, 64>>,
    axis: ptr<function, vec3<f32>>,
    dc: ptr<function, vec3<f32>>
) {
    var min_dot = 256.0 * 256.0;
    var max_dot = 0.0;

    for (var y = 0u; y < 4u; y++) {
        for (var x = 0u; x < 4u; x++) {
            var dot = 0.0;
            for (var p = 0u; p < 3u; p++) {
                dot += ((*block)[p * 16u + y * 4u + x] - (*dc)[p]) * (*axis)[p];
            }

            min_dot = min(min_dot, dot);
            max_dot = max(max_dot, dot);
        }
    }

    if (max_dot - min_dot < 1.0) {
        min_dot -= 0.5;
        max_dot += 0.5;
    }

    var norm_sq = 0.0;
    for (var p = 0u; p < 3u; p++) {
        norm_sq += (*axis)[p] * (*axis)[p];
    }

    let rnorm_sq = rcp(norm_sq);
    for (var p = 0u; p < 3u; p++) {
        (*c0)[p] = clamp((*dc)[p] + min_dot * rnorm_sq * (*axis)[p], 0.0, 255.0);
        (*c1)[p] = clamp((*dc)[p] + max_dot * rnorm_sq * (*axis)[p], 0.0, 255.0);
    }
}

fn dec_rgb565(c: ptr<function, vec3<f32>>, p: i32) {
    let b5 = (p >> 0) & 31;
    let g6 = (p >> 5) & 63;
    let r5 = (p >> 11) & 31;

    (*c)[0] = f32((r5 << 3) + (r5 >> 2));
    (*c)[1] = f32((g6 << 2) + (g6 >> 4));
    (*c)[2] = f32((b5 << 3) + (b5 >> 2));
}

fn enc_rgb565(c: ptr<function, vec3<f32>>) -> i32 {
    let r = i32((*c)[0]);
    let g = i32((*c)[1]);
    let b = i32((*c)[2]);

    let r5 = (r * 31 + 128 + ((r * 31) >> 8)) >> 8;
    let g6 = (g * 63 + 128 + ((g * 63) >> 8)) >> 8;
    let b5 = (b * 31 + 128 + ((b * 31) >> 8)) >> 8;

    return (r5 << 11) + (g6 << 5) + b5;
}

fn fast_quant(block: ptr<function, array<f32, 64>>, p0: i32, p1: i32) -> u32 {
    var c0: vec3<f32>;
    var c1: vec3<f32>;
    dec_rgb565(&c0, p0);
    dec_rgb565(&c1, p1);

    var dir: vec3<f32>;
    for (var p = 0u; p < 3u; p++) {
        dir[p] = c1[p] - c0[p];
    }

    var sq_norm = 0.0;
    for (var p = 0u; p < 3u; p++) {
        sq_norm += sq(dir[p]);
    }

    let rsq_norm = rcp(sq_norm);

    for (var p = 0u; p < 3u; p++) {
        dir[p] *= rsq_norm * 3.0;
    }

    var bias = 0.5;
    for (var p = 0u; p < 3u; p++) {
        bias -= c0[p] * dir[p];
    }

    var bits = 0u;
    var scaler = 1u;
    for (var k = 0u; k < 16u; k++) {
        var dot = 0.0;
        for (var p = 0u; p < 3u; p++) {
            dot += (*block)[k + p * 16u] * dir[p];
        }

        let q = clamp(i32(dot + bias), 0, 3);
        bits += u32(q) * scaler;
        scaler *= 4u;
    }

    return bits;
}

fn bc1_refine(pe: ptr<function, vec2<i32>>, block: ptr<function, array<f32, 64>>, bits: u32, dc: ptr<function, vec3<f32>>) {
    var c0: vec3<f32>;
    var c1: vec3<f32>;

    if ((bits ^ (bits * 4u)) < 4u) {
        for (var p = 0u; p < 3u; p++) {
            c0[p] = (*dc)[p];
            c1[p] = (*dc)[p];
        }
    } else {
        var atb1: vec3<f32>;
        var sum_q = 0.0;
        var sum_qq = 0.0;
        var shifted_bits = bits;

        for (var k = 0u; k < 16u; k++) {
            let q = f32(shifted_bits & 3u);
            shifted_bits = shifted_bits >> 2u;

            let x = 3.0 - q;

            sum_q += q;
            sum_qq += q * q;

            for (var p = 0u; p < 3u; p++) {
                atb1[p] += x * (*block)[k + p * 16u];
            }
        }

        var sum: vec3<f32>;
        var atb2: vec3<f32>;

        for (var p = 0u; p < 3u; p++) {
            sum[p] = (*dc)[p] * 16.0;
            atb2[p] = 3.0 * sum[p] - atb1[p];
        }

        let cxx = 16.0 * sq(3.0) - 2.0 * 3.0 * sum_q + sum_qq;
        let cyy = sum_qq;
        let cxy = 3.0 * sum_q - sum_qq;
        let scale = 3.0 * rcp(cxx * cyy - cxy * cxy);

        for (var p = 0u; p < 3u; p++) {
            c0[p] = (atb1[p] * cyy - atb2[p] * cxy) * scale;
            c1[p] = (atb2[p] * cxx - atb1[p] * cxy) * scale;

            c0[p] = clamp(c0[p], 0.0, 255.0);
            c1[p] = clamp(c1[p], 0.0, 255.0);
        }
    }

    (*pe)[0] = enc_rgb565(&c0);
    (*pe)[1] = enc_rgb565(&c1);
}

fn fix_qbits(qbits: u32) -> u32 {
    const MASK_01B: u32 = 0x55555555u;
    const MASK_10B: u32 = 0xAAAAAAAAu;

    let qbits0 = qbits & MASK_01B;
    let qbits1 = qbits & MASK_10B;
    return (qbits1 >> 1u) + (qbits1 ^ (qbits0 << 1u));
}

fn compress_block_bc1_core(block: ptr<function, array<f32, 64>>) -> vec2<u32> {
    let power_iterations = 4;
    let refine_iterations = 1;

    var covar: array<f32, 6>;
    var dc: vec3<f32>;
    compute_covar_dc(&covar, &dc, block);

    const eps = 0.001;
    covar[0] += eps;
    covar[3] += eps;
    covar[5] += eps;

    var axis: vec3<f32>;
    compute_axis3(&axis, &covar, power_iterations);

    var c0: vec3<f32>;
    var c1: vec3<f32>;
    pick_endpoints(&c0, &c1, block, &axis, &dc);

    var p: vec2<i32>;
    p[0] = enc_rgb565(&c0);
    p[1] = enc_rgb565(&c1);
    if (p[0] < p[1]) {
        let temp = p[0];
        p[0] = p[1];
        p[1] = temp;
    }

    var data: vec2<u32>;
    data[0] = (u32(p[1]) << 16u) | u32(p[0]);
    data[1] = fast_quant(block, p[0], p[1]);

    for (var i = 0; i < refine_iterations; i++) {
        bc1_refine(&p, block, data[1], &dc);
        if (p[0] < p[1]) {
            let temp = p[0];
            p[0] = p[1];
            p[1] = temp;
        }
        data[0] = (u32(p[1]) << 16u) | u32(p[0]);
        data[1] = fast_quant(block, p[0], p[1]);
    }

    data[1] = fix_qbits(data[1]);
    return data;
}

fn compress_block_bc3_alpha(block: ptr<function, array<f32, 64>>) -> vec2<u32> {
    var ep = vec2<f32>(255.0, 0.0);

    // Find min/max endpoints using block[48] to block[63] for alpha
    for (var k: u32 = 0u; k < 16u; k++) {
        ep[0] = min(ep[0], (*block)[48 + k]);
        ep[1] = max(ep[1], (*block)[48 + k]);
    }

    // Prevent division by zero
    if (ep[0] == ep[1]) {
        ep[1] = ep[0] + 0.1;
    }

    var qblock: vec2<u32>;
    let scale = 7.0 / (ep[1] - ep[0]);

    for (var k: u32 = 0u; k < 16u; k++) {
        let v = (*block)[48u + k];
        let proj = (v - ep[0]) * scale + 0.5;

        var q = clamp(i32(proj), 0, 7);
        q = 7 - q;

        if (q > 0) {
            q += 1;
        }
        if (q == 8) {
            q = 1;
        }

        qblock[k / 8u] |= u32(q) << ((k % 8u) * 3u);
    }

    var data: vec2<u32>;
    data[0] = (clamp(u32(ep[0]), 0u, 255u) << 8u) | clamp(u32(ep[1]), 0u, 255u);
    data[0] |= qblock[0] << 16u;
    data[1] = qblock[0] >> 16u;
    data[1] |= qblock[1] << 8u;

    return data;
}

@compute
@workgroup_size(8, 8)
fn compress_bc1(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;
    var compressed_data: vec2<u32>;

    load_block_interleaved_rgba(&block, xx, yy);

    let color_result = compress_block_bc1_core(&block);
    compressed_data[0] = color_result[0];
    compressed_data[1] = color_result[1];

    store_data_2(block_width, xx, yy, compressed_data);
}

@compute
@workgroup_size(8, 8)
fn compress_bc2(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;
    var compressed_data: vec4<u32>;

    let alpha_result = load_block_alpha_4bit(xx, yy);
    compressed_data[0] = alpha_result[0];
    compressed_data[1] = alpha_result[1];

    load_block_interleaved_rgba(&block, xx, yy);

    let color_result = compress_block_bc1_core(&block);
    compressed_data[2] = color_result[0];
    compressed_data[3] = color_result[1];

    store_data_4(block_width, xx, yy, compressed_data);
}

@compute
@workgroup_size(8, 8)
fn compress_bc3(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;
    var compressed_data: vec4<u32>;

    load_block_interleaved_rgba(&block, xx, yy);

    let alpha_result = compress_block_bc3_alpha(&block);
    compressed_data[0] = alpha_result[0];
    compressed_data[1] = alpha_result[1];

    let color_result = compress_block_bc1_core(&block);
    compressed_data[2] = color_result[0];
    compressed_data[3] = color_result[1];

    store_data_4(block_width, xx, yy, compressed_data);
}

@compute
@workgroup_size(8, 8)
fn compress_bc4(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;
    var compressed_data: vec2<u32>;

    load_block_r_8bit(&block, xx, yy);

    let color_result = compress_block_bc3_alpha(&block);
    compressed_data[0] = color_result[0];
    compressed_data[1] = color_result[1];

    store_data_2(block_width, xx, yy, compressed_data);
}

@compute
@workgroup_size(8, 8)
fn compress_bc5(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let xx = global_id.x;
    let yy = global_id.y;

    let block_width = (uniforms.width + 3u) / 4u;
    let block_height = (uniforms.height + 3u) / 4u;

    if (xx >= block_width || yy >= block_height) {
        return;
    }

    var block: array<f32, 64>;
    var compressed_data: vec4<u32>;

    load_block_r_8bit(&block, xx, yy);

    let red_result = compress_block_bc3_alpha(&block);
    compressed_data[0] = red_result[0];
    compressed_data[1] = red_result[1];

    load_block_g_8bit(&block, xx, yy);

    let green_result = compress_block_bc3_alpha(&block);
    compressed_data[2] = green_result[0];
    compressed_data[3] = green_result[1];

    store_data_4(block_width, xx, yy, compressed_data);
}
