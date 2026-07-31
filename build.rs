use std::path::PathBuf;

fn main() {
    // 1. Build the Qt6 GUI via CMake.
    let mut cmake_cfg = cmake::Config::new("modules/gui");
    if std::env::var_os("CMAKE_GENERATOR").is_none() {
        if cfg!(target_os = "windows") {
            cmake_cfg.generator("Ninja");
        } else {
            cmake_cfg.generator("Unix Makefiles");
        }
    }
    let dst = cmake_cfg.build();

    // 2. Link our custom static library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/lib64", dst.display());
    println!("cargo:rustc-link-lib=static=playtune_gui");

    // 3. Link Qt6 libraries via pkg-config (Linux) or via CMAKE_PREFIX_PATH
    //    fallback (macOS / Windows).
    let config = pkg_config::Config::new();
    let mut qt_found = true;
    if let Err(e) = config.probe("Qt6Widgets") {
        println!("cargo:warning=Failed to find Qt6Widgets via pkg-config: {}", e);
        println!("cargo:warning=On macOS/Windows, set CMAKE_PREFIX_PATH to your Qt6 install.");
        println!("cargo:warning=On Linux, install qt6-widgets-dev or equivalent.");
        qt_found = false;

        let qt_prefix =
            std::env::var_os("CMAKE_PREFIX_PATH").or_else(|| std::env::var_os("QT6_DIR"));

        if let Some(prefix) = qt_prefix {
            let prefix = PathBuf::from(prefix);

            let candidates = [
                prefix.join("lib"),
                prefix.join("lib64"),
                prefix.join("msvc2019_64/lib"),
                prefix.join("msvc2022_64/lib"),
            ];

            for dir in candidates {
                if dir.exists() {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
            }
            // Emit link directives for the three Qt6 libs we actually use.
            // The CMake build already links them into the static lib via
            // target_link_libraries(... PUBLIC Qt6::Widgets ...), but a
            // static lib does NOT propagate link directives to the final
            // Rust binary's link line — we must emit them here.
            for lib in ["Qt6Widgets", "Qt6Core", "Qt6Gui"] {
                if cfg!(target_os = "windows") && cfg!(target_env = "msvc") {
                    // MSVC: <name>.lib, linked as `<name>` (no `lib` prefix).
                    println!("cargo:rustc-link-lib=dylib={}", lib);
                } else if cfg!(target_os = "macos") {
                    println!("cargo:rustc-link-lib=framework={}", lib);
                } else {
                    // Linux/MinGW: lib<name>.so / lib<name>.dll.a — the
                    // `dylib=` kind asks the linker for `lib<name>.so`.
                    println!("cargo:rustc-link-lib=dylib={}", lib);
                }
            }
        } else {
            println!("cargo:warning=No Qt6 link path found (neither pkg-config nor CMAKE_PREFIX_PATH/QT6_DIR is set).");
            println!("cargo:warning=The build will likely fail at link time with 'undefined reference to Qt6* symbols'.");
            println!("cargo:warning=Set CMAKE_PREFIX_PATH to your Qt6 install (e.g. ~/Qt/6.7.0/macos) and rebuild.");
        }
    }
    if qt_found {
        for lib in ["Qt6Gui", "Qt6Core"] {
            if let Err(e) = pkg_config::Config::new().probe(lib) {
                println!("cargo:warning=Failed to find {} via pkg-config: {}", lib, e);
                println!(
                    "cargo:warning=Install {}-dev or set CMAKE_PREFIX_PATH to your Qt6 install.",
                    lib
                );
            }
        }
    }

    // 4. Link C++ Standard Library.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if cfg!(target_env = "msvc") {
        // MSVC links the C runtime automatically, but the C++ standard
        // library (msvcp140) is NOT — it must be linked explicitly.
        // Without this, linking the CMake-built C++ static library
        // (playtune_gui) fails with LNK1120 (unresolved externals).
        println!("cargo:rustc-link-lib=dylib=msvcp140");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    // Re-run build if C++ files or build script change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=modules/gui");
}
