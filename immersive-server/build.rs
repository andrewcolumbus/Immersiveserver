//! Build script for linking platform-specific texture sharing frameworks.
//!
//! This handles:
//! - macOS: Syphon.framework for GPU texture sharing
//! - Windows: Spout SDK for DirectX texture sharing

fn main() {
    // Re-run if build script or external libraries change
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "macos")]
    {
        // Link Syphon.framework from external_libraries (one level up from immersive-server)
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let external_libs = format!("{}/../external_libraries", manifest_dir);
        let syphon_path = format!("{}/Syphon.framework", external_libs);

        // Check if Syphon.framework exists
        if std::path::Path::new(&syphon_path).exists() {
            // Add framework search path (parent directory of the framework)
            println!("cargo:rustc-link-search=framework={}", external_libs);
            // Link the Syphon framework
            println!("cargo:rustc-link-lib=framework=Syphon");
            // Set rpath so the framework can be found at runtime
            println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", external_libs);
        } else {
            // Syphon.framework not found - the stub implementation will be used
            println!(
                "cargo:warning=Syphon.framework not found at {}. Texture sharing will use stub implementation.",
                syphon_path
            );
        }

        // Also need to link system frameworks used by Syphon
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=IOSurface");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    #[cfg(target_os = "windows")]
    {
        // Copy SpoutLibrary.dll to the output directory for runtime loading
        // We use libloading for dynamic DLL loading instead of static linking
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let external_libs = format!("{}/../external_libraries", manifest_dir);
        let spout_dll_src = format!(
            "{}/Spout-SDK-binaries/Libs_2-007-017/MT/bin/SpoutLibrary.dll",
            external_libs
        );

        // Find the target directory (3 levels up from OUT_DIR which is target/debug/build/<pkg>/out)
        let target_dir = std::path::Path::new(&out_dir)
            .parent() // out -> build/<pkg>
            .and_then(|p| p.parent()) // build/<pkg> -> build
            .and_then(|p| p.parent()) // build -> debug/release
            .map(|p| p.to_path_buf());

        if let Some(target_dir) = target_dir {
            let dll_dst = target_dir.join("SpoutLibrary.dll");

            if std::path::Path::new(&spout_dll_src).exists() {
                // Copy DLL to target directory
                if std::fs::copy(&spout_dll_src, &dll_dst).is_ok() {
                    println!("cargo:warning=Copied SpoutLibrary.dll to {:?}", dll_dst);
                }
            } else {
                println!(
                    "cargo:warning=SpoutLibrary.dll not found at {}. Spout texture sharing will not be available.",
                    spout_dll_src
                );
            }
        }
    }
}
