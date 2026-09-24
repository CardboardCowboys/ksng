# ksng

A work-in-progress rewrite of [KaraokeStudio](https://github.com/azrogers/KaraokeStudio). This is nowhere near stable, or even usable. Work continues.

The goal is a fully-featured karaoke video creator.

## Building

ksng uses ffmpeg to export videos. You have two options here:

- Download a shared binary build of FFmpeg, such as from [BtbN/FFmpeg-builds](https://github.com/BtbN/FFmpeg-Builds/releases/). The location of the extracted archive (the directory containing `bin`, `lib`, etc), must be set in the `FFMPEG_DIR` environment variable. ksng's `build.rs` will take care of copying the required shared object files to the output directory.
- Have ffmpeg be built automatically along with ksng using the `build-ffmpeg` feature. You must consult the ffmpeg [CompilationGuide](https://trac.ffmpeg.org/wiki/CompilationGuide) to see what packages need to be installed to get ffmpeg to build.

## License

The ksng application itself is [licensed under the GPL](./src/LICENSE). The libraries in `lib` are [licensed under the MIT license](./lib/LICENSE).

Take note that `klib` and `spectrasonic` can optionally link with GPL'd libraries when using the `build-ffmpeg` feature, or when supplying an ffmpeg installation with GPL'd components via `FFMPEG_DIR`.