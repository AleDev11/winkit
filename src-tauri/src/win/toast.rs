//! Notificaciones de Windows (las del centro de notificaciones).
//!
//! Para que salgan con el nombre e icono de Winkit, Windows necesita un acceso
//! directo en el menú Inicio con el mismo AppUserModelID: lo crea el instalador
//! (com.alejandrofont.winkit). En desarrollo no existe, así que se usa el de
//! PowerShell para poder probarlas.
//!
//! Los clics se reciben con el evento `Activated` mientras Winkit está abierto
//! (siempre, porque vive en la bandeja): no hace falta registrar un activador COM.

use std::sync::Mutex;
use windows::core::{Interface, HSTRING};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager};

const AUMID: &str = "com.alejandrofont.winkit";
const DEV_AUMID: &str = r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// Las notificaciones mostradas deben seguir vivas para recibir sus clics.
static ALIVE: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());

fn aumid() -> &'static str {
    if cfg!(debug_assertions) {
        DEV_AUMID
    } else {
        AUMID
    }
}

/// En la app instalada, que el proceso use el mismo id que el acceso directo.
pub fn init() {
    if !cfg!(debug_assertions) {
        unsafe {
            let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(AUMID));
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Qué hacer al pulsar la notificación o uno de sus botones.
#[derive(Clone)]
pub enum OnClick {
    /// Abre el archivo con su programa (el vídeo con el reproductor).
    Open(String),
    /// Abre el Explorador con el archivo seleccionado.
    Reveal(String),
    /// Abre Ajustes de Winkit.
    Settings,
}

impl OnClick {
    fn encode(&self) -> String {
        match self {
            OnClick::Open(p) => format!("open|{p}"),
            OnClick::Reveal(p) => format!("reveal|{p}"),
            OnClick::Settings => "settings|".into(),
        }
    }

    fn run(arg: &str) {
        let (kind, path) = arg.split_once('|').unwrap_or((arg, ""));
        match kind {
            "open" if !path.is_empty() => super::system::open(path),
            "reveal" if !path.is_empty() => {
                let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{path}")).spawn();
            }
            "settings" => {
                tauri::async_runtime::spawn(async { crate::ui::open_settings() });
            }
            _ => {}
        }
    }
}

/// Muestra una notificación. Devuelve false si Windows no la ha aceptado
/// (para avisar de otra forma).
pub fn show(title: &str, body: &str, click: Option<OnClick>, buttons: &[(&str, OnClick)]) -> bool {
    match try_show(title, body, click, buttons) {
        Ok(()) => true,
        Err(e) => {
            crate::log_warn!("toast", "no se ha podido mostrar la notificación: {e}");
            false
        }
    }
}

fn try_show(title: &str, body: &str, click: Option<OnClick>, buttons: &[(&str, OnClick)]) -> windows::core::Result<()> {
    let launch = click.as_ref().map(|c| format!(r#" launch="{}""#, escape(&c.encode()))).unwrap_or_default();
    let actions: String = buttons
        .iter()
        .map(|(label, c)| {
            format!(r#"<action content="{}" arguments="{}" activationType="foreground"/>"#, escape(label), escape(&c.encode()))
        })
        .collect();
    let xml = format!(
        r#"<toast{launch} activationType="foreground"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>"#,
        escape(title),
        escape(body),
        if actions.is_empty() { String::new() } else { format!("<actions>{actions}</actions>") }
    );

    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    toast.Activated(&TypedEventHandler::new(|_, args: windows::core::Ref<windows::core::IInspectable>| {
        if let Some(a) = args.as_ref().and_then(|a| a.cast::<ToastActivatedEventArgs>().ok()) {
            if let Ok(arg) = a.Arguments() {
                OnClick::run(&arg.to_string());
            }
        }
        Ok(())
    }))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid()))?.Show(&toast)?;

    let mut alive = ALIVE.lock().unwrap_or_else(|e| e.into_inner());
    alive.push(toast);
    // Basta con las últimas: las antiguas ya no están en pantalla.
    let excess = alive.len().saturating_sub(10);
    alive.drain(..excess);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Muestra una notificación de verdad (en desarrollo, con el id de PowerShell).
    #[test]
    #[ignore]
    fn notificacion_real() {
        let ok = show("Winkit · prueba", "Si ves esto, las notificaciones funcionan.", None, &[("Mostrar en la carpeta", OnClick::Reveal(r"C:Windows
otepad.exe".into()))]);
        assert!(ok);
    }

    #[test]
    fn escapa_el_xml() {
        assert_eq!(escape(r#"a & <b> "c" 'd'"#), "a &amp; &lt;b&gt; &quot;c&quot; &apos;d&apos;");
    }

    #[test]
    fn ida_y_vuelta_de_los_argumentos() {
        let arg = OnClick::Reveal(r"C:\Vídeos\Winkit\clip | 1.mp4".into()).encode();
        let (kind, path) = arg.split_once('|').unwrap();
        assert_eq!(kind, "reveal");
        assert_eq!(path, r"C:\Vídeos\Winkit\clip | 1.mp4");
    }
}
