//! Descargas grandes: reanudables (HTTP Range), cancelables, con progreso,
//! verificadas con SHA-256 antes de usarlas y con renombrado atómico al final.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum Error {
    Cancelled,
    /// Mensaje para el usuario.
    Failed(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Failed(format!("No se ha podido escribir en el disco: {e}"))
    }
}

/// Qué hacer con un `.part` que ya existe.
#[derive(Debug, PartialEq)]
pub enum Plan {
    Fresh,
    Resume(u64),
    /// Ya está entero: solo falta verificarlo.
    Verify,
}

pub fn plan(existing: u64, total: u64) -> Plan {
    match existing {
        0 => Plan::Fresh,
        n if n == total => Plan::Verify,
        n if n < total => Plan::Resume(n),
        // Más grande de lo esperado: no es nuestro archivo.
        _ => Plan::Fresh,
    }
}

/// ¿La respuesta continúa donde lo dejamos? 206 con `Content-Range: bytes N-…`
/// sí; un 200 trae el archivo entero desde el principio.
pub fn resumes_at(status: u16, content_range: Option<&str>, from: u64) -> Option<bool> {
    match status {
        206 => {
            let start = content_range?.strip_prefix("bytes ")?.split('-').next()?.trim().parse::<u64>().ok()?;
            (start == from).then_some(true)
        }
        200 => Some(false),
        _ => None,
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// SHA-256 de un archivo, en hexadecimal (minúsculas).
pub fn sha256_file(path: &Path, cancel: &AtomicBool) -> Result<String, Error> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        // Si la conexión se queda colgada, mejor fallar y reanudar.
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("Winkit/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// Descarga `url` en `dest` si no está ya. `progress(hecho, total)` se llama
/// como mucho cada 200 ms. Al volver con éxito, `dest` existe y su hash es el esperado.
pub async fn fetch(
    url: &str,
    dest: &Path,
    size: u64,
    sha256: &str,
    cancel: &Arc<AtomicBool>,
    progress: &(dyn Fn(u64, u64) + Send + Sync),
    verifying: &(dyn Fn() + Send + Sync),
) -> Result<(), Error> {
    if std::fs::metadata(dest).is_ok_and(|m| m.len() == size) {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(dest);
    // Dos intentos: si el .part guardado estaba mal, el segundo empieza de cero.
    for attempt in 0..2 {
        let existing = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        match plan(existing, size) {
            Plan::Verify => {}
            p => {
                let from = if let Plan::Resume(n) = p { n } else { 0 };
                download(url, &part, from, size, cancel, progress).await?;
            }
        }
        verifying();
        let (p, c) = (part.clone(), cancel.clone());
        let hash = tauri::async_runtime::spawn_blocking(move || sha256_file(&p, &c))
            .await
            .map_err(|_| Error::Failed("No se ha podido verificar la descarga.".into()))??;
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        if hash == sha256 {
            std::fs::rename(&part, dest)?;
            return Ok(());
        }
        crate::log::warn("local-ai", &format!("hash distinto en {} (intento {attempt})", dest.display()));
        let _ = std::fs::remove_file(&part);
    }
    Err(Error::Failed("La descarga ha llegado dañada. Prueba de nuevo.".into()))
}

async fn download(
    url: &str,
    part: &Path,
    from: u64,
    size: u64,
    cancel: &AtomicBool,
    progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<(), Error> {
    let mut req = client().get(url);
    if from > 0 {
        req = req.header("Range", format!("bytes={from}-"));
    }
    let res = req.send().await.map_err(|_| Error::Failed("No hay conexión para descargar.".into()))?;
    let status = res.status().as_u16();
    let range = res.headers().get("content-range").and_then(|v| v.to_str().ok()).map(String::from);
    let append = match resumes_at(status, range.as_deref(), from) {
        Some(a) => a,
        None if status == 416 => false,
        None => return Err(Error::Failed(format!("El servidor de descargas responde {status}."))),
    };
    if status == 416 {
        // El .part no encaja con el archivo del servidor: de cero en el siguiente intento.
        let _ = std::fs::remove_file(part);
        return Err(Error::Failed("No se ha podido reanudar la descarga. Prueba de nuevo.".into()));
    }
    let mut file = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(part)?;
    let mut done = if append { from } else { 0 };
    progress(done, size);
    let mut last = Instant::now();
    let mut body = res.bytes_stream();
    while let Some(chunk) = body.next().await {
        if cancel.load(Ordering::Relaxed) {
            file.flush()?;
            return Err(Error::Cancelled);
        }
        let chunk = chunk.map_err(|_| Error::Failed("Se ha cortado la descarga. Pulsa Instalar para reanudarla.".into()))?;
        file.write_all(&chunk)?;
        done += chunk.len() as u64;
        if done > size {
            drop(file);
            let _ = std::fs::remove_file(part);
            return Err(Error::Failed("La descarga es más grande de lo esperado.".into()));
        }
        if last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            progress(done, size);
        }
    }
    file.flush()?;
    progress(done, size);
    if done != size {
        return Err(Error::Failed("Se ha cortado la descarga. Pulsa Instalar para reanudarla.".into()));
    }
    Ok(())
}

/// Bytes libres en la unidad de `path` (o de su carpeta más cercana que exista).
pub fn free_space(path: &Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(p.as_os_str()), Some(&mut free), None, None).ok()? };
    Some(free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn planes_de_reanudacion() {
        assert_eq!(plan(0, 10), Plan::Fresh);
        assert_eq!(plan(4, 10), Plan::Resume(4));
        assert_eq!(plan(10, 10), Plan::Verify);
        assert_eq!(plan(11, 10), Plan::Fresh);
        assert_eq!(resumes_at(206, Some("bytes 4-9/10"), 4), Some(true));
        assert_eq!(resumes_at(206, Some("bytes 0-9/10"), 4), None);
        assert_eq!(resumes_at(200, None, 4), Some(false));
        assert_eq!(resumes_at(404, None, 0), None);
        assert_eq!(part_path(Path::new("C:/x/m.gguf")), Path::new("C:/x/m.gguf.part"));
    }

    /// Servidor HTTP mínimo que sirve `data` y entiende `Range: bytes=N-`.
    /// Con `honor_range = false` ignora Range y manda todo (200), como algunos CDN.
    fn serve(data: Vec<u8>, honor_range: bool) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/f.bin", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                h.fetch_add(1, Ordering::SeqCst);
                let mut from = 0usize;
                let mut reader = BufReader::new(s.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(r) = line.to_lowercase().strip_prefix("range: bytes=") {
                        from = r.trim().trim_end_matches('-').parse().unwrap_or(0);
                    }
                }
                let (head, body) = if from > 0 && honor_range {
                    (format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", data.len() - 1, data.len(), data.len() - from), &data[from..])
                } else {
                    (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", data.len()), &data[..])
                };
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(body);
            }
        });
        (url, hits)
    }

    fn sha(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("winkit-dl-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn run(url: &str, dest: &Path, data: &[u8], hash: &str) -> Result<(), Error> {
        let cancel = Arc::new(AtomicBool::new(false));
        tauri::async_runtime::block_on(fetch(url, dest, data.len() as u64, hash, &cancel, &|_, _| {}, &|| {}))
    }

    #[test]
    fn reanuda_verifica_y_renombra() {
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        for honor in [true, false] {
            let dir = tmp(&format!("ok-{honor}"));
            let dest = dir.join("m.gguf");
            // Media descarga anterior.
            std::fs::write(part_path(&dest), &data[..120_000]).unwrap();
            let (url, _) = serve(data.clone(), honor);
            run(&url, &dest, &data, &sha(&data)).unwrap();
            assert_eq!(std::fs::read(&dest).unwrap(), data);
            assert!(!part_path(&dest).exists());
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn un_part_danado_se_descarta_y_se_baja_de_nuevo() {
        let data: Vec<u8> = (0..50_000u32).map(|i| (i % 7) as u8).collect();
        let dir = tmp("bad-part");
        let dest = dir.join("m.gguf");
        let mut bad = data[..20_000].to_vec();
        bad[10] ^= 0xFF;
        std::fs::write(part_path(&dest), &bad).unwrap();
        let (url, hits) = serve(data.clone(), true);
        run(&url, &dest, &data, &sha(&data)).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(hits.load(Ordering::SeqCst), 2, "reanuda una vez y luego baja de cero");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn un_hash_que_no_cuadra_no_deja_nada() {
        let data = vec![1u8; 10_000];
        let dir = tmp("bad-hash");
        let dest = dir.join("m.gguf");
        let (url, _) = serve(data.clone(), true);
        let e = run(&url, &dest, &data, &"0".repeat(64)).unwrap_err();
        assert!(matches!(e, Error::Failed(m) if m.contains("dañada")));
        assert!(!dest.exists() && !part_path(&dest).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cancelar_conserva_lo_descargado() {
        let data = vec![3u8; 1_000_000];
        let dir = tmp("cancel");
        let dest = dir.join("m.gguf");
        let (url, _) = serve(data.clone(), true);
        let cancel = Arc::new(AtomicBool::new(true));
        let r = tauri::async_runtime::block_on(fetch(&url, &dest, data.len() as u64, &sha(&data), &cancel, &|_, _| {}, &|| {}));
        assert_eq!(r, Err(Error::Cancelled));
        assert!(!dest.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hay_espacio_libre() {
        assert!(free_space(&std::env::temp_dir().join("no/existe")).unwrap() > 0);
    }
}
