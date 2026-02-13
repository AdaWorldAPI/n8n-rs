fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile protobuf definitions using tonic-prost-build (tonic 0.14+)
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/generated")
        .compile_protos(&["proto/n8n.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/n8n.proto");

    Ok(())
}
