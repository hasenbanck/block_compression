use block_compression::*;
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, TextureFormat::Rgba8Unorm,
    TextureViewDescriptor,
};

use crate::common::{
    create_blocks_buffer, create_wgpu_resources, download_blocks_data,
    read_image_and_create_texture, BRICK_FILE_PATH, MARBLE_FILE_PATH,
};

mod common;

fn test_multi_task_compression(variant: CompressionVariantLDR, settings: Option<Settings>) {
    let (device, queue) = create_wgpu_resources();
    let mut block_compressor = BlockCompressor::new(device.clone(), queue.clone());

    let (brick_texture, _) = read_image_and_create_texture(&device, &queue, BRICK_FILE_PATH);
    let (marble_texture, _) = read_image_and_create_texture(&device, &queue, MARBLE_FILE_PATH);

    let bricks_size = variant.blocks_byte_size(brick_texture.width(), brick_texture.height());
    let marble_size = variant.blocks_byte_size(marble_texture.width(), marble_texture.height());
    let total_size = bricks_size + marble_size;

    let blocks = create_blocks_buffer(&device, total_size as u64);

    block_compressor.add_compression_task_ldr(
        variant,
        &brick_texture.create_view(&TextureViewDescriptor {
            format: Some(Rgba8Unorm),
            ..Default::default()
        }),
        brick_texture.width(),
        brick_texture.height(),
        &blocks,
        None,
        settings,
    );
    block_compressor.add_compression_task_ldr(
        variant,
        &marble_texture.create_view(&TextureViewDescriptor {
            format: Some(Rgba8Unorm),
            ..Default::default()
        }),
        marble_texture.width(),
        marble_texture.height(),
        &blocks,
        Some(marble_size as _),
        settings,
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

    let bricks_data_is_not_empty = !blocks_data[..bricks_size].iter().all(|&data| data == 0);
    let marble_data_is_not_empty = !blocks_data[bricks_size..].iter().all(|&data| data == 0);

    assert!(bricks_data_is_not_empty);
    assert!(marble_data_is_not_empty);
}

#[test]
fn multi_task_compression_bc1() {
    test_multi_task_compression(CompressionVariantLDR::BC1, None);
}

#[test]
fn multi_task_compression_bc2() {
    test_multi_task_compression(CompressionVariantLDR::BC2, None);
}

#[test]
fn multi_task_compression_bc3() {
    test_multi_task_compression(CompressionVariantLDR::BC3, None);
}

#[test]
fn multi_task_compression_bc4() {
    test_multi_task_compression(CompressionVariantLDR::BC4, None);
}

#[test]
fn multi_task_compression_bc5() {
    test_multi_task_compression(CompressionVariantLDR::BC5, None);
}

#[test]
fn multi_task_compression_bc7() {
    test_multi_task_compression(
        CompressionVariantLDR::BC7,
        Some(Settings::BC7(BC7Settings::opaque_ultra_fast())),
    );
}
