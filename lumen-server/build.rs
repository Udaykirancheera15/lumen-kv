//! Compile the protobuf definitions into Rust source at build time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path(out_dir.join("kv_descriptor.bin"))
        .compile(
            &["../proto/kv.proto"],
            &["../proto"],
        )?;

    println!("cargo:rerun-if-changed=../proto/kv.proto");
    Ok(())
}
