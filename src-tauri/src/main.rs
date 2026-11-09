// Sin consola en la versión de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    winkit_lib::run()
}
