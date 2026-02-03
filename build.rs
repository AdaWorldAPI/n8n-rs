//! Build script for proto compilation
//!
//! Only compiles protos when the "flight" feature is enabled.

fn main() {
    #[cfg(feature = "flight")]
    {
        let proto_file = "proto/ada.proto";

        // Rerun if proto changes
        println!("cargo:rerun-if-changed={}", proto_file);

        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .out_dir("src/generated")
            .compile(&[proto_file], &["proto"])
            .expect("Failed to compile protos");
    }
}
