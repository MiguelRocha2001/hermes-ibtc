fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/ibtc_ibc_service_grpc.proto")?;
    tonic_build::compile_protos("proto/ibtc_service_grpc.proto")?;
    Ok(())
}