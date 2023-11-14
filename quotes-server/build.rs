fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("PROTOC", protobuf_src::protoc());
    tonic_build::compile_protos("../proto/quotes/quote_service.proto")?;
    Ok(())
}
