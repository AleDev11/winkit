//! Codificación de cada segmento con el `IMFSinkWriter` de Media Foundation:
//! H.264 con el codificador por hardware de la GPU (NVENC, AMF, Quick Sync)
//! si lo hay, y AAC para cada pista de audio.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use windows::core::{implement, Interface, Ref, Result, GUID, HSTRING};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

/// Tasa de bits del vídeo: la de referencia a 1080p60 escalada por píxeles × fps.
pub fn bitrate(quality: &str, width: u32, height: u32, fps: u32) -> u32 {
    let reference = match quality {
        "low" => 6_000_000.0,
        "medium" => 10_000_000.0,
        _ => 16_000_000.0,
    };
    let scale = (width as f64 * height as f64 * fps as f64) / (1920.0 * 1080.0 * 60.0);
    (reference * scale).clamp(1_500_000.0, 100_000_000.0) as u32
}

/// Bytes por segundo de AAC: 192 kbps para el sistema, 128 kbps para el micro.
pub const AAC_SYSTEM: u32 = 24_000;
pub const AAC_MIC: u32 = 16_000;
pub const SAMPLE_RATE: u32 = 48_000;

#[derive(Clone)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    /// Fotogramas entre fotogramas clave (múltiplo de `aac_aligned_step`).
    pub gop: u32,
    /// Una entrada por pista de audio, en orden: el tipo AAC que sale del
    /// codificador continuo de esa pista (los segmentos lo copian tal cual).
    pub audio: Vec<AudioType>,
}

pub struct SegmentWriter {
    pub writer: IMFSinkWriter,
    pub video: u32,
    pub audio: Vec<u32>,
    pub path: PathBuf,
    pub encoder: String,
}

// Media Foundation es libre de hilos: el escritor se crea y se finaliza en el
// hilo de E/S y se usa en el de grabación.
unsafe impl Send for SegmentWriter {}

fn attrs(n: u32) -> Result<IMFAttributes> {
    let mut a = None;
    unsafe { MFCreateAttributes(&mut a, n)? };
    Ok(a.unwrap())
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn video_type(cfg: &EncoderConfig, subtype: &GUID, output: bool) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(cfg.width, cfg.height))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(cfg.fps, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
        t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
        if output {
            t.SetUINT32(&MF_MT_AVG_BITRATE, cfg.bitrate)?;
            t.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
        }
        Ok(t)
    }
}

fn audio_type(aac_bytes: Option<u32>) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE)?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        match aac_bytes {
            Some(b) => {
                t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
                t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, b)?;
            }
            None => {
                t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
                t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, SAMPLE_RATE * 4)?;
                t.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
            }
        }
        Ok(t)
    }
}

/// Crea el escritor de un segmento, ya listo para recibir muestras. Prueba
/// primero con codificadores por hardware y, si falla, por software.
pub fn create(path: &Path, cfg: &EncoderConfig, manager: &IMFDXGIDeviceManager) -> std::result::Result<SegmentWriter, String> {
    match create_with(path, cfg, manager, true) {
        Ok(w) => Ok(w),
        Err(hw) => {
            let _ = std::fs::remove_file(path);
            create_with(path, cfg, manager, false).map_err(|sw| {
                let _ = std::fs::remove_file(path);
                format!("No se ha podido iniciar el codificador de vídeo ({}; {}).", hw.message(), sw.message())
            })
        }
    }
}

fn create_with(path: &Path, cfg: &EncoderConfig, manager: &IMFDXGIDeviceManager, hardware: bool) -> Result<SegmentWriter> {
    unsafe {
        let a = attrs(4)?;
        a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, hardware as u32)?;
        a.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, manager)?;
        // Menos fotogramas retenidos dentro del codificador (y sin fotogramas B).
        a.SetUINT32(&MF_LOW_LATENCY, 1)?;
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &a)?;

        let video = writer.AddStream(&video_type(cfg, &MFVideoFormat_H264, true)?)?;
        let input = video_type(cfg, &MFVideoFormat_NV12, false)?;
        // Un fotograma clave cada ~2 s: el clip puede empezar en cualquiera de ellos.
        let params = attrs(2)?;
        params.SetUINT32(&CODECAPI_AVEncMPVGOPSize, cfg.gop)?;
        params.SetUINT32(&CODECAPI_AVEncMPVDefaultBPictureCount, 0)?;
        if writer.SetInputMediaType(video, &input, &params).is_err() {
            writer.SetInputMediaType(video, &input, None)?;
        }

        let mut audio = Vec::new();
        for t in &cfg.audio {
            // Mismo tipo de entrada y salida: el escritor no recodifica.
            let idx = writer.AddStream(&t.0)?;
            writer.SetInputMediaType(idx, &t.0, None)?;
            audio.push(idx);
        }
        writer.BeginWriting()?;
        let encoder = encoder_name(&writer, video);
        Ok(SegmentWriter { writer, video, audio, path: path.to_path_buf(), encoder })
    }
}

/// Nombre legible del codificador de vídeo que ha elegido Media Foundation.
fn encoder_name(writer: &IMFSinkWriter, stream: u32) -> String {
    let Ok(ex) = writer.cast::<IMFSinkWriterEx>() else { return "desconocido".into() };
    for i in 0..8 {
        let mut category = GUID::zeroed();
        let mut transform = None;
        if unsafe { ex.GetTransformForStream(stream, i, Some(&mut category), &mut transform) }.is_err() {
            break;
        }
        if category != MFT_CATEGORY_VIDEO_ENCODER {
            continue;
        }
        let Some(attrs) = transform.and_then(|t| unsafe { t.GetAttributes() }.ok()) else { break };
        let vendor = get_string(&attrs, &MFT_ENUM_HARDWARE_VENDOR_ID_Attribute).unwrap_or_default().to_uppercase();
        let name = get_string(&attrs, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default();
        return friendly_encoder(&vendor, &name);
    }
    "desconocido".into()
}

pub fn friendly_encoder(vendor: &str, name: &str) -> String {
    let lower = name.to_lowercase();
    if vendor.contains("10DE") || lower.contains("nvidia") {
        "NVIDIA NVENC".into()
    } else if vendor.contains("1002") || lower.contains("amd") {
        "AMD AMF".into()
    } else if vendor.contains("8086") || lower.contains("intel") {
        "Intel Quick Sync".into()
    } else if !vendor.is_empty() {
        if name.is_empty() { "por hardware".into() } else { name.into() }
    } else {
        "por software (CPU)".into()
    }
}

fn get_string(attrs: &IMFAttributes, key: &GUID) -> Option<String> {
    unsafe {
        let len = attrs.GetStringLength(key).ok()?;
        let mut buf = vec![0u16; len as usize + 1];
        attrs.GetString(key, &mut buf, None).ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Devuelve el índice de la textura al grupo cuando el codificador suelta la muestra.
#[implement(IMFAsyncCallback)]
struct Recycle {
    idx: usize,
    free: Arc<Mutex<Vec<usize>>>,
}

impl IMFAsyncCallback_Impl for Recycle_Impl {
    fn GetParameters(&self, _flags: *mut u32, _queue: *mut u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Invoke(&self, _result: Ref<IMFAsyncResult>) -> Result<()> {
        self.free.lock().unwrap().push(self.idx);
        Ok(())
    }
}

/// Envía una textura NV12 al codificador. La textura `idx` vuelve a `free`
/// cuando Media Foundation deja de usarla (aunque falle la escritura).
pub fn write_video(
    w: &SegmentWriter,
    tex: &ID3D11Texture2D,
    idx: usize,
    free: &Arc<Mutex<Vec<usize>>>,
    time: i64,
    duration: i64,
) -> Result<()> {
    unsafe {
        let sample = match MFCreateVideoSampleFromSurface(None) {
            Ok(s) => s,
            Err(e) => {
                free.lock().unwrap().push(idx);
                return Err(e);
            }
        };
        let tracked: IMFTrackedSample = sample.cast()?;
        let cb: IMFAsyncCallback = Recycle { idx, free: free.clone() }.into();
        tracked.SetAllocator(&cb, None)?;
        let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, tex, 0, false)?;
        let len = buffer.cast::<IMF2DBuffer>()?.GetContiguousLength()?;
        buffer.SetCurrentLength(len)?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(time)?;
        sample.SetSampleDuration(duration)?;
        w.writer.WriteSample(w.video, &sample)
    }
}

/// Tipo de medio AAC compartido entre hilos (Media Foundation es libre de hilos).
#[derive(Clone)]
pub struct AudioType(pub IMFMediaType);
unsafe impl Send for AudioType {}
unsafe impl Sync for AudioType {}

/// Muestras por trama AAC.
pub const AAC_FRAME: i64 = 1024;

/// Un codificador AAC para toda la sesión de grabación de una pista. Así el
/// flujo AAC es continuo y los segmentos solo reparten sus tramas: al unirlos
/// no hay «priming» ni arranque del MDCT en cada unión (lo que daba un hueco
/// de ~21 ms cada 15 s con un codificador nuevo por segmento).
pub struct AacEncoder {
    mft: IMFTransform,
    pub output: AudioType,
    out_size: u32,
    fed: i64,
    frames: i64,
}

/// Una trama AAC ya codificada y la muestra (reloj de la grabación) en que empieza.
pub struct AacFrame {
    pub start: i64,
    pub sample: IMFSample,
}

impl AacEncoder {
    /// Retardo del codificador AAC de Windows: la trama n contiene el audio
    /// que empieza en la muestra (n − 1)·1024; la primera es solo «priming».
    /// Tras codificar y decodificar con Media Foundation quedan ~176 muestras
    /// (3,7 ms) más de retraso, inapreciables (medido en la prueba
    /// `audio_continuo_en_las_uniones`).
    pub const DELAY: i64 = AAC_FRAME;

    pub fn new(bytes_per_second: u32) -> Result<Self> {
        unsafe {
            let mft: IMFTransform = CoCreateInstance(&AACMFTEncoder, None, CLSCTX_INPROC_SERVER)?;
            let input = audio_type(None)?;
            let output = audio_type(Some(bytes_per_second))?;
            output.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
            mft.SetInputType(0, &input, 0)?;
            mft.SetOutputType(0, &output, 0)?;
            let output = mft.GetOutputCurrentType(0)?;
            let out_size = mft.GetOutputStreamInfo(0)?.cbSize.max(8192);
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            Ok(Self { mft, output: AudioType(output), out_size, fed: 0, frames: 0 })
        }
    }

    /// Codifica PCM estéreo de 16 bits a 48 kHz (contiguo con lo anterior) y
    /// devuelve las tramas que estén listas.
    pub fn push(&mut self, pcm: &[i16]) -> Result<Vec<AacFrame>> {
        let mut out = Vec::new();
        if pcm.is_empty() {
            return Ok(out);
        }
        unsafe {
            let bytes = (pcm.len() * 2) as u32;
            let buffer = MFCreateMemoryBuffer(bytes)?;
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr() as *const u8, ptr, bytes as usize);
            buffer.Unlock()?;
            buffer.SetCurrentLength(bytes)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            let frames = pcm.len() as i64 / 2;
            sample.SetSampleTime(frames_to_hns(self.fed))?;
            sample.SetSampleDuration(frames_to_hns(frames))?;
            loop {
                match self.mft.ProcessInput(0, &sample, 0) {
                    Ok(()) => break,
                    // Tiene salida pendiente: sacarla y volver a intentarlo.
                    Err(e) if e.code() == MF_E_NOTACCEPTING => self.drain(&mut out)?,
                    Err(e) => return Err(e),
                }
            }
            self.fed += frames;
            self.drain(&mut out)?;
        }
        Ok(out)
    }

    fn drain(&mut self, out: &mut Vec<AacFrame>) -> Result<()> {
        unsafe {
            loop {
                let buffer = MFCreateMemoryBuffer(self.out_size)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buffer)?;
                let mut data = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: std::mem::ManuallyDrop::new(Some(sample)),
                    dwStatus: 0,
                    pEvents: std::mem::ManuallyDrop::new(None),
                }];
                let mut status = 0u32;
                let r = self.mft.ProcessOutput(0, &mut data, &mut status);
                let [mut d] = data;
                let sample = std::mem::ManuallyDrop::take(&mut d.pSample);
                std::mem::ManuallyDrop::drop(&mut d.pEvents);
                match r {
                    Ok(()) => {
                        let Some(sample) = sample else { continue };
                        if sample.GetTotalLength().unwrap_or(0) == 0 {
                            continue;
                        }
                        let start = self.frames * AAC_FRAME - Self::DELAY;
                        self.frames += 1;
                        sample.SetSampleDuration(frames_to_hns(AAC_FRAME))?;
                        out.push(AacFrame { start, sample });
                    }
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

/// Escribe una trama AAC ya codificada en un segmento (sin recodificar).
pub fn write_aac(w: &SegmentWriter, track: usize, sample: &IMFSample, time: i64) -> Result<()> {
    unsafe {
        sample.SetSampleTime(time)?;
        w.writer.WriteSample(w.audio[track], sample)
    }
}

/// Cada cuántos fotogramas de vídeo coincide el inicio de un fotograma con el
/// de una trama AAC (1024 muestras a 48 kHz): 16 a 30 fps, 32 a 60 fps. Los
/// cortes de segmento y los fotogramas clave caen solo ahí, así el audio de
/// cada segmento y de cada clip empieza justo en 0 y en la rejilla de tramas.
/// (Con desfases de fracciones de trama, el MP4 de Media Foundation reescribe
/// las duraciones y pierde tramas al unir.)
pub fn aac_aligned_step(fps: u32) -> u32 {
    let per_frame = SAMPLE_RATE / fps; // 1600 o 800, exacto para 30 y 60
    let g = gcd(per_frame, AAC_FRAME as u32);
    AAC_FRAME as u32 / g
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// GOP de ~2 s alineado con las tramas AAC (2,13 s a 30 y a 60 fps).
pub fn aligned_gop(fps: u32) -> u32 {
    let step = aac_aligned_step(fps);
    ((fps * 2 + step / 2) / step).max(1) * step
}

/// Muestras (por canal) a unidades de 100 ns.
pub fn frames_to_hns(frames: i64) -> i64 {
    frames * 10_000_000 / SAMPLE_RATE as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_de_referencia_a_1080p60() {
        assert_eq!(bitrate("low", 1920, 1080, 60), 6_000_000);
        assert_eq!(bitrate("medium", 1920, 1080, 60), 10_000_000);
        assert_eq!(bitrate("high", 1920, 1080, 60), 16_000_000);
    }

    #[test]
    fn bitrate_escala_con_pixeles_y_fps() {
        assert_eq!(bitrate("high", 1920, 1080, 30), 8_000_000);
        assert_eq!(bitrate("medium", 3840, 2160, 60), 40_000_000);
        let qhd = bitrate("high", 2560, 1440, 60);
        assert!((28_000_000..=28_500_000).contains(&qhd), "{qhd}");
        // Límites razonables en los extremos.
        assert_eq!(bitrate("low", 640, 360, 30), 1_500_000);
        assert_eq!(bitrate("high", 7680, 4320, 60), 100_000_000);
        // Calidad desconocida = alta.
        assert_eq!(bitrate("???", 1920, 1080, 60), 16_000_000);
    }

    #[test]
    fn cortes_alineados_con_las_tramas_aac() {
        assert_eq!(aac_aligned_step(30), 16);
        assert_eq!(aac_aligned_step(60), 32);
        assert_eq!(aligned_gop(30), 64);
        assert_eq!(aligned_gop(60), 128);
        for fps in [30, 60] {
            let s = aac_aligned_step(fps) as i64;
            assert_eq!(s * SAMPLE_RATE as i64 / fps as i64 % AAC_FRAME, 0);
            assert_eq!(aligned_gop(fps) as i64 * SAMPLE_RATE as i64 / fps as i64 % AAC_FRAME, 0);
        }
    }

    #[test]
    fn nombres_de_codificador() {
        assert_eq!(friendly_encoder("VEN_10DE", "NVIDIA H.264 Encoder MFT"), "NVIDIA NVENC");
        assert_eq!(friendly_encoder("VEN_1002", "AMDh264Encoder"), "AMD AMF");
        assert_eq!(friendly_encoder("VEN_8086", "Intel® Quick Sync Video H.264 Encoder MFT"), "Intel Quick Sync");
        assert_eq!(friendly_encoder("", "H264 Encoder MFT"), "por software (CPU)");
    }
}
