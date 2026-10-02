//! Versioned, offline runtime definitions. No SDK or website is needed to load them.
use serde_json::Value;
use std::sync::OnceLock;

pub const MANIFEST: &str = include_str!("../data/manifest.json");
pub fn catalog() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../data/catalog.json"))
            .expect("bundled Fellersim catalog")
    })
}
pub fn build_id() -> &'static str {
    catalog()["buildId"].as_str().expect("bundled game build")
}
