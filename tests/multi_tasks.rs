use block_compression::*;
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor, TextureViewDescriptor};

use crate::common::{
    create_blocks_buffer, create_wgpu_resources, download_blocks_data,
    read_image_and_create_texture, BRICK_FILE_PATH, MARBLE_FILE_PATH,
};

mod common;

fn test_multi_task_compression(variant: CompressionVariant) {
    let (device, queue) = create_wgpu_resources();
    let mut block_compressor = GpuBlockCompressor::new(device.clone(), queue.clone());

    let (brick_texture, _) =
        read_image_and_create_texture(&device, &queue, BRICK_FILE_PATH, variant);
    let (marble_texture, _) =
        read_image_and_create_texture(&device, &queue, MARBLE_FILE_PATH, variant);

    let bricks_size = variant.blocks_byte_size(brick_texture.width(), brick_texture.height());
    let marble_size = variant.blocks_byte_size(marble_texture.width(), marble_texture.height());
    let total_size = bricks_size + marble_size;

    let blocks = create_blocks_buffer(&device, total_size as u64);

    block_compressor.add_compression_task(
        variant,
        &brick_texture.create_view(&TextureViewDescriptor::default()),
        brick_texture.width(),
        brick_texture.height(),
        &blocks,
        None,
    );
    block_compressor.add_compression_task(
        variant,
        &marble_texture.create_view(&TextureViewDescriptor::default()),
        marble_texture.width(),
        marble_texture.height(),
        &blocks,
        Some(marble_size as _),
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
    test_multi_task_compression(CompressionVariant::BC1);
}

#[test]
fn multi_task_compression_bc2() {
    test_multi_task_compression(CompressionVariant::BC2);
}

#[test]
fn multi_task_compression_bc3() {
    test_multi_task_compression(CompressionVariant::BC3);
}

#[test]
fn multi_task_compression_bc4() {
    test_multi_task_compression(CompressionVariant::BC4);
}

#[test]
fn multi_task_compression_bc5() {
    test_multi_task_compression(CompressionVariant::BC5);
}

#[test]
fn multi_task_compression_bc6h() {
    test_multi_task_compression(CompressionVariant::BC6H(BC6HSettings::very_fast()));
}

#[test]
fn multi_task_compression_bc7() {
    test_multi_task_compression(CompressionVariant::BC7(BC7Settings::opaque_ultra_fast()));
}
