# mediasoup-rs
There are a couple developer prerequisites before you can build the project (These are handled in `mediasoup-sys/build.rs`)

The following environment variables need to be set
```
LIBWEBRTC_INCLUDE_PATH=/Users/haze/src/webrtc/src
LIBWEBRTC_BINARY_PATH=/Users/haze/src/webrtc/src/out/m84/obj
COMPILER_RT_PATH=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/clang/12.0.0/lib/darwin
```

`LIBWEBRTC_BINARY_PATH` is the library search folder for the `libwebrtc` static library

`LIBWEBRTC_INCLUDE_PATH` is the source folder (and not includes!)  for libwebrtc. this is needed for the cxx crate to analyze the code for our bindings

`COMPILER_RT_PATH` is needed by the linker on apple toolchains since it uses some macro to tell the minimum ios version. This is currently blocking the project to being mac only. Once I get the main functionality out of the way I can open up build.rs for working on other platforms.

After you set those, go to the root of this repo and run `cargo run -- --help` to get help information on what flags are available / required

## TODO
- explain how to install rust, what are it's tools, etc
- explain architecture
