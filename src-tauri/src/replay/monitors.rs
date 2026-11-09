//! Monitores conectados: nombre legible, resolución, si es el principal, si
//! tiene HDR activado y su nivel de blanco SDR.
//!
//! El id estable de cada monitor es su ruta de dispositivo de DisplayConfig
//! (`monitorDevicePath`, p. ej. `\\?\DISPLAY#DEL4123#5&…#{e6f07b5f-…}`): incluye
//! el modelo (EDID) y la instancia del conector, así que sobrevive a reinicios
//! y a reordenaciones de `\\.\DISPLAYn` (que cambian al enchufar y desenchufar).
//! Si un monitor no la tiene, se usa su nombre GDI (`\\.\DISPLAY2`).

use windows::core::{Interface, BOOL, PCWSTR};
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, DEVMODEW, ENUM_CURRENT_SETTINGS, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW,
};

#[derive(Clone, Debug)]
pub struct Display {
    pub id: String,
    pub name: String,
    pub hmonitor: HMONITOR,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    /// Blanco de lo SDR en unidades scRGB (1,0 = 80 nits). 1,0 si no se sabe.
    pub sdr_white: f32,
    left: i32,
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

/// Lo que DisplayConfig sabe de cada salida activa, por nombre GDI.
struct PathInfo {
    gdi: String,
    friendly: String,
    path: String,
    sdr_white: f32,
}

fn display_paths() -> Vec<PathInfo> {
    let mut out = Vec::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm).is_err() {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None).is_err() {
            return out;
        }
        for p in &paths[..np as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: p.sourceInfo.adapterId,
                id: p.sourceInfo.id,
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: p.targetInfo.adapterId,
                id: p.targetInfo.id,
            };
            let named = DisplayConfigGetDeviceInfo(&mut target.header) == 0;
            let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                    size: std::mem::size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                    adapterId: p.targetInfo.adapterId,
                    id: p.targetInfo.id,
                },
                SDRWhiteLevel: 0,
            };
            // SDRWhiteLevel: 1000 = 80 nits = 1,0 en scRGB.
            let sdr_white = if DisplayConfigGetDeviceInfo(&mut white.header) == 0 && white.SDRWhiteLevel >= 1000 {
                white.SDRWhiteLevel as f32 / 1000.0
            } else {
                1.0
            };
            out.push(PathInfo {
                gdi: wide(&source.viewGdiDeviceName),
                friendly: if named { wide(&target.monitorFriendlyDeviceName) } else { String::new() },
                path: if named { wide(&target.monitorDevicePath) } else { String::new() },
                sdr_white,
            });
        }
    }
    out
}

unsafe extern "system" fn collect(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    (*(data.0 as *mut Vec<HMONITOR>)).push(h);
    true.into()
}

/// Monitores activos: el principal primero y luego de izquierda a derecha.
pub fn list() -> Vec<Display> {
    let paths = display_paths();
    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut handles as *mut _ as isize));
    }
    let mut out: Vec<Display> = Vec::new();
    for h in handles {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(h, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            continue;
        }
        let gdi = wide(&info.szDevice);
        let rc = info.monitorInfo.rcMonitor;
        // Resolución real del modo actual (rcMonitor depende del DPI del proceso).
        let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        let gdi_w: Vec<u16> = gdi.encode_utf16().chain([0]).collect();
        let (width, height) = if unsafe { EnumDisplaySettingsW(PCWSTR(gdi_w.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool() {
            (mode.dmPelsWidth, mode.dmPelsHeight)
        } else {
            ((rc.right - rc.left) as u32, (rc.bottom - rc.top) as u32)
        };
        let path = paths.iter().find(|p| p.gdi.eq_ignore_ascii_case(&gdi));
        let number = gdi.trim_start_matches(|c: char| !c.is_ascii_digit()).to_string();
        let name = match path.map(|p| p.friendly.trim()).filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None if !number.is_empty() => format!("Pantalla {number}"),
            None => "Pantalla".into(),
        };
        out.push(Display {
            id: path.map(|p| p.path.clone()).filter(|p| !p.is_empty()).unwrap_or(gdi),
            name,
            hmonitor: h,
            width,
            height,
            primary: info.monitorInfo.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
            sdr_white: path.map_or(1.0, |p| p.sdr_white),
            left: rc.left,
        });
    }
    out.sort_by_key(|d| (!d.primary, d.left));
    // Dos monitores iguales: «DELL U2720Q» y «DELL U2720Q (2)».
    for i in 1..out.len() {
        let same = out[..i].iter().filter(|d| d.name == out[i].name || d.name.starts_with(&format!("{} (", out[i].name))).count();
        if same > 0 {
            out[i].name = format!("{} ({})", out[i].name, same + 1);
        }
    }
    out
}

/// El monitor elegido (`id` vacío = el principal). Si no está conectado, el
/// principal y `true` (para avisar).
pub fn resolve(id: &str) -> Option<(Display, bool)> {
    let all = list();
    if !id.is_empty() {
        if let Some(d) = all.iter().find(|d| d.id.eq_ignore_ascii_case(id)) {
            return Some((d.clone(), false));
        }
    }
    let primary = all.iter().find(|d| d.primary).or(all.first())?.clone();
    Some((primary, !id.is_empty()))
}

/// ¿Tiene el monitor el HDR de Windows activado? (Salida en ST.2084 / BT.2020.)
pub fn is_hdr(monitor: HMONITOR) -> bool {
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return false };
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            let mut o = 0;
            while let Ok(output) = adapter.EnumOutputs(o) {
                if let Ok(desc) = output.cast::<IDXGIOutput6>().and_then(|o6| o6.GetDesc1()) {
                    if desc.Monitor == monitor {
                        return desc.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
                    }
                }
                o += 1;
            }
            a += 1;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_los_monitores_de_este_equipo() {
        let all = list();
        for d in &all {
            println!("{} · {}×{} · principal {} · HDR {} · blanco SDR {:.2} · {}", d.name, d.width, d.height, d.primary, is_hdr(d.hmonitor), d.sdr_white, d.id);
        }
        assert!(!all.is_empty(), "al menos un monitor");
        assert_eq!(all.iter().filter(|d| d.primary).count(), 1);
        assert!(all[0].primary, "el principal va primero");
        let (p, fallback) = resolve("").unwrap();
        assert!(p.primary && !fallback);
        let (p, fallback) = resolve(r"\\?\DISPLAY#NOEXISTE#0").unwrap();
        assert!(p.primary && fallback, "un monitor desconectado cae en el principal");
        let (d, fallback) = resolve(&all[all.len() - 1].id).unwrap();
        assert!(!fallback && d.id == all[all.len() - 1].id);
    }
}
