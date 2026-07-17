fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Instruct tonic-build to compile your protobuf file
    tonic_prost_build::compile_protos("proto/gateway.proto")?;
    Ok(())
}