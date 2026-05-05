pub mod commands;
pub mod error;
pub mod formats;
pub mod model;
pub mod recents;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::open_folder,
            commands::open_file,
            commands::save_all,
            recents::get_recents,
            recents::add_recent,
            recents::remove_recent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
