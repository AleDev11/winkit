//! Reproducción por la salida predeterminada de Windows con WASAPI (modo
//! compartido). Las muestras llegan a trozos mientras se sintetizan o se
//! descargan, y empiezan a sonar en cuanto hay un poco de colchón.

use super::pcm::Resampler;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

/// 200 ms de búfer en el dispositivo.
const BUFFER_100NS: i64 = 2_000_000;
/// Antes de arrancar se espera a tener este colchón (o al final del audio).
const PREBUFFER_SECONDS: f64 = 0.12;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// Un audio que se está reproduciendo. Se le van dando muestras con `push` y
/// `finish` espera a que acabe de sonar.
pub struct Playback {
    tx: Option<Sender<Vec<f32>>>,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl Playback {
    /// `rate`: frecuencia de las muestras (mono). `on_start` se llama cuando
    /// empieza a sonar de verdad. `stop` corta en seco.
    pub fn start(rate: u32, volume: f32, stop: Arc<AtomicBool>, on_start: Box<dyn FnOnce() + Send>) -> Self {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("voz-audio".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                let r = run(rate, volume.clamp(0.0, 1.0), &stop, rx, on_start);
                unsafe { CoUninitialize() };
                r
            })
            .ok();
        Self { tx: Some(tx), thread }
    }

    pub fn push(&self, samples: Vec<f32>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(samples);
        }
    }

    /// No llegan más muestras: espera a que termine de sonar (o a que se pare).
    pub fn finish(mut self) -> Result<(), String> {
        self.tx.take();
        match self.thread.take() {
            Some(t) => t.join().map_err(|_| "el hilo de audio ha fallado".to_string())?,
            None => Err("no se ha podido crear el hilo de audio".into()),
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.tx.take();
    }
}

/// Formato en el que se escribe al dispositivo.
struct Out {
    rate: u32,
    channels: usize,
    float: bool,
}

struct Device {
    client: IAudioClient,
    render: IAudioRenderClient,
    frames: u32,
    out: Out,
}

unsafe fn open(rate: u32) -> windows::core::Result<Device> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
    // Lo normal: mono en coma flotante a la frecuencia de la voz y Windows convierte.
    let want = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_IEEE_FLOAT,
        nChannels: 1,
        nSamplesPerSec: rate,
        nAvgBytesPerSec: rate * 4,
        nBlockAlign: 4,
        wBitsPerSample: 32,
        cbSize: 0,
    };
    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
    let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    let (client, out) = match client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, BUFFER_100NS, 0, &want, None) {
        Ok(()) => (client, Out { rate, channels: 1, float: true }),
        Err(_) => {
            // Sin conversión de Windows: el formato del mezclador y remuestreo propio.
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let mix = client.GetMixFormat()?;
            let f = &*mix;
            let mut tag = f.wFormatTag;
            if tag == WAVE_FORMAT_EXTENSIBLE {
                let ext = mix as *const WAVEFORMATEXTENSIBLE;
                tag = std::ptr::read_unaligned(std::ptr::addr_of!((*ext).SubFormat)).data1 as u16;
            }
            let out = Out { rate: f.nSamplesPerSec, channels: f.nChannels.max(1) as usize, float: tag == WAVE_FORMAT_IEEE_FLOAT };
            let bits = f.wBitsPerSample;
            let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, BUFFER_100NS, 0, mix, None);
            CoTaskMemFree(Some(mix as *const _));
            r?;
            if !(out.float && bits == 32) && !(!out.float && bits == 16) {
                return Err(windows::core::Error::from(windows::Win32::Foundation::E_NOTIMPL));
            }
            (client, out)
        }
    };
    let render: IAudioRenderClient = client.GetService()?;
    let frames = client.GetBufferSize()?;
    Ok(Device { client, render, frames, out })
}

fn run(rate: u32, volume: f32, stop: &AtomicBool, rx: Receiver<Vec<f32>>, on_start: Box<dyn FnOnce() + Send>) -> Result<(), String> {
    let dev = unsafe { open(rate) }.map_err(|e| {
        crate::log_warn!("voz", "no se ha podido abrir la salida de audio: {e}");
        "No se ha podido abrir la salida de audio.".to_string()
    })?;
    let mut resampler = (dev.out.rate != rate).then(|| Resampler::new(rate, dev.out.rate));
    let mut queue: VecDeque<f32> = VecDeque::new();
    let prebuffer = (dev.out.rate as f64 * PREBUFFER_SECONDS) as usize;
    let mut on_start = Some(on_start);
    let mut ended = false;
    let mut written: usize = 0;
    let mut take = |v: Vec<f32>, queue: &mut VecDeque<f32>| match resampler.as_mut() {
        Some(r) => queue.extend(r.process(&v)),
        None => queue.extend(v),
    };
    let result = (|| -> windows::core::Result<()> {
        loop {
            if stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            loop {
                match rx.try_recv() {
                    Ok(v) => take(v, &mut queue),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
            let padding = unsafe { dev.client.GetCurrentPadding()? };
            let free = dev.frames.saturating_sub(padding) as usize;
            let ready = on_start.is_none() || queue.len() >= prebuffer || ended;
            let n = free.min(queue.len());
            if ready && n > 0 {
                unsafe { write(&dev, &mut queue, n, volume)? };
                written += n;
            }
            if on_start.is_some() && written > 0 && (written >= prebuffer || ended) {
                unsafe { dev.client.Start()? };
                if let Some(f) = on_start.take() {
                    f();
                }
            }
            if ended && queue.is_empty() {
                if written == 0 {
                    return Ok(());
                }
                if padding == 0 {
                    // Lo último aún pasa por el hardware: un momento antes de parar.
                    std::thread::sleep(Duration::from_millis(40));
                    return Ok(());
                }
            }
            if queue.is_empty() && !ended {
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(v) => take(v, &mut queue),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => ended = true,
                }
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    })();
    unsafe {
        let _ = dev.client.Stop();
    }
    result.map_err(|e| {
        crate::log_warn!("voz", "fallo de la salida de audio: {e}");
        "Se ha perdido la salida de audio.".to_string()
    })
}

unsafe fn write(dev: &Device, queue: &mut VecDeque<f32>, n: usize, volume: f32) -> windows::core::Result<()> {
    let ptr = dev.render.GetBuffer(n as u32)?;
    let ch = dev.out.channels;
    if dev.out.float {
        let buf = std::slice::from_raw_parts_mut(ptr as *mut f32, n * ch);
        for frame in buf.chunks_exact_mut(ch) {
            let v = queue.pop_front().unwrap_or(0.0) * volume;
            frame.fill(v);
        }
    } else {
        let buf = std::slice::from_raw_parts_mut(ptr as *mut i16, n * ch);
        for frame in buf.chunks_exact_mut(ch) {
            let v = (queue.pop_front().unwrap_or(0.0) * volume).clamp(-1.0, 1.0);
            frame.fill((v * 32767.0) as i16);
        }
    }
    dev.render.ReleaseBuffer(n as u32, 0)
}
