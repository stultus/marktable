pub mod commands;
pub mod error;
pub mod formats;
pub mod model;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::open_folder,
            commands::open_file,
            commands::save_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
