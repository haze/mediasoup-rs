// TODO(haze): static library suffix

#[cfg(not(feature = "docs-rs"))]
fn main() {
    use std::env;

    let mut cxx = cxx_build::bridge("src/lib.rs");

    let mediasoup_include_dir = "libmediasoupclient/include";
    let libsdptransform_include_dir = "libmediasoupclient/deps/libsdptransform/include";

    let libwebrtc_include_path = env::var("LIBWEBRTC_INCLUDE_PATH").unwrap();
    let libwebrtc_abseil_include_path = libwebrtc_include_path.clone() + "/third_party/abseil-cpp";

    let libwebrtc_binary = env::var("LIBWEBRTC_BINARY_PATH").unwrap();
    let compilerrt_binary = env::var("COMPILER_RT_PATH").unwrap();
    // println!("cargo:rustc-link-search=/Users/haze/src/webrtc/src/out/m84/obj");
    // println!("cargo:rustc-link-lib=static=/Users/haze/src/webrtc/src/out/m84/obj/libwebrtc.a");

    println!("cargo:rustc-link-lib=clang_rt.osx");
    println!("cargo:rustc-link-search={}", compilerrt_binary);

    println!("cargo:rustc-link-search=native={}", libwebrtc_binary);
    println!("cargo:rustc-link-lib=static=webrtc");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=CoreServices");
    println!("cargo:rustc-link-lib=framework=AppKit");

    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let libmediasoupclient_src_path = "libmediasoupclient/src";
    let libmediasoupclient_sdp_src_path = "libmediasoupclient/src/sdp";
    let libsdptransform_src_path = "libmediasoupclient/deps/libsdptransform/src";

    let libmediasoupclient_src_paths = &[
        libmediasoupclient_src_path,
        libmediasoupclient_sdp_src_path,
        libsdptransform_src_path,
    ];

    match target_os.as_str() {
        "linux" => {
            todo!("do linux");
            // let lib = pkg_config::probe_library("libsecret-1").unwrap();
            // for dir in lib.include_paths.into_iter() {
            //     cxx.include(dir);
            // }

            // cxx.file("node-keytar/src/keytar_posix.cc")
            //     .flag("-fexceptions")
            //     .flag("-Wno-missing-field-initializers")
            //     .flag("-Wno-deprecated-declarations");
        }
        "macos" => {
            // println!("cargo:rustc-link-lib=framework=AppKit");
            // println!("cargo:rustc-link-lib=framework=Security");
            for path in libmediasoupclient_src_paths {
                let mut iter = std::fs::read_dir(path).expect("Failed to read source file");
                while let Some(Ok(file)) = iter.next() {
                    if let Ok(metadata) = file.metadata() {
                        if metadata.is_file() {
                            cxx.file(file.path());
                        }
                    }
                }
            }
        }
        "windows" => {
            todo!("do windows");
            // cxx.file("node-keytar/src/keytar_win.cc");
        }
        _ => panic!("unsupported TARGET_OS: {}", target_os),
    }

    cxx.file("src/lib.cc")
        .include(manifest_dir)
        .include(out_dir)
        .include(mediasoup_include_dir)
        .include(libsdptransform_include_dir)
        .include(libwebrtc_include_path)
        .include(libwebrtc_abseil_include_path)
        .define("WEBRTC_MAC", "1")
        .define("WEBRTC_POSIX", "1")
        .flag_if_supported("-std=c++14")
        .warnings(false)
        .compile("mediasoup-sys");

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/lib.h");
    println!("cargo:rerun-if-changed=src/lib.cc");
}
