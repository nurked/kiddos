#[test]
fn describe_file_from_env() {
    if let Ok(p) = std::env::var("KIDDOS_DESCRIBE") {
        let bytes = std::fs::read(&p).unwrap();
        println!("{}", kiddos_wasm::describe(&bytes).unwrap());
    }
}
