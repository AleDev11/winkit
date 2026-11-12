//! Winkit: herramientas para Windows con Claude. Vive en la bandeja del sistema
//! y aparece con atajos globales.

#[macro_use]
mod log;
mod ai;
mod ipc;
mod lang;
mod launcher;
mod palette;
mod picker;
mod replay;
mod settings;
mod shortcuts;
mod ui;
mod updater;
mod voice;
mod win;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, RunEvent, Wry, WindowEvent};

static TRAY_MENU: OnceLock<Menu<Wry>> = OnceLock::new();

/// Añade arriba del menú de la bandeja «Reiniciar y actualizar».
pub fn tray_update_ready(version: &str) {
    let (Some(menu), app) = (TRAY_MENU.get(), ui::app()) else { return };
    if menu.get("update").is_some() {
        return;
    }
    if let Ok(item) = MenuItem::with_id(app, "update", format!("Reiniciar y actualizar a {version}"), true, None::<&str>) {
        let _ = menu.prepend(&item);
        if let Ok(sep) = PredefinedMenuItem::separator(app) {
            let _ = menu.insert(&sep, 1);
        }
    }
}

fn spawn_settings() {
    tauri::async_runtime::spawn(async { ui::open_settings() });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let launcher = MenuItem::with_id(app, "launcher", "Buscador", true, None::<&str>)?;
    let reader = MenuItem::with_id(app, "understand", "Lector", true, None::<&str>)?;
    let color = MenuItem::with_id(app, "color", "Selector de color", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Ajustes…", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pausar atajos", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&launcher, &reader, &color, &settings, &sep()?, &pause, &sep()?, &quit])?;
    let _ = TRAY_MENU.set(menu.clone());

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../../resources/tray.png"))?)
        .tooltip("Winkit")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "launcher" => launcher::toggle(),
            "understand" => drop(tauri::async_runtime::spawn(async { palette::open_understand_empty() })),
            "color" => drop(tauri::async_runtime::spawn(picker::open())),
            "settings" => spawn_settings(),
            "pause" => {
                let paused = !shortcuts::paused();
                shortcuts::set_paused(paused);
                let _ = pause.set_checked(paused);
                if let Some(tray) = app.tray_by_id("main") {
                    let _ = tray.set_tooltip(Some(if paused { "Winkit (en pausa)" } else { "Winkit" }));
                }
            }
            "update" => {
                let _ = updater::install_and_restart();
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|_, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                spawn_settings();
            }
        })
        .build(app)?;
    Ok(())
}

/// `Winkit.exe --launcher` / `--reader` con la app ya abierta: para accesos directos o scripts.
fn handle_args(args: &[String]) {
    if args.iter().any(|a| a == "--launcher") {
        launcher::toggle();
    } else if args.iter().any(|a| a == "--reader" || a == "--understand") {
        tauri::async_runtime::spawn(async { palette::open_understand_empty() });
    } else {
        spawn_settings();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log::install_panic_hook();
    log::info("app", &format!("arranca Winkit {}", env!("CARGO_PKG_VERSION")));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, args, _cwd| handle_args(&args)))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(shortcuts::handler).build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            voice::commands::voice_engines,
            voice::commands::voice_voices,
            voice::commands::voice_save_key,
            voice::commands::voice_remove_key,
            voice::commands::voice_speak,
            voice::commands::voice_stop,
            voice::commands::voice_status,
            voice::commands::voice_install,
            voice::commands::voice_cancel,
            voice::commands::voice_remove,
            ipc::ui_env,
            ipc::copy_text,
            ipc::open_settings,
            ipc::detect_language,
            ipc::palette_current,
            ipc::palette_resize,
            ipc::palette_run,
            ipc::palette_cancel,
            ipc::palette_replace,
            ipc::palette_copy,
            ipc::palette_close,
            ipc::palette_set_free,
            ipc::palette_drag,
            ipc::notice_action,
            ipc::launcher_current,
            ipc::launcher_resize,
            ipc::list_apps,
            ipc::app_icon,
            ipc::launch_app,
            ipc::web_search,
            ipc::open_url,
            ipc::system_command,
            ipc::launcher_ask,
            ipc::launcher_copy,
            ipc::launcher_hide,
            ipc::settings_get,
            ipc::settings_save,
            ipc::api_key_save,
            ipc::api_key_test,
            ipc::shortcuts_suspend,
            ipc::shortcuts_resume,
            ipc::picker_current,
            ipc::picker_capture,
            ipc::picker_done,
            ipc::clips_status,
            ipc::clips_monitors,
            ipc::clips_audio_devices,
            ipc::clips_mic_level,
            ipc::clips_choose_folder,
            ipc::clips_open_folder,
            ipc::clips_save,
            ipc::update_status,
            ipc::update_check,
            ipc::update_restart,
            ai::commands::ai_providers,
            ai::commands::ai_save_key,
            ai::commands::ai_remove_key,
            ai::commands::ai_models,
            ai::commands::ai_test,
            ai::commands::ai_model_for,
            ai::commands::local_ai_catalog,
            ai::commands::local_ai_status,
            ai::commands::local_ai_install,
            ai::commands::local_ai_cancel,
            ai::commands::local_ai_remove,
            ai::commands::local_ai_unload,
            ai::commands::local_ai_open_folder,
        ])
        .on_window_event(|window, event| match (window.label(), event) {
            (ui::PALETTE, WindowEvent::Focused(f)) => palette::on_focus_change(*f),
            (ui::LAUNCHER, WindowEvent::Focused(false)) => launcher::hide(),
            // La paleta y el buscador no se cierran: se ocultan y se reutilizan.
            (ui::PALETTE | ui::LAUNCHER, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .setup(|app| {
            let handle = app.handle().clone();
            ui::set_app(handle.clone());
            settings::migrate_electron_key();
            ui::create_popups(&handle)?;
            build_tray(&handle)?;
            // El índice de apps tarda un poco: que esté listo antes del primer Alt+Espacio.
            launcher::warm_up();
            shortcuts::register_all();
            ipc::apply_autostart(settings::get().launch_at_login);
            replay::apply(&settings::get().clips);
            updater::start();

            let args: Vec<String> = std::env::args().collect();
            let s = settings::get();
            // Sin IA lista para escribir (falta la key, el servidor o el modelo local), lo primero es configurarla.
            if !ai::ready(&s, &s.ai.for_tool("writing").provider) {
                spawn_settings();
            } else if args.iter().any(|a| a == "--launcher" || a == "--reader" || a == "--understand") {
                handle_args(&args);
            } else {
                let taken = shortcuts::taken();
                if !taken.is_empty() {
                    palette::notice(
                        format!("Otra app ya usa {}. Elige otro atajo en Ajustes.", taken.join(", ")),
                        true,
                        None,
                    );
                }
            }
            let _ = app.get_webview_window(ui::PALETTE);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("no se ha podido iniciar Winkit");

    app.run(|_app, event| {
        // La app vive en la bandeja: cerrar ventanas no la termina (solo «Salir»).
        match event {
            RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            RunEvent::Exit => {
                replay::shutdown();
                // El servidor de la IA local se para con Winkit (y el Job object lo garantiza si muere).
                ai::local::shutdown();
            }
            _ => {}
        }
    });
}
