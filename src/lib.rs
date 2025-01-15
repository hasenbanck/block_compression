//! # block_compression
//!
//! Texture block compression using WGPU compute shader.
//! The shaders are a port of Intel's ISPC Texture Compressor's kernel to WGSL compute shader.
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
//!  * BC7
//!
//! Soon:
//!
//!  * BC6H
mod block_compressor;
pub mod decode;
mod settings;

pub use block_compressor::BlockCompressor;
pub use settings::{BC6HSettings, BC7Settings, Settings};

/// Compression variants supported by this crate for 8-bit LDR data.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum CompressionVariantLDR {
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
    /// BC6H compression (RGB LDR)
    BC6H,
    /// BC7 compression with smooth alpha (RGBA)
    BC7,
}

impl CompressionVariantLDR {
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
            Self::BC2 | Self::BC3 | Self::BC5 | Self::BC6H | Self::BC7 => 16,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::BC1 => "bc1",
            Self::BC2 => "bc2",
            Self::BC3 => "bc3",
            Self::BC4 => "bc4",
            Self::BC5 => "bc5",
            Self::BC6H => "bc6h",
            Self::BC7 => "bc7",
        }
    }

    const fn entry_point(self) -> &'static str {
        match self {
            Self::BC1 => "compress_bc1",
            Self::BC2 => "compress_bc2",
            Self::BC3 => "compress_bc3",
            Self::BC4 => "compress_bc4",
            Self::BC5 => "compress_bc5",
            Self::BC6H => "compress_bc6h",
            Self::BC7 => "compress_bc7",
        }
    }
}

/// Compression variants supported by this crate for 16-bit HDR data.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum CompressionVariantHDR {
    /// BC6H compression (RGB HDR)
    BC6H,
}

impl CompressionVariantHDR {
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
            Self::BC6H => 16,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::BC6H => "bc6h",
        }
    }

    const fn entry_point(self) -> &'static str {
        match self {
            Self::BC6H => "compress_bc6h",
        }
    }
}
