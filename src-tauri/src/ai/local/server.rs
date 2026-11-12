//! El proceso de llama-server: arrancarlo en 127.0.0.1, esperar a que cargue
//! el modelo y pararlo. Va dentro de un Job object con «matar al cerrar»: si
//! Winkit muere (o lo matan), Windows se lleva el servidor y libera la VRAM.

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Lo que puede tardar en cargar un modelo grande desde un disco lento.
const LOAD_TIMEOUT: Duration = Duration::from_secs(240);

pub struct Launch {
    pub exe: PathBuf,
    pub model: PathBuf,
    /// Nombre del modelo en la API (el id del catálogo).
    pub alias: String,
    pub context: u32,
    /// Descargar capas en la GPU (Vulkan). False = solo CPU.
    pub gpu: bool,
    pub log: PathBuf,
}

pub struct Server {
    child: Child,
    pub port: u16,
    pub alias: String,
    pub gpu: bool,
    pub context: u32,
}

impl Server {
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    #[cfg(test)]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Por si se suelta sin `stop()`: nunca dejamos un servidor huérfano.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Los argumentos de llama-server. `--jinja` usa la plantilla de chat del propio
/// modelo; sin razonamiento (corregir y traducir no lo necesitan y tarda más);
/// un solo hueco y sin caché de prompts en RAM: nada de memoria de más.
pub fn args(l: &Launch, port: u16) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-m".into(),
        l.model.to_string_lossy().into(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-c".into(),
        l.context.to_string(),
        "--jinja".into(),
        "--reasoning".into(),
        "off".into(),
        "--no-webui".into(),
        "-np".into(),
        "1".into(),
        "--cache-ram".into(),
        "0".into(),
        "--offline".into(),
        "-a".into(),
        l.alias.clone(),
    ];
    if !l.gpu {
        // Sin GPU: ni se intenta (el motor Vulkan trae también los de CPU).
        a.extend(["--device".into(), "none".into(), "-ngl".into(), "0".into()]);
    }
    // Con GPU, `-ngl auto` (lo predeterminado) mete en VRAM todo lo que quepa.
    a
}

fn free_port() -> std::io::Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Un Job object para todos los servidores, abierto mientras viva Winkit.
fn job() -> Option<isize> {
    static JOB: OnceLock<Option<isize>> = OnceLock::new();
    *JOB.get_or_init(|| unsafe {
        use windows::Win32::System::JobObjects::*;
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .ok()?;
        // El handle no se cierra nunca: al morir Winkit lo cierra Windows y mata el Job.
        Some(job.0 as isize)
    })
}

fn assign_to_job(child: &Child) {
    let Some(job) = job() else { return };
    unsafe {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        if let Err(e) = AssignProcessToJobObject(HANDLE(job as _), HANDLE(child.as_raw_handle() as _)) {
            crate::log::warn("local-ai", &format!("no se ha podido meter el servidor en el Job: {e}"));
        }
    }
}

/// Las últimas líneas útiles del registro del servidor, para explicar un fallo.
pub fn log_tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .rev()
        .filter(|l| l.contains(" E ") || l.to_lowercase().contains("error") || l.to_lowercase().contains("failed"))
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| l.split_once(": ").map_or(l, |(_, m)| m).trim())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// ¿Ve el motor alguna GPU por Vulkan? (`--list-devices`, tarda menos de un segundo).
pub fn vulkan_device(exe: &Path) -> Option<String> {
    let out = Command::new(exe).arg("--list-devices").creation_flags(CREATE_NO_WINDOW).stdin(Stdio::null()).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with("Vulkan"))
        .map(|l| l.split_once(": ").map_or(l, |(_, n)| n).split(" (").next().unwrap_or(l).trim().to_string())
}

/// Arranca el servidor y espera a que el modelo esté cargado.
pub async fn start(l: &Launch) -> Result<Server, String> {
    let port = free_port().map_err(|e| e.to_string())?;
    if let Some(dir) = l.log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let log = std::fs::File::create(&l.log).map_err(|e| e.to_string())?;
    let child = Command::new(&l.exe)
        .args(args(l, port))
        .current_dir(l.exe.parent().unwrap_or(Path::new(".")))
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("no se ha podido lanzar llama-server: {e}"))?;
    assign_to_job(&child);
    let mut server = Server { child, port, alias: l.alias.clone(), gpu: l.gpu, context: l.context };
    let health = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build().unwrap_or_default();
    let t = Instant::now();
    while t.elapsed() < LOAD_TIMEOUT {
        if !server.alive() {
            let why = log_tail(&l.log);
            return Err(if why.is_empty() { "el servidor se ha cerrado al cargar el modelo".into() } else { why });
        }
        // 503 mientras carga; 200 con el modelo listo.
        if client.get(&health).send().await.is_ok_and(|r| r.status().is_success()) {
            crate::log::info("local-ai", &format!("servidor listo en {} ms (gpu: {})", t.elapsed().as_millis(), l.gpu));
            return Ok(server);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    server.stop();
    Err("el modelo tarda demasiado en cargar".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumentos_del_servidor() {
        let l = Launch {
            exe: "C:/e/llama-server.exe".into(),
            model: "C:/m/x.gguf".into(),
            alias: "gemma".into(),
            context: 4096,
            gpu: true,
            log: "C:/l.log".into(),
        };
        let a = args(&l, 1234).join(" ");
        assert!(a.contains("--host 127.0.0.1 --port 1234 -c 4096 --jinja --reasoning off"));
        assert!(!a.contains("--device none"));
        let cpu = args(&Launch { gpu: false, ..l }, 1).join(" ");
        assert!(cpu.ends_with("--device none -ngl 0"));
    }

    #[test]
    fn el_servidor_va_dentro_del_job() {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::IsProcessInJob;
        let mut child = Command::new("ping").args(["-n", "30", "127.0.0.1"]).creation_flags(CREATE_NO_WINDOW).stdout(Stdio::null()).spawn().unwrap();
        assign_to_job(&child);
        let mut inside = windows::core::BOOL(0);
        unsafe { IsProcessInJob(HANDLE(child.as_raw_handle() as _), Some(HANDLE(job().unwrap() as _)), &mut inside).unwrap() };
        let _ = child.kill();
        let _ = child.wait();
        assert!(inside.as_bool(), "el proceso no está en el Job con «matar al cerrar»");
    }

    #[test]
    fn explica_los_fallos_con_el_registro() {
        let p = std::env::temp_dir().join(format!("winkit-llama-log-{}.log", std::process::id()));
        std::fs::write(&p, "0.1 I srv x: ok\n0.2 E llama_model_load: error loading model: bad magic\n0.3 I main: exit\n").unwrap();
        assert_eq!(log_tail(&p), "error loading model: bad magic");
        let _ = std::fs::remove_file(p);
    }
}
