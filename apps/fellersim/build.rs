use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let directory = root
        .ancestors()
        .map(|p| p.join("default-apls"))
        .find(|p| p.join("ardeos.apl").is_file())
        .expect("shipped default-apls directory");
    let mut generated = String::from(
        "pub fn default_apl(hero: &str) -> Option<&'static str> { Some(match hero {\n",
    );
    for (hero, name) in [
        ("firemage", "ardeos"),
        ("rime", "rime"),
        ("ink", "tariq"),
        ("bowguy", "elarion"),
        ("mara", "mara"),
        ("gunde", "gunde"),
    ] {
        let path = directory.join(format!("{name}.apl"));
        println!("cargo:rerun-if-changed={}", path.display());
        let source = fs::read_to_string(&path).expect("read shipped APL");
        generated.push_str(&format!("{hero:?} => {source:?},\n"));
    }
    generated.push_str("_ => return None, }) }\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("default_apls.rs"),
        generated,
    )
    .unwrap();
}
