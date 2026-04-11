fn main() -> Result<(), Box<dyn std::error::Error>> {
    prost_build::compile_protos(
        &["src/remote/prompb/remote.proto"],
        &["src/remote/prompb"],
    )?;
    Ok(())
}
