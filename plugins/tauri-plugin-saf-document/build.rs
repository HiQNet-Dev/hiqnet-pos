const COMMANDS: &[&str] = &["export_document", "import_document"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
