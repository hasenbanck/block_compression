#[cfg(any(feature = "bc6h", feature = "bc7"))]
use bytemuck::{Pod, Zeroable};

/// Encoding settings for ASTC.
#[cfg(feature = "astc")]
#[cfg_attr(docsrs, doc(cfg(feature = "astc")))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct ASTCSettings {
    pub(crate) block_width: u32,
    pub(crate) block_height: u32,
    pub(crate) channels: u32,
    pub(crate) fast_skip_threshold: u32,
    pub(crate) refine_iterations: u32,
}

/// The block sizes ASTS can use when encoding.
#[cfg(feature = "astc")]
#[cfg_attr(docsrs, doc(cfg(feature = "astc")))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ASTCBlockSize {
    _4x4,
    _5x4,
    _5x5,
    _6x5,
    _6x6,
    _8x5,
    _8x6,
    _8x8,
}

impl ASTCBlockSize {
    /// Returns the size of a block as width and height in pixels.
    pub const fn block_size(self) -> (u32, u32) {
        match self {
            ASTCBlockSize::_4x4 => (4, 4),
            ASTCBlockSize::_5x4 => (5, 4),
            ASTCBlockSize::_5x5 => (5, 5),
            ASTCBlockSize::_6x5 => (6, 5),
            ASTCBlockSize::_6x6 => (6, 6),
            ASTCBlockSize::_8x5 => (8, 5),
            ASTCBlockSize::_8x6 => (8, 6),
            ASTCBlockSize::_8x8 => (8, 8),
        }
    }
}
#[cfg(feature = "astc")]
impl ASTCSettings {
    /// Opaque fast settings.
    pub const fn opaque_fast(block_size: ASTCBlockSize) -> Self {
        let (block_width, block_height) = block_size.block_size();
        Self {
            block_width,
            block_height,
            channels: 3,
            fast_skip_threshold: 5,
            refine_iterations: 2,
        }
    }

    /// Opaque slow settings.
    pub const fn opaque_slow(block_size: ASTCBlockSize) -> Self {
        let (block_width, block_height) = block_size.block_size();
        Self {
            block_width,
            block_height,
            channels: 3,
            fast_skip_threshold: 64,
            refine_iterations: 2,
        }
    }

    /// Alpha fast settings.
    pub const fn alpha_fast(block_size: ASTCBlockSize) -> Self {
        let (block_width, block_height) = block_size.block_size();
        Self {
            block_width,
            block_height,
            channels: 4,
            fast_skip_threshold: 5,
            refine_iterations: 2,
        }
    }

    /// Alpha slow settings.
    pub const fn alpha_slow(block_size: ASTCBlockSize) -> Self {
        let (block_width, block_height) = block_size.block_size();
        Self {
            block_width,
            block_height,
            channels: 4,
            fast_skip_threshold: 64,
            refine_iterations: 2,
        }
    }
}

/// Encoding settings for BC6H.
#[cfg(feature = "bc6h")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc6h")))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct BC6HSettings {
    pub(crate) slow_mode: u32,
    pub(crate) fast_mode: u32,
    pub(crate) refine_iterations_1p: u32,
    pub(crate) refine_iterations_2p: u32,
    pub(crate) fast_skip_threshold: u32,
}

#[cfg(feature = "bc6h")]
impl BC6HSettings {
    /// Very fast settings.
    pub const fn very_fast() -> Self {
        Self {
            slow_mode: false as _,
            fast_mode: true as _,
            fast_skip_threshold: 0,
            refine_iterations_1p: 0,
            refine_iterations_2p: 0,
        }
    }

    /// Fast settings.
    pub const fn fast() -> Self {
        Self {
            slow_mode: false as _,
            fast_mode: true as _,
            fast_skip_threshold: 2,
            refine_iterations_1p: 0,
            refine_iterations_2p: 1,
        }
    }

    /// Basic settings.
    pub const fn basic() -> Self {
        Self {
            slow_mode: false as _,
            fast_mode: false as _,
            fast_skip_threshold: 4,
            refine_iterations_1p: 2,
            refine_iterations_2p: 2,
        }
    }

    /// Slow settings.
    pub const fn slow() -> Self {
        Self {
            slow_mode: true as _,
            fast_mode: false as _,
            fast_skip_threshold: 10,
            refine_iterations_1p: 2,
            refine_iterations_2p: 2,
        }
    }

    /// Very slow settings.
    pub const fn very_slow() -> Self {
        Self {
            slow_mode: true as _,
            fast_mode: false as _,
            fast_skip_threshold: 32,
            refine_iterations_1p: 2,
            refine_iterations_2p: 2,
        }
    }
}

#[cfg(feature = "bc7")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc7")))]
/// Encoding settings for BC7.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct BC7Settings {
    pub(crate) refine_iterations: [u32; 8],
    pub(crate) mode_selection: [u32; 4],
    pub(crate) skip_mode2: u32,
    pub(crate) fast_skip_threshold_mode1: u32,
    pub(crate) fast_skip_threshold_mode3: u32,
    pub(crate) fast_skip_threshold_mode7: u32,
    pub(crate) mode45_channel0: u32,
    pub(crate) refine_iterations_channel: u32,
    pub(crate) channels: u32,
}

#[cfg(feature = "bc7")]
#[cfg_attr(docsrs, doc(cfg(feature = "bc7")))]
impl BC7Settings {
    /// Opaque ultra fast settings.
    pub const fn opaque_ultra_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, false as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 3,
            fast_skip_threshold_mode3: 1,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 1, 0],
        }
    }

    /// Opaque very fast settings.
    pub const fn opaque_very_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, true as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 3,
            fast_skip_threshold_mode3: 1,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 1, 0],
        }
    }

    /// Opaque fast settings.
    pub const fn opaque_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, true as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 4,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 2, 0],
        }
    }

    /// Opaque basic settings.
    pub const fn opaque_basic() -> Self {
        Self {
            channels: 3,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 8,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 2,
            refine_iterations: [2, 2, 2, 2, 2, 2, 2, 0],
        }
    }

    /// Opaque slow settings.
    pub const fn opaque_slow() -> Self {
        Self {
            channels: 3,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: false as _,
            fast_skip_threshold_mode1: 64,
            fast_skip_threshold_mode3: 64,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 4,
            refine_iterations: [4, 4, 4, 4, 4, 4, 4, 0],
        }
    }

    /// Alpha ultra fast settings.
    pub const fn alpha_ultrafast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, false as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 0,
            fast_skip_threshold_mode3: 0,
            fast_skip_threshold_mode7: 4,
            mode45_channel0: 3,
            refine_iterations_channel: 1,
            refine_iterations: [2, 1, 2, 1, 1, 1, 2, 2],
        }
    }

    /// Alpha very fast settings.
    pub const fn alpha_very_fast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 0,
            fast_skip_threshold_mode3: 0,
            fast_skip_threshold_mode7: 4,
            mode45_channel0: 3,
            refine_iterations_channel: 2,
            refine_iterations: [2, 1, 2, 1, 2, 2, 2, 2],
        }
    }

    /// Alpha fast settings.
    pub const fn alpha_fast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 4,
            fast_skip_threshold_mode3: 4,
            fast_skip_threshold_mode7: 8,
            mode45_channel0: 3,
            refine_iterations_channel: 2,
            refine_iterations: [2, 1, 2, 1, 2, 2, 2, 2],
        }
    }

    /// Alpha basic settings.
    pub const fn alpha_basic() -> Self {
        Self {
            channels: 4,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 8,
            fast_skip_threshold_mode7: 8,
            mode45_channel0: 0,
            refine_iterations_channel: 2,
            refine_iterations: [2, 2, 2, 2, 2, 2, 2, 2],
        }
    }

    /// Alpha slow settings.
    pub const fn alpha_slow() -> Self {
        Self {
            channels: 4,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: false as _,
            fast_skip_threshold_mode1: 64,
            fast_skip_threshold_mode3: 64,
            fast_skip_threshold_mode7: 64,
            mode45_channel0: 0,
            refine_iterations_channel: 4,
            refine_iterations: [4, 4, 4, 4, 4, 4, 4, 4],
        }
    }
}
