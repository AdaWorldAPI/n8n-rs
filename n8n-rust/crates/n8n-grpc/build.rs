fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile protobuf definitions
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/generated")
        .compile(&["proto/n8n.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/n8n.proto");

    Ok(())
}
