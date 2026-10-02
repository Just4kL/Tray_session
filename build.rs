//! Генерирует build_info.rs с датой сборки.
//!
//! Раньше `BUILD` был ручной константой в app.rs и забывался при бампе
//! (при 0.7.33-beta.1 остался старым). Теперь дата проставляется сама
//! при каждой сборке.

fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // chrono уже в дереве зависимостей — переиспользуем, без новых крейтов.
    let date_str = chrono::Local::now().format("%Y%m%d").to_string();
    let content = format!("pub const BUILD: &str = \"{}\";\n", date_str);
    std::fs::write(out_dir.join("build_info.rs"), content).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
