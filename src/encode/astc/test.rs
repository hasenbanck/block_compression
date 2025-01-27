use super::*;
use crate::ASTCBlockSize;

#[test]
fn pack_block_4x4_0() {
    const INPUT_BLOCK: AstcBlock = AstcBlock {
        width: 4,
        height: 4,
        dual_plane: 0,
        weight_range: 8,
        weights: [
            1, 8, 14, 13, 3, 9, 9, 8, 0, 7, 7, 2, 1, 2, 5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0,
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
            7, 9, 10, 4, 1, 5, 3, 8, 1, 1, 11, 10, 5, 5, 5, 7, 4, 3, 2, 3, 4, 1, 3, 1, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 48, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 64, 0, 0, 0,
            21, 0, 0, 0, 21, 0, 0, 0,
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
            4, 4, 0, 1, 0, 0, 7, 5, 3, 7, 4, 6, 7, 5, 2, 2, 3, 5, 2, 4, 2, 4, 1, 2, 16, 0, 0, 0,
            16, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 21, 0, 0, 0, 64, 0, 0,
            0, 21, 0, 0, 0, 21, 0, 0, 0,
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
            1, 1, 3, 5, 3, 5, 3, 1, 5, 3, 5, 3, 3, 1, 4, 4, 4, 1, 5, 5, 0, 0, 0, 4, 38, 0, 0, 0, 0,
            0, 0, 0, 26, 0, 0, 0, 38, 0, 0, 0, 26, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0,
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
            0, 8, 2, 6, 9, 9, 9, 4, 1, 7, 8, 8, 8, 7, 9, 3, 5, 0, 4, 4, 4, 1, 4, 1, 26, 0, 0, 0,
            64, 0, 0, 0, 0, 0, 0, 0, 51, 0, 0, 0, 38, 0, 0, 0, 64, 0, 0, 0, 64, 0, 0, 0, 64, 0, 0,
            0, 0, 0, 0, 0, 21, 0, 0, 0,
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

#[test]
fn test_block_layout_4x4() {
    let rgba_data = [255u8; 64];
    let mut pixels = [0.0f32; 256];

    let settings = ASTCSettings::alpha_slow(ASTCBlockSize::_4x4);
    let mode_ranker = ModeRankerASTC::new(&settings);
    mode_ranker.load_block_interleaved(&rgba_data, 0, 0, 16, &mut pixels);

    // Check first row of red channel (should use first 4 of 8 slots)
    assert_eq!(
        &pixels[0..8],
        &[255.0, 255.0, 255.0, 255.0, 0.0, 0.0, 0.0, 0.0]
    );

    // Check first row of green channel (offset by 64)
    assert_eq!(
        &pixels[64..72],
        &[255.0, 255.0, 255.0, 255.0, 0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn test_block_layout_8x8() {
    // Alternating red, green, blue, yellow pixels
    const RGBA_DATA: [u8; 256] = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255,
        0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255,
        255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0,
        0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255,
        0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255,
        0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255,
        0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0,
        255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0,
        255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0,
        255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255,
        255, 255, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];

    let settings = ASTCSettings::alpha_slow(ASTCBlockSize::_8x8);
    let mut mode_ranker = ModeRankerASTC::new(&settings);

    let mut mode_buffer = [0; 64];
    let mut pixels = [0.0; 256];

    mode_ranker.rank(&RGBA_DATA, 0, 0, 32, &mut mode_buffer, &mut pixels);

    // red channel
    for i in 0..64 {
        assert_eq!(pixels[i] as u8, RGBA_DATA[i * 4]);
    }
    // green channel
    for i in 0..64 {
        assert_eq!(pixels[64 + i] as u8, RGBA_DATA[i * 4 + 1]);
    }
    // blue channel
    for i in 0..64 {
        assert_eq!(pixels[128 + i] as u8, RGBA_DATA[i * 4 + 2]);
    }
    // alpha channel
    for i in 0..64 {
        assert_eq!(pixels[192 + i] as u8, RGBA_DATA[i * 4 + 3]);
    }
}

#[test]
fn rank_block_0() {
    let rgba_data: [u8; 64] = [
        152, 92, 59, 255, 147, 89, 58, 255, 145, 89, 58, 255, 145, 89, 59, 255, 151, 91, 59, 255,
        151, 91, 58, 255, 146, 89, 59, 255, 144, 89, 60, 255, 154, 92, 59, 255, 156, 93, 59, 255,
        150, 91, 59, 255, 147, 90, 60, 255, 155, 93, 59, 255, 153, 93, 60, 255, 152, 93, 60, 255,
        146, 91, 61, 255,
    ];

    let mut expected_modes: [u32; 64] = [
        0x6961544B, 0x61DA3344, 0x6961544B, 0x6961544B, 0x6961544B, 0x6961544B, 0x5FB2344B,
        0x5FB2344B, 0x6951544A, 0x65FA3364, 0x69415449, 0x69415449, 0x69415449, 0x69415449,
        0x5FA2344A, 0x5FA2344A, 0x69415449, 0x61CA3443, 0x6951544A, 0x6951544A, 0x6951544A,
        0x6951544A, 0x5F723447, 0x5F923449, 0x69315448, 0xA8FA1447, 0x69315448, 0x69315448,
        0x69315448, 0x69315448, 0x5F823448, 0x5F823448, 0x69215447, 0xA8EA1446, 0x69215447,
        0x69215447, 0x69215447, 0x69215447, 0x5F923449, 0x5F723447, 0x6A6154C7, 0x74E93446,
        0x6A6154C7, 0x6A6154C7, 0x6A7153C8, 0x6A6154C7, 0x40BA5262, 0x60B234C7, 0x6A7153C8,
        0x65EA3463, 0x69115446, 0x6A7153C8, 0x6A6154C7, 0x6A7153C8, 0x60B234C7, 0x60C233C8,
        0x5FA2344A, 0xAE7A1467, 0x6A5154C6, 0x6A5154C6, 0x5F923449, 0x6A5154C6, 0x5F623446,
        0x60A234C6,
    ];

    let settings = ASTCSettings::alpha_slow(ASTCBlockSize::_4x4);
    let mut mode_ranker = ModeRankerASTC::new(&settings);

    let mut mode_buffer = [0; 64];
    mode_ranker.rank(&rgba_data, 0, 0, 16, &mut mode_buffer, &mut [0.0; 256]);

    expected_modes.sort();
    mode_buffer.sort();

    assert_eq!(expected_modes, mode_buffer)
}

#[test]
fn test_compute_moments() {
    let pixels: [f32; 256] = [
        190.0, 189.0, 189.0, 185.0, 0.0, 0.0, 0.0, 0.0, 191.0, 189.0, 188.0, 185.0, 0.0, 0.0, 0.0,
        0.0, 188.0, 187.0, 187.0, 183.0, 0.0, 0.0, 0.0, 0.0, 185.0, 185.0, 183.0, 184.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 190.0,
        189.0, 189.0, 177.0, 0.0, 0.0, 0.0, 0.0, 191.0, 189.0, 188.0, 171.0, 0.0, 0.0, 0.0, 0.0,
        188.0, 187.0, 187.0, 162.0, 0.0, 0.0, 0.0, 0.0, 185.0, 185.0, 183.0, 176.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 190.0, 189.0,
        189.0, 172.0, 0.0, 0.0, 0.0, 0.0, 191.0, 189.0, 188.0, 164.0, 0.0, 0.0, 0.0, 0.0, 188.0,
        187.0, 187.0, 151.0, 0.0, 0.0, 0.0, 0.0, 185.0, 185.0, 183.0, 171.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 255.0, 255.0, 255.0,
        255.0, 0.0, 0.0, 0.0, 0.0, 255.0, 255.0, 255.0, 255.0, 0.0, 0.0, 0.0, 0.0, 255.0, 255.0,
        255.0, 255.0, 0.0, 0.0, 0.0, 0.0, 255.0, 255.0, 255.0, 255.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ];

    let mut stats = [0.0f32; 15];

    let set = PixelSet {
        pixels,
        block_width: 4,
        block_height: 4,
    };

    set.compute_moments(&mut stats, 4);

    let expected_stats = [
        558104.0, 548719.0, 543566.0, 761940.0, 540099.0, 535355.0, 748935.0, 530831.0, 741795.0,
        1040400.0, 2988.0, 2937.0, 2909.0, 4080.0, 16.0,
    ];

    assert_eq!(stats, expected_stats);
}
