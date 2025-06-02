# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2025-06-02

### Changed

- `GpuBlockCompressor::new()` takes the WGPU device and queue directly without an Arc wrapped around it. WGPU 25
  made the main structures clonable, since they are internally reference counted, so it's not needed anymore to wrap
  them in a smart pointer anymore.

### Fixed

- Fix an issue with AMD integrated GPU's where WGPU's forced loop bounding in shaders made running the BC7 shader
  impossible.

## [0.4.0] - 2025-04-11

### Updated

- Target WGPU 25

## [0.3.0] - 2025-02-21

### Updated

- Allow the GPU compressor to use row based offsets into the texture to
  allow submitting smaller chunks of work.

## [0.2.1] - 2025-02-17

### Updated

- Fix BC6H encoding for black pixels
- Use adapter limits in the example compressor
- Improve PSNR output CPU of when compared to the GPU versions of BC6H / BC7

## [0.2.0] - 2025-01-22

### Added

- Provide more feature flags for optional features
- Implemented CPU based BC6H encoding
- Implemented CPU based BC7 encoding

## [0.1.1] - 2025-01-20

### Updated

- Fix compilation with no default features.

## [0.1.0] - 2025-01-20

### Added

- Initial release.
