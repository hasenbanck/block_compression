# block_compression

Texture block compression using WGPU compute shader.
The shaders are a port of Intel's ISPC Texture Compressor's kernel to WGSL compute shader.

Tested with the following backends:

* DX12
* Metal
* Vulkan

## Supported block compressions

Currently supported block compressions are:

* BC1
* BC2
* BC3
* BC4
* BC5
* BC6H
* BC7

## License

This project is licensed under the [MIT](LICENSE) license.
