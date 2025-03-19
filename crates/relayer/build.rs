fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/ibc_service_grpc.proto")?;
    Ok(())
}