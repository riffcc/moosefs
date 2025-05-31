use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Enhanced tonic build configuration for performance and features
    tonic_build::configure()
        .out_dir(out_dir)
        .build_server(true)
        .build_client(true)
        .build_transport(true)
        // Skip serde for now to focus on core functionality
        // Optimize for zero-copy operations where possible
        .bytes(["."])
        // Add custom attributes for better performance
        .server_mod_attribute("attrs", "#[cfg(feature = \"server\")]")
        .client_mod_attribute("attrs", "#[cfg(feature = \"client\")]")
        // Include file descriptor sets for reflection
        .include_file("mod.rs")
        .emit_rerun_if_changed(true)
        .compile_protos(
            &[
                "proto/common.proto",
                "proto/master.proto", 
                "proto/chunkserver.proto",
                "proto/client.proto",
            ],
            &["proto"],
        )?;

    // Rebuild on proto file changes
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=proto/common.proto");
    println!("cargo:rerun-if-changed=proto/master.proto");
    println!("cargo:rerun-if-changed=proto/chunkserver.proto");
    println!("cargo:rerun-if-changed=proto/client.proto");
        
    Ok(())
}