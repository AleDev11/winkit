//! Audio con WASAPI: el del sistema (loopback del dispositivo de salida por
//! defecto) y el del micrófono. Cada fuente entrega PCM estéreo de 16 bits a
//! 48 kHz, continuo y alineado con el reloj de la grabación: la muestra `n`
//! corresponde al instante `n / 48000` s desde el inicio. Cuando el dispositivo
//! no entrega nada (el loopback calla si no suena nada, o no hay micro) se
//! rellena con silencio para que el audio no se desfase del vídeo.

use super::encoder::SAMPLE_RATE;
use super::qpc100;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::core::{Result, PWSTR};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

/// Bloque de PCM estéreo intercalado. `start` es el índice (en muestras por
/// canal) de su primera muestra; los bloques de una pista son contiguos.
pub struct Chunk {
    pub track: usize,
    pub start: u64,
    pub data: Vec<i16>,
}

/// Margen antes de insertar silencio o recortar (20 ms).
const TOLERANCE: i64 = SAMPLE_RATE as i64 / 50;
/// Sin paquetes, se rellena con silencio cuando el retraso pasa de 100 ms…
const FILL_AFTER: i64 = SAMPLE_RATE as i64 / 10;
/// …hasta 50 ms antes de «ahora» (por si llegan paquetes con retraso).
const FILL_MARGIN: i64 = SAMPLE_RATE as i64 / 20;

pub struct AudioSource {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Hay un dispositivo abierto y entregando audio.
    pub available: Arc<AtomicBool>,
}

impl AudioSource {
    pub fn spawn(track: usize, loopback: bool, start100: i64, tx: Sender<Chunk>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let available = Arc::new(AtomicBool::new(false));
        let (s, a) = (stop.clone(), available.clone());
        let thread = std::thread::Builder::new()
            .name(if loopback { "clips-audio-sistema" } else { "clips-audio-micro" }.into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                let mut p = Pump { track, start100, produced: 0, tx };
                run(loopback, &s, &a, &mut p);
                unsafe { CoUninitialize() };
            })
            .ok();
        Self { stop, thread, available }
    }

    /// Fuente sintética para las pruebas: un seno continuo de `freq` Hz a media
    /// escala, en tiempo real y sin pasar por WASAPI.
    pub fn sine(track: usize, freq: f64, start100: i64, tx: Sender<Chunk>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let available = Arc::new(AtomicBool::new(true));
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("clips-audio-seno".into())
            .spawn(move || {
                let mut p = Pump { track, start100, produced: 0, tx };
                while !s.load(Ordering::SeqCst) {
                    let now = p.pos_at(qpc100()).max(0) as u64;
                    if now > p.produced {
                        let data = (p.produced..now)
                            .flat_map(|n| {
                                let v = ((n as f64 * freq / SAMPLE_RATE as f64 * std::f64::consts::TAU).sin() * 16_384.0) as i16;
                                [v, v]
                            })
                            .collect();
                        p.push(data);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .ok();
        Self { stop, thread, available }
    }
}

impl Drop for AudioSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Coloca el audio en la línea de tiempo de la grabación.
struct Pump {
    track: usize,
    start100: i64,
    produced: u64,
    tx: Sender<Chunk>,
}

impl Pump {
    fn pos_at(&self, qpc100: i64) -> i64 {
        ((qpc100 - self.start100) as i128 * SAMPLE_RATE as i128 / 10_000_000) as i64
    }

    fn push(&mut self, data: Vec<i16>) {
        let frames = (data.len() / 2) as u64;
        if frames == 0 {
            return;
        }
        let _ = self.tx.send(Chunk { track: self.track, start: self.produced, data });
        self.produced += frames;
    }

    fn silence(&mut self, frames: i64) {
        // En bloques de como mucho 100 ms, y nunca más de 2 s de golpe (tras una
        // suspensión el grabador se reinicia enseguida).
        let frames = frames.min(SAMPLE_RATE as i64 * 2);
        let mut left = frames;
        while left > 0 {
            let n = left.min(SAMPLE_RATE as i64 / 10);
            self.push(vec![0; n as usize * 2]);
            left -= n;
        }
    }

    /// Un paquete del dispositivo con la marca QPC de su primera muestra.
    fn packet(&mut self, mut data: Vec<i16>, qpc100: Option<i64>) {
        let frames = (data.len() / 2) as i64;
        let produced = self.produced as i64;
        let pos = match qpc100 {
            Some(q) if q > 0 => self.pos_at(q),
            _ => produced,
        };
        if pos > produced + TOLERANCE {
            self.silence(pos - produced);
        } else if pos + frames <= produced - TOLERANCE {
            return; // ya cubierto por silencio
        } else if pos < produced - TOLERANCE {
            data.drain(..((produced - pos) as usize * 2).min(data.len()));
        }
        self.push(data);
    }

    /// Sin paquetes durante un rato: silencio hasta casi «ahora».
    fn fill_gap(&mut self) {
        let now = self.pos_at(qpc100());
        let produced = self.produced as i64;
        if now - produced > FILL_AFTER {
            self.silence(now - FILL_MARGIN - produced);
        }
    }
}

fn run(loopback: bool, stop: &AtomicBool, available: &AtomicBool, p: &mut Pump) {
    let enumerator: Option<IMMDeviceEnumerator> = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() };
    let flow = if loopback { eRender } else { eCapture };
    let mut dev: Option<Device> = None;
    let mut next_open = Instant::now();
    let mut next_check = Instant::now() + Duration::from_secs(2);
    while !stop.load(Ordering::SeqCst) {
        let now = Instant::now();
        if dev.is_none() && now >= next_open {
            dev = match enumerator.as_ref().map(|e| Device::open(e, flow, loopback)) {
                Some(Ok(d)) => {
                    eprintln!("clips audio {}: abierto ({})", p.track, if d.format.is_none() { "PCM 48 kHz de Windows" } else { "conversión propia" });
                    Some(d)
                }
                Some(Err(e)) => {
                    eprintln!("clips audio {}: no se puede abrir ({})", p.track, e.message());
                    None
                }
                None => None,
            };
            available.store(dev.is_some(), Ordering::SeqCst);
            next_open = now + Duration::from_secs(2);
        }
        if let Some(d) = &mut dev {
            if let Err(e) = d.drain(p) {
                eprintln!("clips audio {}: error al leer ({})", p.track, e.message());
                // Dispositivo desconectado o invalidado: se reabre el nuevo por defecto.
                dev = None;
                available.store(false, Ordering::SeqCst);
                next_open = Instant::now();
            }
        }
        // ¿Ha cambiado el dispositivo por defecto (auriculares, HDMI…)?
        if now >= next_check {
            next_check = now + Duration::from_secs(2);
            if let (Some(d), Some(e)) = (&dev, &enumerator) {
                if default_id(e, flow).is_some_and(|id| id != d.id) {
                    eprintln!("clips audio {}: cambio de dispositivo por defecto", p.track);
                    dev = None;
                    next_open = Instant::now();
                }
            }
        }
        #[cfg(test)]
        if let Some(d) = take_stall(p.track) {
            std::thread::sleep(d); // simula un WASAPI bloqueado
        }
        p.fill_gap();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn default_id(e: &IMMDeviceEnumerator, flow: EDataFlow) -> Option<String> {
    unsafe { device_id(&e.GetDefaultAudioEndpoint(flow, eConsole).ok()?) }
}

unsafe fn device_id(d: &IMMDevice) -> Option<String> {
    let p: PWSTR = d.GetId().ok()?;
    let s = p.to_string().ok();
    CoTaskMemFree(Some(p.0 as *const _));
    s
}

/// Formato del dispositivo cuando Windows no convierte por nosotros.
struct Format {
    channels: usize,
    float: bool,
    bits: u16,
    resampler: Option<Resampler>,
}

struct Device {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    id: String,
    /// None = Windows ya entrega PCM 16 bits estéreo a 48 kHz.
    format: Option<Format>,
}

impl Device {
    fn open(e: &IMMDeviceEnumerator, flow: EDataFlow, loopback: bool) -> Result<Self> {
        unsafe {
            let device = e.GetDefaultAudioEndpoint(flow, eConsole)?;
            let id = device_id(&device).unwrap_or_default();
            let base = if loopback { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
            let want = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: 2,
                nSamplesPerSec: SAMPLE_RATE,
                nAvgBytesPerSec: SAMPLE_RATE * 4,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let flags = base | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
            let (client, format) = match client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &want, None) {
                Ok(()) => (client, None),
                Err(_) => {
                    // Un cliente que ha fallado Initialize no se puede reutilizar.
                    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
                    let mix = client.GetMixFormat()?;
                    let format = parse_format(mix);
                    let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, base, 2_000_000, 0, mix, None);
                    CoTaskMemFree(Some(mix as *const _));
                    r?;
                    (client, Some(format?))
                }
            };
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            Ok(Self { client, capture, id, format })
        }
    }

    fn drain(&mut self, p: &mut Pump) -> Result<()> {
        unsafe {
            loop {
                if self.capture.GetNextPacketSize()? == 0 {
                    return Ok(());
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                let pcm = match &mut self.format {
                    None => {
                        if silent || data.is_null() {
                            vec![0i16; frames as usize * 2]
                        } else {
                            std::slice::from_raw_parts(data as *const i16, frames as usize * 2).to_vec()
                        }
                    }
                    Some(f) => convert(f, data, frames as usize, silent),
                };
                self.capture.ReleaseBuffer(frames)?;
                p.packet(pcm, Some(qpc as i64));
            }
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

unsafe fn parse_format(f: *const WAVEFORMATEX) -> Result<Format> {
    let wf = std::ptr::read_unaligned(f);
    let tag = wf.wFormatTag as u32;
    // WAVE_FORMAT_EXTENSIBLE: el subformato (GUID) va en el byte 24; su primer
    // campo es la etiqueta clásica (1 = PCM, 3 = float).
    let real_tag = if tag == 0xFFFE && wf.cbSize >= 22 { std::ptr::read_unaligned((f as *const u8).add(24) as *const u32) } else { tag };
    let float = real_tag == 3;
    let bits = wf.wBitsPerSample;
    if !(float && bits == 32) && !(real_tag == 1 && matches!(bits, 16 | 24 | 32)) {
        return Err(windows::core::Error::from(windows::Win32::Foundation::E_NOTIMPL));
    }
    let rate = wf.nSamplesPerSec;
    Ok(Format {
        channels: wf.nChannels.max(1) as usize,
        float,
        bits,
        resampler: (rate != SAMPLE_RATE).then(|| Resampler::new(rate)),
    })
}

/// Cualquier formato del dispositivo → estéreo 16 bits a 48 kHz.
unsafe fn convert(f: &mut Format, data: *const u8, frames: usize, silent: bool) -> Vec<i16> {
    let mut stereo = Vec::with_capacity(frames);
    let bytes = (f.bits / 8) as usize;
    for i in 0..frames {
        let mut lr = [0f32; 2];
        if !silent && !data.is_null() {
            for (c, v) in lr.iter_mut().enumerate() {
                let ch = c.min(f.channels - 1);
                let p = data.add((i * f.channels + ch) * bytes);
                *v = match (f.float, f.bits) {
                    (true, _) => std::ptr::read_unaligned(p as *const f32),
                    (false, 16) => std::ptr::read_unaligned(p as *const i16) as f32 / 32768.0,
                    (false, 24) => {
                        let b = std::slice::from_raw_parts(p, 3);
                        (((b[2] as i32) << 24 | (b[1] as i32) << 16 | (b[0] as i32) << 8) >> 8) as f32 / 8_388_608.0
                    }
                    _ => std::ptr::read_unaligned(p as *const i32) as f32 / 2_147_483_648.0,
                };
            }
        }
        stereo.push(lr);
    }
    let stereo = match &mut f.resampler {
        Some(r) => r.process(&stereo),
        None => stereo,
    };
    let mut out = Vec::with_capacity(stereo.len() * 2);
    for [l, r] in stereo {
        out.push((l.clamp(-1.0, 1.0) * 32767.0) as i16);
        out.push((r.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    out
}

/// Remuestreo lineal a 48 kHz (solo si el dispositivo usa otra frecuencia y
/// Windows no ha aceptado convertir).
pub struct Resampler {
    step: f64,
    t: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(input_rate: u32) -> Self {
        Self { step: input_rate as f64 / SAMPLE_RATE as f64, t: 0.0, prev: [0.0; 2] }
    }

    pub fn process(&mut self, input: &[[f32; 2]]) -> Vec<[f32; 2]> {
        let n = input.len();
        let mut out = Vec::with_capacity((n as f64 / self.step) as usize + 2);
        // x[0] es la última muestra del bloque anterior; x[1..=n], el bloque actual.
        let x = |i: usize| if i == 0 { self.prev } else { input[i - 1] };
        while self.t < n as f64 {
            let i = self.t as usize;
            let frac = (self.t - i as f64) as f32;
            let (a, b) = (x(i), x(i + 1));
            out.push([a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]);
            self.t += self.step;
        }
        self.t -= n as f64;
        if n > 0 {
            self.prev = input[n - 1];
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn pump() -> (Pump, std::sync::mpsc::Receiver<Chunk>) {
        let (tx, rx) = channel();
        (Pump { track: 0, start100: 0, produced: 0, tx }, rx)
    }

    #[test]
    fn inserta_silencio_en_los_huecos_y_recorta_lo_repetido() {
        let (mut p, rx) = pump();
        // Paquete de 10 ms en t = 0.
        p.packet(vec![1; 960], Some(1));
        // El siguiente llega 100 ms después: 90 ms de silencio en medio.
        p.packet(vec![2; 960], Some(1_000_000));
        // Uno que se solapa 30 ms con lo ya producido: se recorta.
        let produced = p.produced as i64;
        p.packet(vec![3; 1920 * 2], Some((produced - 1440) * 10_000_000 / 48_000));
        let chunks: Vec<Chunk> = rx.try_iter().collect();
        let mut expected = 0;
        for c in &chunks {
            assert_eq!(c.start, expected, "los bloques deben ser contiguos");
            expected += c.data.len() as u64 / 2;
        }
        assert_eq!(chunks[0].data.len(), 960);
        assert_eq!(chunks[1].data.len(), (4800 - 480) * 2); // silencio
        assert!(chunks[1].data.iter().all(|&s| s == 0));
        assert_eq!(chunks[2].start, 4800);
        // 1920 muestras del tercero, menos las 1440 solapadas → 480.
        assert_eq!(chunks.last().unwrap().data.len(), 480 * 2);
        assert_eq!(p.produced, 4800 + 480 + 480);
    }

    #[test]
    fn remuestreo_conserva_la_duracion() {
        let mut r = Resampler::new(44_100);
        let mut total = 0;
        for _ in 0..100 {
            total += r.process(&vec![[0.5, -0.5]; 441]).len();
        }
        // 1 s a 44,1 kHz → 1 s a 48 kHz.
        assert!((47_990..=48_010).contains(&total), "{total}");
    }
}

/// Solo en pruebas: bloquea el hilo de una pista una vez (pista, duración).
#[cfg(test)]
pub static STALL: std::sync::Mutex<Option<(usize, Duration)>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn take_stall(track: usize) -> Option<Duration> {
    let mut s = STALL.lock().unwrap();
    match *s {
        Some((t, d)) if t == track => {
            *s = None;
            Some(d)
        }
        _ => None,
    }
}
