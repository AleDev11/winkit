//! Une los segmentos elegidos en un solo MP4 sin recodificar: SourceReader
//! (muestras comprimidas) → SinkWriter en modo «passthrough», con las marcas
//! de tiempo desplazadas para que sean continuas.

use super::ring::Segment;
use std::path::Path;
use windows::core::{Result, HSTRING};
use windows::Win32::Media::MediaFoundation::*;

fn reader(path: &Path) -> Result<IMFSourceReader> {
    unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None) }
}

fn stream_types(r: &IMFSourceReader) -> Vec<IMFMediaType> {
    (0..16).map_while(|i| unsafe { r.GetNativeMediaType(i, 0) }.ok()).collect()
}

fn is_video(t: &IMFMediaType) -> bool {
    unsafe { t.GetGUID(&MF_MT_MAJOR_TYPE) }.is_ok_and(|g| g == MFMediaType_Video)
}

/// Lee todas las muestras de todas las pistas (por defecto el lector solo
/// activa la primera de audio, y aquí puede haber dos).
fn read_all(r: &IMFSourceReader, streams: usize, mut f: impl FnMut(u32, i64, IMFSample) -> Result<()>) -> Result<()> {
    unsafe { r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, true)? };
    let mut done = vec![false; streams];
    let mut guard = 0;
    while done.iter().any(|d| !d) {
        let mut stream = 0u32;
        let mut flags = 0u32;
        let mut ts = 0i64;
        let mut sample = None;
        unsafe {
            r.ReadSample(MF_SOURCE_READER_ANY_STREAM.0 as u32, 0, Some(&mut stream), Some(&mut flags), Some(&mut ts), Some(&mut sample))?
        };
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            match done.get_mut(stream as usize) {
                Some(d) if !*d => *d = true,
                _ => {
                    guard += 1;
                    if guard > 16 {
                        break;
                    }
                }
            }
            continue;
        }
        if let Some(s) = sample {
            f(stream, ts, s)?;
        }
    }
    Ok(())
}

fn is_keyframe(s: &IMFSample) -> bool {
    unsafe { s.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0
}

/// Marca de tiempo (relativa al segmento) del último fotograma clave en o
/// antes de `cut`; si no hay ninguno antes, el primero.
fn keyframe_before(path: &Path, video: u32, cut: i64) -> Result<i64> {
    let r = reader(path)?;
    unsafe {
        r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        r.SetStreamSelection(video, true)?;
    }
    let mut best: Option<i64> = None;
    loop {
        let mut flags = 0u32;
        let mut ts = 0i64;
        let mut sample = None;
        unsafe { r.ReadSample(video, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))? };
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(s) = sample else { continue };
        if !is_keyframe(&s) {
            continue;
        }
        if ts <= cut || best.is_none() {
            best = Some(ts);
        }
        if ts >= cut {
            break;
        }
    }
    Ok(best.unwrap_or(0))
}

/// Clip guardado: duración y dónde caen las uniones entre segmentos (100 ns,
/// desde el inicio del clip).
#[derive(Debug)]
pub struct Clip {
    #[allow(dead_code)]
    pub duration: i64,
    #[allow(dead_code)]
    pub joins: Vec<i64>,
    /// Dónde empieza el clip en el reloj de la grabación (100 ns).
    #[allow(dead_code)]
    pub start: i64,
}

/// Escribe en `out` los segmentos `segs` desde `cut` (reloj de la época).
pub fn remux(segs: &[Segment], cut: i64, out: &Path) -> std::result::Result<Clip, String> {
    let r = run(segs, cut, out).map_err(|e| format!("No se ha podido guardar el clip ({}).", e.message()));
    if r.is_err() {
        let _ = std::fs::remove_file(out);
    }
    r
}

fn run(segs: &[Segment], cut: i64, out: &Path) -> Result<Clip> {
    let first = segs.first().ok_or_else(|| windows::core::Error::from(MF_E_INVALIDREQUEST))?;
    let types = stream_types(&reader(&first.path)?);
    let video = types.iter().position(is_video).ok_or_else(|| windows::core::Error::from(MF_E_INVALIDMEDIATYPE))? as u32;
    // El clip empieza en un fotograma clave: como mucho 2 s antes del corte.
    let keyframe = first.start + keyframe_before(&first.path, video, (cut - first.start).max(0))?;
    // El clip arranca en la trama AAC (rejilla de 1024 muestras del reloj de la
    // época) que contiene ese fotograma clave: el audio empieza justo en 0 y el
    // vídeo como mucho 21 ms después. Si el bucle se ha saltado fotogramas, el
    // codificador pone los clave fuera de la rejilla (cuenta fotogramas, no
    // tiempo), y empezar el audio a mitad de trama hace que el MP4 de Media
    // Foundation reescriba sus duraciones y pierda tramas.
    let frame = super::encoder::AAC_FRAME as i128;
    let grid = (keyframe as i128 * 48_000 / 10_000_000).div_euclid(frame) * frame;
    let clip_start = (grid * 10_000_000 / 48_000) as i64;

    // El lector no enumera las pistas en el orden del archivo: se añaden en
    // orden fijo, vídeo y luego el audio del sistema (192 kbps) antes que el
    // del micrófono (128 kbps).
    let mut order: Vec<usize> = (0..types.len()).collect();
    order.sort_by_key(|&i| {
        let bytes = unsafe { types[i].GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND) }.unwrap_or(0);
        (!is_video(&types[i]), std::cmp::Reverse(bytes), i)
    });
    let writer = unsafe { MFCreateSinkWriterFromURL(&HSTRING::from(out.as_os_str()), None, None)? };
    let mut out_index = vec![0u32; types.len()];
    for &i in &order {
        unsafe {
            out_index[i] = writer.AddStream(&types[i])?;
            writer.SetInputMediaType(out_index[i], &types[i], None)?;
        }
    }
    unsafe { writer.BeginWriting()? };

    let mut end = 0i64;
    for seg in segs {
        let r = reader(&seg.path)?;
        if stream_types(&r).len() != types.len() {
            continue; // no debería pasar dentro de una época
        }
        let mut started_video = false;
        read_all(&r, types.len(), |stream, ts, s| {
            // Las marcas van en unidades de 100 ns y las tramas AAC no son un
            // número entero de ellas: ±0,5 ms de redondeo cuentan como 0.
            let t = seg.start + ts - clip_start;
            if t < -5_000 {
                return Ok(());
            }
            let t = t.max(0);
            if stream == video && !started_video {
                // El primer fotograma que se escribe de cada segmento tiene que ser clave.
                if !is_keyframe(&s) {
                    return Ok(());
                }
                started_video = true;
            }
            unsafe {
                let _ = s.DeleteItem(&MFSampleExtension_Discontinuity);
                s.SetSampleTime(t)?;
                end = end.max(t + s.GetSampleDuration().unwrap_or(0));
                writer.WriteSample(out_index[stream as usize], &s)
            }
        })?;
    }
    unsafe { writer.Finalize()? };
    let joins = segs.iter().skip(1).map(|s| s.start - clip_start).filter(|&t| t > 0).collect();
    Ok(Clip { duration: end, joins, start: clip_start })
}

/// Lo que se ve al abrir un MP4 (para comprobar los clips en las pruebas).
#[cfg(test)]
pub struct Probe {
    pub duration: i64,
    pub video_streams: usize,
    pub audio_streams: usize,
    pub width: u32,
    pub height: u32,
    /// Por pista: tipo ("vídeo" o kbps del AAC).
    pub kinds: Vec<String>,
    /// Por pista: (primera marca, final, nº de muestras, mayor salto entre muestras).
    pub streams: Vec<(i64, i64, usize, i64)>,
}

#[cfg(test)]
pub fn probe(path: &Path) -> Result<Probe> {
    let r = reader(path)?;
    let types = stream_types(&r);
    let video_streams = types.iter().filter(|t| is_video(t)).count();
    let audio_streams = types
        .iter()
        .filter(|t| unsafe { t.GetGUID(&MF_MT_MAJOR_TYPE) }.is_ok_and(|g| g == MFMediaType_Audio))
        .count();
    let size = types.iter().find(|t| is_video(t)).and_then(|t| unsafe { t.GetUINT64(&MF_MT_FRAME_SIZE) }.ok()).unwrap_or(0);
    let mut streams = vec![(i64::MAX, 0i64, 0usize, 0i64); types.len()];
    let mut duration = 0i64;
    read_all(&r, types.len(), |i, ts, s| {
        let end = ts + unsafe { s.GetSampleDuration() }.unwrap_or(0);
        let st = &mut streams[i as usize];
        if st.2 > 0 {
            st.3 = st.3.max(ts - st.1);
        }
        st.0 = st.0.min(ts);
        st.1 = st.1.max(end);
        st.2 += 1;
        duration = duration.max(end);
        Ok(())
    })?;
    let kinds = types
        .iter()
        .map(|t| if is_video(t) { "vídeo".into() } else { format!("audio {} kbps", unsafe { t.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND) }.unwrap_or(0) * 8 / 1000) })
        .collect();
    Ok(Probe { duration, video_streams, audio_streams, width: (size >> 32) as u32, height: size as u32, kinds, streams })
}

/// Decodifica la primera pista de audio a PCM estéreo de 16 bits a 48 kHz,
/// colocando cada bloque según su marca de tiempo (los huecos quedan a cero).
/// Devuelve también cuántas muestras faltaban por huecos en las marcas.
#[cfg(test)]
pub fn decode_audio(path: &Path) -> Result<(Vec<i16>, u64)> {
    let r = reader(path)?;
    let types = stream_types(&r);
    let audio = types
        .iter()
        .position(|t| unsafe { t.GetGUID(&MF_MT_MAJOR_TYPE) }.is_ok_and(|g| g == MFMediaType_Audio))
        .ok_or_else(|| windows::core::Error::from(MF_E_INVALIDMEDIATYPE))? as u32;
    // Si hay dos pistas, la del sistema (más kbps) es la que interesa.
    let audio = types
        .iter()
        .enumerate()
        .filter(|(_, t)| unsafe { t.GetGUID(&MF_MT_MAJOR_TYPE) }.is_ok_and(|g| g == MFMediaType_Audio))
        .max_by_key(|(_, t)| unsafe { t.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND) }.unwrap_or(0))
        .map_or(audio, |(i, _)| i as u32);
    unsafe {
        r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        r.SetStreamSelection(audio, true)?;
        let pcm = MFCreateMediaType()?;
        pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
        pcm.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        pcm.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
        pcm.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        pcm.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
        pcm.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000)?;
        r.SetCurrentMediaType(audio, None, &pcm)?;
    }
    let mut out: Vec<i16> = Vec::new();
    let mut missing = 0u64;
    loop {
        let mut flags = 0u32;
        let mut ts = 0i64;
        let mut sample = None;
        unsafe { r.ReadSample(audio, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))? };
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(s) = sample else { continue };
        unsafe {
            let buf = s.ConvertToContiguousBuffer()?;
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            buf.Lock(&mut ptr, None, Some(&mut len))?;
            let data = std::slice::from_raw_parts(ptr as *const i16, len as usize / 2).to_vec();
            buf.Unlock()?;
            let at = (ts.max(0) as i128 * 48_000 / 10_000_000) as usize * 2;
            // Tolerancia de una muestra por redondeo de las marcas.
            if at > out.len() + 2 {
                println!("  hueco en el audio decodificado: {:.3} s, {:.2} ms", out.len() as f64 / 96_000.0, (at - out.len()) as f64 / 96.0);
                missing += ((at - out.len()) / 2) as u64;
                out.resize(at, 0);
            }
            out.extend_from_slice(&data);
        }
    }
    Ok((out, missing))
}
