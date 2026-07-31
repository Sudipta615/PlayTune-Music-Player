use std::path::PathBuf;

fn emit_link_search_if_exists(path: PathBuf) {
    if path.is_dir() {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
}

fn add_msvc_runtime_search_paths() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "x86_64-pc-windows-msvc".to_string());

    let arch_dir = if target.contains("aarch64") {
        "arm64"
    } else if target.contains("x86_64") || target.contains("x86-64") {
        "x64"
    } else {
        "x86"
    };

    if let Some(vctools) = std::env::var_os("VCToolsInstallDir") {
        let vctools = PathBuf::from(vctools);
        emit_link_search_if_exists(vctools.join("lib").join(arch_dir));
    }

    if let Some(vcinstall) = std::env::var_os("VCINSTALLDIR") {
        let vcinstall = PathBuf::from(vcinstall);
        emit_link_search_if_exists(vcinstall.join("Tools").join("MSVC").join("lib").join(arch_dir));
    }

    if let Some(tool) = cc::windows_registry::find_tool(&target, "cl.exe") {
        let mut root = tool.path().to_path_buf();

        for _ in 0..4 {
            root.pop();
        }

        emit_link_search_if_exists(root.join("lib").join(arch_dir));
    }
}

fn main() {
    let mut cmake_cfg = cmake::Config::new("modules/gui");
    if std::env::var_os("CMAKE_GENERATOR").is_none() {
        if cfg!(target_os = "windows") {
            cmake_cfg.generator("Ninja");
        } else {
            cmake_cfg.generator("Unix Makefiles");
        }
    }
    let dst = cmake_cfg.build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/lib64", dst.display());
    println!("cargo:rustc-link-lib=static=playtune_gui");

    let config = pkg_config::Config::new();
    let mut qt_found = true;

    if let Err(e) = config.probe("Qt6Widgets") {
        println!("cargo:warning=Failed to find Qt6Widgets via pkg-config: {}", e);
        println!("cargo:warning=On macOS/Windows, set CMAKE_PREFIX_PATH to your Qt6 install.");
        println!("cargo:warning=On Linux, install qt6-widgets-dev or equivalent.");
        qt_found = false;

        let mut qt_prefixes: Vec<PathBuf> = Vec::new();

        if let Some(v) = std::env::var_os("CMAKE_PREFIX_PATH") {
            qt_prefixes.extend(std::env::split_paths(&v));
        }

        if let Some(v) = std::env::var_os("QT6_DIR") {
            qt_prefixes.push(PathBuf::from(v));
        }

        if qt_prefixes.is_empty() {
            println!("cargo:warning=No Qt6 link path found (neither pkg-config nor CMAKE_PREFIX_PATH/QT6_DIR is set).");
            println!("cargo:warning=The build will likely fail at link time with unresolved Qt6 symbols.");
        } else {
            for prefix in qt_prefixes {
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
            }

            for lib in ["Qt6Widgets", "Qt6Core", "Qt6Gui"] {
                if cfg!(target_os = "macos") {
                    println!("cargo:rustc-link-lib=framework={}", lib);
                } else {
                    println!("cargo:rustc-link-lib=dylib={}", lib);
                }
            }
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

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if cfg!(target_env = "msvc") {
        add_msvc_runtime_search_paths();
        println!("cargo:rustc-link-lib=dylib=msvcp140");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=modules/gui");
}
