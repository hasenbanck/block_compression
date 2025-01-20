//! # block_compression
//!
//! Texture block compression using WGPU compute shader.
//! The shaders are a port of Intel's ISPC Texture Compressor's kernel to WGSL compute shader.
//!
//! Tested with the following backends:
//!
//! * DX12
//! * Metal
//! * Vulkan
//!
//! ## DX12 pipeline creation
//!
//! The pipeline creation for BC7 and especially BC6H takes a long time under DX12. The DXC compiler
//! seems to take a very long time to compile the shader. For this reason we moved them behind
//! features, which are included in the default features.
//!
//! ## Supported block compressions
//!
//! Currently supported block compressions are:
//!
//!  * BC1
//!  * BC2
//!  * BC3
//!  * BC4
//!  * BC5
//!  * BC6H
//!  * BC7

#![cfg_attr(docsrs, feature(doc_cfg))]

mod block_compressor;
pub mod decode;
pub mod encode;
mod settings;

use std::hash::{Hash, Hasher};

pub use block_compressor::GpuBlockCompressor;
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
pub use half;
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
pub use settings::BC6HSettings;
#[cfg(feature = "bc7")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc7")))]
pub use settings::BC7Settings;

/// Block compression variants supported by this crate.
#[derive(Copy, Clone, Debug)]
pub enum CompressionVariant {
    /// BC1 compression (RGB)
    BC1,
    /// BC2 compression with sharp alpha (RGBA)
    BC2,
    /// BC3 compression with smooth alpha (RGBA)
    BC3,
    /// BC4 compression (R)
    BC4,
    /// BC5 compression (RG)
    BC5,
    #[cfg(feature = "bc6h")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
    /// BC6H compression (RGB)
    BC6H(BC6HSettings),
    #[cfg(feature = "bc7")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bc7")))]
    /// BC7 compression with smooth alpha (RGBA)
    BC7(BC7Settings),
}

impl PartialEq for CompressionVariant {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Eq for CompressionVariant {}

impl Hash for CompressionVariant {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

impl CompressionVariant {
    /// Returns the bytes per row for the given width.
    ///
    /// The width is used to calculate how many blocks are needed per row,
    /// which is then multiplied by the block size.
    /// Width is rounded up to the nearest multiple of 4.
    pub const fn bytes_per_row(self, width: u32) -> u32 {
        let blocks_per_row = (width + 3) / 4;
        blocks_per_row * self.block_byte_size()
    }

    /// Returns the byte size required for storing compressed blocks for the given dimensions.
    ///
    /// The size is calculated based on the block compression format and rounded up dimensions.
    /// Width and height are rounded up to the nearest multiple of 4.
    pub const fn blocks_byte_size(self, width: u32, height: u32) -> usize {
        let block_width = (width as usize + 3) / 4;
        let block_height = (height as usize + 3) / 4;
        let block_count = block_width * block_height;
        let block_size = self.block_byte_size() as usize;
        block_count * block_size
    }

    const fn block_byte_size(self) -> u32 {
        match self {
            Self::BC1 | Self::BC4 => 8,
            Self::BC2 | Self::BC3 | Self::BC5 => 16,
            #[cfg(feature = "bc6h")]
            Self::BC6H(..) => 16,
            #[cfg(feature = "bc7")]
            Self::BC7(..) => 16,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::BC1 => "bc1",
            Self::BC2 => "bc2",
            Self::BC3 => "bc3",
            Self::BC4 => "bc4",
            Self::BC5 => "bc5",
            #[cfg(feature = "bc6h")]
            Self::BC6H(..) => "bc6h",
            #[cfg(feature = "bc7")]
            Self::BC7(..) => "bc7",
        }
    }

    const fn entry_point(self) -> &'static str {
        match self {
            Self::BC1 => "compress_bc1",
            Self::BC2 => "compress_bc2",
            Self::BC3 => "compress_bc3",
            Self::BC4 => "compress_bc4",
            Self::BC5 => "compress_bc5",
            #[cfg(feature = "bc6h")]
            Self::BC6H(..) => "compress_bc6h",
            #[cfg(feature = "bc7")]
            Self::BC7(..) => "compress_bc7",
        }
    }
}
