fn main()  {
    let proto_file = "src/httpgrpc.proto";
    tonic_build::configure()
        .emit_rerun_if_changed(true)
        .build_server(true)
        .out_dir("./src")
        .compile(&[proto_file], &["."])
        .unwrap_or_else(|e| panic!("protobuf compile error: {}", e));
}