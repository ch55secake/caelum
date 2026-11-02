mod events;
mod lifecycle;
pub(crate) mod persistence;

pub(crate) fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(lifecycle::setup);
    let app = crate::commands::register(builder)
        .build(tauri::generate_context!())
        .expect("error while running Caelum desktop application");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            lifecycle::shutdown(handle);
        }
    });
}
