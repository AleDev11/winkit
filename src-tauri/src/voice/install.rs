//! Descarga de la voz local la primera vez que se activa: las DLL de
//! sherpa-onnx (con ONNX Runtime) y el modelo, desde las releases oficiales de
//! k2-fsa/sherpa-onnx en GitHub. Versión fijada, SHA-256 comprobado, descarga
//! que se reanuda (HTTP Range), cancelable, y renombrado atómico al terminar.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const SHERPA_VERSION: &str = "2.0.1";

/// Un archivo .tar.bz2 que se descarga y se descomprime en su carpeta.
#[derive(Clone, Copy)]
pub struct Asset {
    /// Para los mensajes: «Motor de voz», «Voces».
    pub label: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    /// Lo que ocupa descomprimido (para comprobar el espacio libre).
    pub unpacked: u64,
    /// Carpeta final dentro de `voice\`.
    pub dir: &'static str,
    /// Qué archivos del .tar se quedan (por nombre, sin carpetas).
    pub keep: fn(&str) -> bool,
    /// Archivos que tienen que estar para darlo por instalado.
    pub required: &'static [&'static str],
}

pub const ENGINE: Asset = Asset {
    label: "Motor de voz",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/sherpa-onnx-v2.0.1-win-x64-shared-MT-Release-lib.tar.bz2",
    sha256: "afa4de3ca4ab0afd53bb4aab40d28ee15608364b549ffc0fe0caf6489bfc39a5",
    size: 7_899_712,
    unpacked: 22_200_000,
    dir: "sherpa-onnx-2.0.1",
    keep: |name| name.ends_with(".dll") && name != "sherpa-onnx-cxx-api.dll",
    required: &["onnxruntime.dll", "onnxruntime_providers_shared.dll", "sherpa-onnx-c-api.dll"],
};

pub const MODEL: Asset = Asset {
    label: "Voces",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2",
    sha256: "82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427",
    size: 128_774_318,
    unpacked: 145_400_000,
    dir: "supertonic-3-int8-2026-05-11",
    keep: |name| name != "README.md",
    required: &[
        "duration_predictor.int8.onnx",
        "text_encoder.int8.onnx",
        "vector_estimator.int8.onnx",
        "vocoder.int8.onnx",
        "tts.json",
        "unicode_indexer.bin",
        "voice.bin",
    ],
};

pub const ASSETS: [Asset; 2] = [ENGINE, MODEL];

/// Megas que se descargan en total.
pub fn download_mb() -> u32 {
    (ASSETS.iter().map(|a| a.size).sum::<u64>() as f64 / 1_000_000.0).round() as u32
}

/// %LOCALAPPDATA%\Winkit\voice
pub fn root() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("Winkit").join("voice")
}

pub fn installed_in(root: &Path, a: &Asset) -> bool {
    let dir = root.join(a.dir);
    a.required.iter().all(|f| dir.join(f).is_file())
}

pub fn all_installed(root: &Path) -> bool {
    ASSETS.iter().all(|a| installed_in(root, a))
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Cancelled,
    Failed(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Failed(s)
    }
}

/// Progreso total: bytes descargados de todo lo que hay que descargar.
pub struct Progress<'a> {
    pub cancel: &'a AtomicBool,
    /// (hecho, total, qué se descarga). `None` en `hecho` = descomprimiendo.
    pub report: &'a (dyn Fn(Option<u64>, u64, &str) + Send + Sync),
}

fn part_path(root: &Path, a: &Asset) -> PathBuf {
    root.join("downloads").join(format!("{}.tar.bz2.part", a.dir))
}

/// Descarga e instala lo que falte. Lo ya instalado no se vuelve a bajar.
pub async fn install(root: &Path, assets: &[Asset], p: &Progress<'_>) -> Result<(), Error> {
    let pending: Vec<Asset> = assets.iter().copied().filter(|a| !installed_in(root, a)).collect();
    if pending.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(root.join("downloads")).map_err(|e| e.to_string())?;
    // Espacio: lo que falta por bajar + descomprimido + margen.
    let have = |a: &Asset| std::fs::metadata(part_path(root, a)).map(|m| m.len()).unwrap_or(0).min(a.size);
    let need: u64 = pending.iter().map(|a| a.size - have(a) + a.unpacked).sum::<u64>() + 64_000_000;
    if let Some(free) = free_space(root) {
        if free < need {
            return Err(Error::Failed(format!(
                "No hay espacio suficiente: hacen falta unos {} MB libres y quedan {} MB.",
                need / 1_000_000,
                free / 1_000_000
            )));
        }
    }
    let total: u64 = pending.iter().map(|a| a.size).sum();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .user_agent(concat!("Winkit/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut done_before = 0u64;
    for a in &pending {
        let part = part_path(root, a);
        let base = done_before;
        download(&client, a.url, &part, a.size, p.cancel, &|got| (p.report)(Some(base + got), total, a.label)).await?;
        if p.cancel.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        (p.report)(None, total, a.label);
        let (root2, a2, part2) = (root.to_path_buf(), *a, part.clone());
        tauri::async_runtime::spawn_blocking(move || verify_and_unpack(&root2, &a2, &part2))
            .await
            .map_err(|e| Error::Failed(e.to_string()))??;
        done_before += a.size;
        crate::log_info!("voz", "instalado {} ({} MB)", a.dir, a.size / 1_000_000);
    }
    let _ = std::fs::remove_dir(root.join("downloads"));
    Ok(())
}

/// Descarga `url` en `part`, siguiendo desde lo que ya hubiera.
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    size: u64,
    cancel: &AtomicBool,
    progress: &(dyn Fn(u64) + Send + Sync),
) -> Result<(), Error> {
    let mut have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > size {
        let _ = std::fs::remove_file(part);
        have = 0;
    }
    if have == size {
        progress(have);
        return Ok(());
    }
    let mut req = client.get(url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let resp = req.send().await.map_err(|_| Error::Failed("No se ha podido conectar para descargar la voz.".into()))?;
    let status = resp.status();
    let append = match status.as_u16() {
        206 => true,
        200 => false, // el servidor no reanuda: desde cero
        416 if have > 0 => {
            // Ya estaba todo (o el archivo cambió): se comprueba el hash al terminar.
            progress(have);
            return Ok(());
        }
        _ => return Err(Error::Failed(format!("La descarga ha fallado (HTTP {}).", status.as_u16()))),
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(|e| Error::Failed(format!("No se puede escribir la descarga: {e}")))?;
    let mut got = if append { have } else { 0 };
    progress(got);
    let mut stream = resp.bytes_stream();
    let mut last_report = got;
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = file.flush();
            return Err(Error::Cancelled);
        }
        // Sin datos en 30 s se da por cortada (se reanudará en el siguiente intento).
        let next = match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
            Ok(n) => n,
            Err(_) => return Err(Error::Failed("La descarga se ha quedado parada. Vuelve a intentarlo para seguir.".into())),
        };
        match next {
            None => break,
            Some(Ok(bytes)) => {
                file.write_all(&bytes).map_err(|e| Error::Failed(format!("No se puede escribir la descarga: {e}")))?;
                got += bytes.len() as u64;
                if got - last_report >= 256 * 1024 || got >= size {
                    last_report = got;
                    progress(got);
                }
            }
            Some(Err(_)) => {
                let _ = file.flush();
                return Err(Error::Failed("Se ha cortado la descarga. Vuelve a intentarlo para seguir donde se quedó.".into()));
            }
        }
    }
    file.flush().map_err(|e| Error::Failed(e.to_string()))?;
    if got < size {
        return Err(Error::Failed("Se ha cortado la descarga. Vuelve a intentarlo para seguir donde se quedó.".into()));
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Comprueba el hash, descomprime en `<dir>.tmp` y lo renombra a `<dir>`.
pub fn verify_and_unpack(root: &Path, a: &Asset, part: &Path) -> Result<(), Error> {
    let hash = sha256_file(part)?;
    if !hash.eq_ignore_ascii_case(a.sha256) {
        let _ = std::fs::remove_file(part);
        crate::log_warn!("voz", "hash incorrecto en {}: {hash}", a.dir);
        return Err(Error::Failed("La descarga ha llegado dañada. Vuelve a intentarlo.".into()));
    }
    let tmp = root.join(format!("{}.tmp", a.dir));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    if let Err(e) = unpack(part, &tmp, a.keep) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(Error::Failed(e));
    }
    if let Some(missing) = a.required.iter().find(|f| !tmp.join(f).is_file()) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(Error::Failed(format!("Al paquete descargado le falta {missing}.")));
    }
    let dest = root.join(a.dir);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| format!("No se ha podido terminar la instalación: {e}"))?;
    let _ = std::fs::remove_file(part);
    Ok(())
}

/// Extrae los archivos de un .tar.bz2 en `dest`, sin carpetas.
pub fn unpack(archive: &Path, dest: &Path, keep: fn(&str) -> bool) -> Result<u64, String> {
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let bz = bzip2::read::BzDecoder::new(std::io::BufReader::with_capacity(1 << 20, f));
    let mut tar = tar::Archive::new(bz);
    let mut bytes = 0u64;
    for entry in tar.entries().map_err(|e| format!("Paquete dañado: {e}"))? {
        let mut entry = entry.map_err(|e| format!("Paquete dañado: {e}"))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        // Solo nombres simples (nada de rutas raras dentro del .tar).
        if name.contains(['\\', '/', ':']) || name.starts_with('.') || !keep(&name) {
            continue;
        }
        let mut out = std::fs::File::create(dest.join(&name)).map_err(|e| e.to_string())?;
        bytes += std::io::copy(&mut entry, &mut out).map_err(|e| format!("Paquete dañado: {e}"))?;
    }
    Ok(bytes)
}

/// Borra lo descargado (instalado y a medias).
pub fn remove_all(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(root).map_err(|e| format!("No se ha podido borrar la voz local: {e}"))
}

pub fn free_space(path: &Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    // La carpeta puede no existir aún: se sube hasta una que sí.
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(p.as_os_str()), Some(&mut free), None, None) }.ok()?;
    Some(free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    /// Servidor HTTP mínimo con Range. La primera petición se corta a la mitad.
    fn server(body: Vec<u8>) -> (String, Arc<AtomicUsize>, Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x.tar.bz2", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (h, r) = (hits.clone(), ranges.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut range = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("range: bytes=") {
                        range = v.trim().trim_end_matches('-').parse::<usize>().ok();
                    }
                }
                let n = h.fetch_add(1, Ordering::SeqCst);
                r.lock().unwrap().push(range.map(|s| s.to_string()).unwrap_or_default());
                let start = range.unwrap_or(0);
                let slice = &body[start..];
                let head = if range.is_some() {
                    format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\nConnection: close\r\n\r\n",
                        slice.len(),
                        body.len() - 1,
                        body.len()
                    )
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", slice.len())
                };
                let _ = stream.write_all(head.as_bytes());
                // La primera vez solo se manda la mitad y se cierra (conexión cortada).
                let send = if n == 0 { &slice[..slice.len() / 2] } else { slice };
                let _ = stream.write_all(send);
                let _ = stream.flush();
            }
        });
        (url, hits, ranges)
    }

    fn tar_bz2(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, format!("paquete/lib/{name}"), *data).unwrap();
        }
        let raw = tar.into_inner().unwrap();
        let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    }

    fn sha(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("winkit-voz-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn descarga_reanuda_y_verifica() {
        let body: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let (url, hits, ranges) = server(body.clone());
        let dir = tempdir("reanuda");
        let part = dir.join("a.part");
        let client = reqwest::Client::new();
        let cancel = AtomicBool::new(false);
        let seen = std::sync::Mutex::new(0u64);
        let progress = |n: u64| *seen.lock().unwrap() = n;
        // Primer intento: el servidor corta a la mitad.
        let first = tauri::async_runtime::block_on(download(&client, &url, &part, body.len() as u64, &cancel, &progress));
        assert!(matches!(first, Err(Error::Failed(_))), "{first:?}");
        let half = std::fs::metadata(&part).unwrap().len();
        assert_eq!(half, body.len() as u64 / 2);
        // Segundo: pide solo lo que falta (Range) y completa.
        tauri::async_runtime::block_on(download(&client, &url, &part, body.len() as u64, &cancel, &progress)).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert_eq!(*seen.lock().unwrap(), body.len() as u64);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert_eq!(ranges.lock().unwrap().as_slice(), ["".to_string(), half.to_string()]);
        assert_eq!(sha256_file(&part).unwrap(), sha(&body));
        // Completo: no vuelve a pedir nada.
        tauri::async_runtime::block_on(download(&client, &url, &part, body.len() as u64, &cancel, &progress)).unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        // Cancelado antes de empezar.
        let _ = std::fs::remove_file(&part);
        cancel.store(true, Ordering::SeqCst);
        let r = tauri::async_runtime::block_on(download(&client, &url, &part, body.len() as u64, &cancel, &progress));
        assert_eq!(r, Err(Error::Cancelled));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verifica_y_descomprime_atomico() {
        let dir = tempdir("descomprime");
        let archive = tar_bz2(&[("uno.dll", b"hola"), ("dos.txt", b"fuera"), ("tres.dll", b"adios")]);
        const REQ: &[&str] = &["uno.dll", "tres.dll"];
        let asset = Asset {
            label: "Prueba",
            url: "",
            sha256: Box::leak(sha(&archive).into_boxed_str()),
            size: archive.len() as u64,
            unpacked: 0,
            dir: "paquete",
            keep: |n| n.ends_with(".dll"),
            required: REQ,
        };
        let part = dir.join("p.part");
        std::fs::write(&part, &archive).unwrap();
        verify_and_unpack(&dir, &asset, &part).unwrap();
        assert!(installed_in(&dir, &asset));
        assert_eq!(std::fs::read(dir.join("paquete").join("uno.dll")).unwrap(), b"hola");
        assert!(!dir.join("paquete").join("dos.txt").exists());
        assert!(!dir.join("paquete.tmp").exists());
        assert!(!part.exists(), "el .part se borra al terminar");

        // Hash incorrecto: no se instala nada y se borra la descarga dañada.
        let bad = Asset { sha256: "00", dir: "otro", ..asset };
        std::fs::write(&part, &archive).unwrap();
        assert!(matches!(verify_and_unpack(&dir, &bad, &part), Err(Error::Failed(_))));
        assert!(!part.exists());
        assert!(!dir.join("otro").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn espacio_libre() {
        assert!(free_space(&std::env::temp_dir().join("no-existe").join("tampoco")).unwrap() > 0);
    }
}
