fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 通过系统 protoc 编译 proto 定义，生成 Rust 绑定到 OUT_DIR
    prost_build::compile_protos(&["proto/easyshare.proto"], &["proto/"])?;
    println!("cargo:rerun-if-changed=proto/easyshare.proto");
    Ok(())
}
