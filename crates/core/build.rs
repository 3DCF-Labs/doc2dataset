use std::env;
use std::path::PathBuf;

fn main() {
    if let Ok(path) = protoc_bin_vendored::protoc_bin_path() {
        std::env::set_var("PROTOC", path);
    }
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let proto_dir = manifest_dir.join("proto");
    let proto_file = proto_dir.join("3dcf.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    let mut config = prost_build::Config::new();
    config.bytes(&[
        ".dcf.v1.Cell.code_id",
        ".dcf.v1.DictEntry.code_id",
        ".dcf.v1.NumGuard.sha1",
    ]);

    config
        .out_dir(PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")))
        .compile_protos(&[proto_file], &[proto_dir])
        .expect("compile protos");
}
