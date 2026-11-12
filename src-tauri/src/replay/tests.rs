//! Prueba de verdad (graba la pantalla de esta máquina ~25 s). No corre con
//! `cargo test` normal:
//!   cargo test --lib replay::tests -- --ignored --nocapture

use super::recorder::{Config, Recorder, Shared};
use super::{remux, ring};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// COM (MTA) y Media Foundation en el hilo de la prueba, para leer los MP4.
fn mf_init() {
    use windows::Win32::Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL, MF_VERSION};
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
}

/// Borra la carpeta de la prueba al terminar, también si falla (salvo con
/// WINKIT_KEEP_CLIP, para mirar los archivos).
struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if std::env::var_os("WINKIT_KEEP_CLIP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn cpu_time() -> Duration {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let (mut a, mut b, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u).unwrap() };
    let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    Duration::from_nanos((t(k) + t(u)) * 100)
}

#[test]
#[ignore]
fn graba_de_verdad_y_guarda_un_clip() {
    mf_init();
    let dir = std::env::temp_dir().join(format!("winkit-clips-test-{}", std::process::id()));
    let _cleanup = Cleanup(dir.clone());
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);
    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let t0 = Instant::now();
    let cpu0 = cpu_time();
    let cfg = Config { fps: 60, quality: "high".into(), system_audio: true, microphone: true, segment_secs: 10, dir: buffer.clone(), monitor: String::new(), force_hdr_path: false, synthetic_audio: None, system_audio_device: String::new(), microphone_device: String::new() };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));

    let info = loop {
        let info = shared.info.lock().unwrap().clone();
        match info.state {
            "recording" => break info,
            "error" => panic!("no arranca: {:?}", info.error),
            _ if t0.elapsed() > Duration::from_secs(20) => panic!("no arranca a tiempo"),
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let startup = t0.elapsed();
    // A mitad, el hilo del audio del sistema se bloquea 6 s cruzando un cambio
    // de segmento (como WASAPI al cambiar de dispositivo): no debe desfasarse.
    std::thread::sleep(Duration::from_secs(17));
    *super::audio::STALL.lock().unwrap() = Some((0, Duration::from_secs(6)));
    std::thread::sleep(Duration::from_secs(8));
    let cpu = cpu_time() - cpu0;
    let wall = t0.elapsed();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());

    let segments_before = std::fs::read_dir(&buffer).unwrap().count();
    let out = dir.join("clip.mp4");
    let ts = Instant::now();
    rec.handle().save(15, &out).expect("guardar");
    let save_time = ts.elapsed();
    let p = remux::probe(&out).expect("abrir el MP4");
    let size = std::fs::metadata(&out).unwrap().len();

    // La grabación sigue tras guardar: un segundo clip corto.
    std::thread::sleep(Duration::from_secs(4));
    let out2 = dir.join("clip2.mp4");
    rec.handle().save(5, &out2).expect("guardar el segundo");
    let p2 = remux::probe(&out2).expect("abrir el segundo MP4");


    let mic = info.microphone.as_ref().map(|m| m.load(std::sync::atomic::Ordering::SeqCst));
    println!("codificador: {:?}", info.encoder);
    println!("micrófono disponible: {mic:?}");
    println!("arranque: {startup:?}; guardado: {save_time:?}; segmentos en el búfer antes de guardar: {segments_before}");
    println!(
        "CPU: {:.1}% de un núcleo ({:.2}% del total con {cores} núcleos) durante {:.1} s",
        cpu.as_secs_f64() / wall.as_secs_f64() * 100.0,
        cpu.as_secs_f64() / wall.as_secs_f64() * 100.0 / cores as f64,
        wall.as_secs_f64()
    );
    println!(
        "clip: {:.2} s, {}×{}, {} vídeo + {} audio, {:.1} MB ({:.1} Mbps)",
        p.duration as f64 / 1e7,
        p.width,
        p.height,
        p.video_streams,
        p.audio_streams,
        size as f64 / 1e6,
        size as f64 * 8.0 / (p.duration as f64 / 1e7) / 1e6
    );
    let seg = std::fs::read_dir(&buffer).ok().and_then(|mut d| d.find_map(|e| e.ok().map(|e| e.path())));
    for (i, (first, end, n, gap)) in p.streams.iter().enumerate() {
        println!(
            "  pista {i} ({}): {:.3}–{:.3} s, {n} muestras, mayor hueco {:.1} ms",
            p.kinds[i],
            *first as f64 / 1e7,
            *end as f64 / 1e7,
            *gap as f64 / 1e4
        );
    }
    let seg_kinds = seg.and_then(|s| remux::probe(&s).ok()).map(|s| s.kinds).expect("segmento del búfer");
    println!("segundo clip: {:.2} s", p2.duration as f64 / 1e7);
    // Sin cortes en las uniones de segmentos: ni huecos de vídeo ni de audio,
    // y las pistas de audio cubren lo mismo que el vídeo.
    let v = p.kinds.iter().position(|k| k == "vídeo").expect("pista de vídeo");
    let (v_first, v_end, _, v_gap) = p.streams[v];
    assert!(v_gap <= 500_000, "hueco de vídeo de {} ms", v_gap / 10_000);
    for (i, &(first, end, _, gap)) in p.streams.iter().enumerate().filter(|(i, _)| *i != v) {
        assert!(gap <= 500_000, "hueco de audio de {} ms en la pista {i}", gap / 10_000);
        assert!((first - v_first).abs() <= 1_000_000 && (end - v_end).abs() <= 1_000_000, "audio desfasado en la pista {i}");
    }
    // Mismo orden de pistas en el archivo que los segmentos, que se escriben
    // como vídeo, sistema (192 kbps), micrófono (128 kbps). (El lector de MF
    // las enumera al revés, de ahí que se compare con un segmento.)
    assert_eq!(p.kinds, seg_kinds, "orden de las pistas");
    assert_eq!(p2.kinds, seg_kinds, "orden de las pistas del segundo clip");

    assert_eq!(p.video_streams, 1);
    assert_eq!(p.audio_streams, 2, "sistema + micrófono");
    assert!((p.duration - 15 * 10_000_000).abs() <= 2 * 10_000_000, "duración {}", p.duration);
    assert!((p2.duration - 5 * 10_000_000).abs() <= 2 * 10_000_000, "duración {}", p2.duration);
    drop(rec);
    if std::env::var_os("WINKIT_KEEP_CLIP").is_some() {
        println!("se conserva {}", dir.display());
        return;
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Continuidad del audio en las uniones entre segmentos: un seno sintético
/// continuo atraviesa varias uniones; se decodifica el clip y se mide el
/// mayor salto entre muestras y el mayor silencio cerca de cada unión.
///   cargo test --lib replay::tests::audio_continuo -- --ignored --nocapture
#[test]
#[ignore]
fn audio_continuo_en_las_uniones() {
    mf_init();
    // 40 Hz: periodo de 1200 muestras, así el desfase medido no es ambiguo.
    const FREQ: f64 = 40.0;
    let dir = std::env::temp_dir().join(format!("winkit-clips-seno-{}", std::process::id()));
    let _cleanup = Cleanup(dir.clone());
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);
    // Mejor uno con otra resolución que el principal: así se nota si se graba el bueno.
    let all = super::monitors::list();
    let primary = all.iter().find(|d| d.primary).expect("monitor principal").clone();
    let other = all
        .iter()
        .find(|d| !d.primary && (d.width, d.height) != (primary.width, primary.height))
        .or(all.last())
        .unwrap()
        .clone();
    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let cfg = Config {
        fps: 30,
        quality: "low".into(),
        system_audio: true,
        microphone: false,
        segment_secs: 4,
        dir: buffer.clone(),
        // De paso: otro monitor (el último de la lista) y la ruta FP16 + mapeo
        // de tonos forzada, para probar ambas de punta a punta.
        monitor: other.id.clone(),
        system_audio_device: String::new(),
        microphone_device: String::new(),
        force_hdr_path: true,
        synthetic_audio: Some(FREQ),
    };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));
    let t0 = Instant::now();
    while shared.info.lock().unwrap().state != "recording" {
        assert!(t0.elapsed() < Duration::from_secs(60), "no arranca");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(15));
    let out = dir.join("seno.mp4");
    let clip = rec.handle().save(12, &out).expect("guardar");
    drop(rec);
    let mut segs: Vec<_> = std::fs::read_dir(&buffer).unwrap().flatten().map(|e| e.path()).collect();
    segs.sort();
    for s in &segs {
        if let Ok(p) = remux::probe(s) {
            let r: Vec<String> = p.streams.iter().zip(&p.kinds).map(|(st, k)| format!("{k} {:.3}–{:.3} ({} muestras)", st.0 as f64 / 1e7, st.1 as f64 / 1e7, st.2)).collect();
            println!("  {}: {}", s.file_name().unwrap().to_string_lossy(), r.join(" · "));
        }
    }
    if let Ok(p) = remux::probe(&out) {
        for (st, k) in p.streams.iter().zip(&p.kinds) {
            println!("  clip {k}: {:.3}–{:.3} ({} muestras, mayor hueco {:.1} ms)", st.0 as f64 / 1e7, st.1 as f64 / 1e7, st.2, st.3 as f64 / 1e4);
        }
    }
    let (pcm, missing) = remux::decode_audio(&out).expect("decodificar");
    let left: Vec<i32> = pcm.iter().step_by(2).map(|&v| v as i32).collect();

    // Un seno de amplitud 16384 cambia como mucho 16384·2π·f/48000 por muestra.
    let max_step = 16_384.0 * std::f64::consts::TAU * FREQ / 48_000.0;
    let stats = |from: usize, to: usize| {
        let (from, to) = (from.max(1), to.min(left.len()));
        let mut jump = 0i32;
        let (mut run, mut longest) = (0usize, 0usize);
        for i in from..to {
            jump = jump.max((left[i] - left[i - 1]).abs());
            // Un seno solo pasa unas pocas muestras cerca de cero; más de 1 ms es un hueco.
            if left[i].abs() < 800 {
                run += 1;
                longest = longest.max(run);
            } else {
                run = 0;
            }
        }
        (jump, longest)
    };
    println!("clip {:.2} s, {} muestras, {missing} perdidas por huecos en las marcas", clip.duration as f64 / 1e7, left.len());
    println!("salto máximo esperable de un seno: {max_step:.0}");
    let mut worst = (0i32, 0usize);
    for j in &clip.joins {
        let at = (*j as i128 * 48_000 / 10_000_000) as usize;
        let (jump, silence) = stats(at.saturating_sub(4_800), at + 4_800);
        println!("  unión en {:.3} s: salto máximo {jump}, silencio más largo {:.2} ms", *j as f64 / 1e7, silence as f64 / 48.0);
        worst = (worst.0.max(jump), worst.1.max(silence));
    }
    // Retardo del audio respecto a la imagen: se compara con el seno ideal (la
    // muestra n de la grabación vale sin(2π·f·n/48000)) buscando el desfase.
    let base = clip.start as f64 * 48_000.0 / 1e7;
    let ideal = |n: f64| (n * FREQ / 48_000.0 * std::f64::consts::TAU).sin() * 16_384.0;
    let lag = (-590i32..=590)
        .min_by_key(|&lag| {
            (24_000..72_000usize)
                .map(|i| (left[i] as f64 - ideal(base + i as f64 - lag as f64)).powi(2))
                .sum::<f64>() as u64
        })
        .unwrap();
    println!("retardo del audio: {lag} muestras ({:.2} ms)", lag as f64 / 48.0);

    // Lejos de las uniones (referencia: el ruido propio del AAC).
    let (ref_jump, ref_silence) = stats(4_800, left.len().saturating_sub(4_800));
    println!("clip entero (sin los primeros 100 ms): salto máximo {ref_jump}, silencio más largo {:.2} ms", ref_silence as f64 / 48.0);
    println!("PEOR EN LAS UNIONES: salto {} ({:.2}× lo esperable), silencio {:.2} ms", worst.0, worst.0 as f64 / max_step, worst.1 as f64 / 48.0);

    let p = remux::probe(&out).expect("abrir el clip");
    println!("monitor grabado: {} ({}×{}), clip {}×{}", other.name, other.width, other.height, p.width, p.height);
    assert_eq!((p.width, p.height), (other.width & !1, other.height & !1), "no se ha grabado el monitor elegido");
    assert!(clip.joins.len() >= 2, "hacen falta al menos dos uniones: {:?}", clip.joins);
    assert_eq!(missing, 0, "huecos en las marcas de tiempo del audio");
    // Codificador + decodificador AAC de Windows dejan ~3,7 ms de retraso fijo
    // (inapreciable; compensarlo rompería la alineación con las tramas).
    assert!(lag.abs() <= 240, "el audio va desfasado {lag} muestras");
    assert!((worst.0 as f64) < max_step * 1.5, "discontinuidad en una unión: salto {}", worst.0);
    assert!(worst.1 < 48, "silencio de {:.2} ms en una unión", worst.1 as f64 / 48.0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// La ruta HDR (captura FP16 + mapeo de tonos) aplicada a un monitor SDR debe
/// dar lo mismo que la captura normal de 8 bits.
///   cargo test --lib replay::tests::fp16 -- --ignored --nocapture
#[test]
#[ignore]
fn fp16_con_mapeo_de_tonos_igual_que_8_bits() {
    use super::capture::{self, Capture, Gpu};
    use super::monitors;
    use super::tonemap::ToneMapper;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    mf_init();

    fn grab(cap: &Capture, gpu: &Gpu, dst: &ID3D11Texture2D) {
        let t0 = Instant::now();
        while cap.grab_latest(gpu, dst).unwrap().is_none() {
            assert!(t0.elapsed() < Duration::from_secs(5), "no llega ningún fotograma");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn readback(gpu: &Gpu, tex: &ID3D11Texture2D, w: u32, h: u32) -> Vec<u8> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        unsafe {
            let mut staging = None;
            gpu.device.CreateTexture2D(&desc, None, Some(&mut staging)).unwrap();
            let staging = staging.unwrap();
            gpu.context.CopyResource(&staging, tex);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            gpu.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map)).unwrap();
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts((map.pData as *const u8).add(y * map.RowPitch as usize), w as usize * 4);
                out.extend_from_slice(row);
            }
            gpu.context.Unmap(&staging, 0);
            out
        }
    }

    let gpu = capture::create_gpu().unwrap();
    let (d, _) = monitors::resolve("").unwrap();
    let hdr = monitors::is_hdr(d.hmonitor);
    let cap8 = Capture::start(&gpu, d.hmonitor, 30, false).unwrap();
    let cap16 = Capture::start(&gpu, d.hmonitor, 30, true).unwrap();
    let (w, h) = (cap8.width, cap8.height);
    let bind = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
    let a = capture::texture(&gpu, w, h, DXGI_FORMAT_B8G8R8A8_UNORM, bind).unwrap();
    let b = capture::texture(&gpu, w, h, DXGI_FORMAT_B8G8R8A8_UNORM, bind).unwrap();
    let tm = ToneMapper::new(&gpu, w, h, &b, if hdr { d.sdr_white } else { 1.0 }).unwrap();
    let t = Instant::now();
    grab(&cap8, &gpu, &a);
    grab(&cap16, &gpu, &tm.input);
    tm.render(&gpu);
    let pa = readback(&gpu, &a, w, h);
    let pb = readback(&gpu, &b, w, h);
    println!("monitor {} {w}×{h}, HDR {hdr}; capturas en {:?}", d.name, t.elapsed());

    let mut sum = [0u64; 3];
    let mut max = [0u8; 3];
    let mut over = 0usize;
    for (x, y) in pa.chunks_exact(4).zip(pb.chunks_exact(4)) {
        let mut big = false;
        for c in 0..3 {
            let diff = x[c].abs_diff(y[c]);
            sum[c] += diff as u64;
            max[c] = max[c].max(diff);
            big |= diff > 8;
        }
        over += big as usize;
    }
    let n = (w * h) as f64;
    let mean = sum.map(|s| s as f64 / n);
    println!(
        "diferencia media por canal (B, G, R): {:.2} {:.2} {:.2} niveles; máxima {:?}; píxeles con más de 8 niveles: {:.3} %",
        mean[0], mean[1], mean[2], max, over as f64 / n * 100.0
    );
    assert!(mean.iter().all(|&m| m <= 3.0), "la ruta FP16 se aparta de la de 8 bits: {mean:?}");
}

/// Comprueba un clip guardado: duración, vídeo sin cortes salvo `max_video_gap`
/// (100 ns), audio que cubre lo mismo que el vídeo y un seno de `freq` Hz sin
/// saltos ni huecos al decodificarlo.
fn check_clip(out: &std::path::Path, keep: i64, freq: f64, max_video_gap: i64) -> Result<String, String> {
    let p = remux::probe(out).map_err(|e| e.message())?;
    let v = p.kinds.iter().position(|k| k == "vídeo").ok_or("sin vídeo")?;
    let (v_first, v_end, _, v_gap) = p.streams[v];
    let secs = p.duration as f64 / 1e7;
    if !(keep as f64 - 0.1..=keep as f64 + 2.5).contains(&secs) {
        return Err(format!("duración {secs:.2} s para {keep} s"));
    }
    if v_gap > max_video_gap {
        return Err(format!("hueco de vídeo de {} ms", v_gap / 10_000));
    }
    for (i, &(first, end, _, gap)) in p.streams.iter().enumerate().filter(|(i, _)| *i != v) {
        // ±0,1 ms de redondeo de las marcas (100 ns) no es un hueco.
        if gap > 1_000 || (first - v_first).abs() > 1_000_000 || (end - v_end).abs() > 1_000_000 {
            return Err(format!("pista {i} de audio: {first}–{end} (vídeo {v_first}–{v_end}), hueco {gap}"));
        }
    }
    let (pcm, missing) = remux::decode_audio(out).map_err(|e| e.message())?;
    let left: Vec<i32> = pcm.iter().step_by(2).map(|&x| x as i32).collect();
    let max_step = 16_384.0 * std::f64::consts::TAU * freq / 48_000.0;
    let jump = left.windows(2).skip(4_800).map(|w| (w[1] - w[0]).abs()).max().unwrap_or(0);
    if missing > 0 || jump as f64 > max_step * 1.5 {
        return Err(format!("audio: {missing} muestras perdidas, salto {jump}"));
    }
    Ok(format!("{secs:.2} s, vídeo con hueco máx. {} ms, salto de audio {jump}", v_gap / 10_000))
}

/// Bajo carga: todos los núcleos al 100 % y bloqueos artificiales del bucle de
/// grabación de vez en cuando; 10 guardados en ~2 min. Cada uno tiene que
/// contestar en < 3 s con un clip correcto. Pensada para `--release`:
///   cargo test --release --lib replay::tests::bajo_carga -- --ignored --nocapture
#[test]
#[ignore]
fn bajo_carga_guarda_siempre_a_tiempo() {
    use std::sync::atomic::{AtomicBool, Ordering};
    mf_init();
    const FREQ: f64 = 40.0;
    let dir = std::env::temp_dir().join(format!("winkit-clips-carga-{}", std::process::id()));
    let _cleanup = Cleanup(dir.clone());
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);

    // Quemador de CPU: un hilo ocupado por núcleo.
    let burn = Arc::new(AtomicBool::new(true));
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let burners: Vec<_> = (0..cores)
        .map(|i| {
            let b = burn.clone();
            std::thread::spawn(move || {
                let mut x = i as u64 + 1;
                while b.load(Ordering::Relaxed) {
                    for _ in 0..10_000 {
                        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    }
                    std::hint::black_box(x);
                }
            })
        })
        .collect();

    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let cfg = Config {
        fps: 60,
        quality: "high".into(),
        system_audio: true,
        microphone: false,
        segment_secs: 15,
        dir: buffer.clone(),
        monitor: String::new(),
        system_audio_device: String::new(),
        microphone_device: String::new(),
        force_hdr_path: false,
        synthetic_audio: Some(FREQ),
    };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));
    let t0 = Instant::now();
    while shared.info.lock().unwrap().state != "recording" {
        assert!(t0.elapsed() < Duration::from_secs(60), "no arranca");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(12));

    let stalls = [0u64, 300, 0, 800, 150, 0, 600, 0, 900, 250];
    let mut latencies = Vec::new();
    let mut failures = Vec::new();
    for (i, &stall) in stalls.iter().enumerate() {
        std::thread::sleep(Duration::from_millis(6_000 + (i as u64 * 1_337) % 5_000));
        if stall > 0 {
            *super::recorder::LOOP_STALL.lock().unwrap() = Some(Duration::from_millis(stall));
            std::thread::sleep(Duration::from_millis(30)); // que el guardado caiga en pleno bloqueo
        }
        let out = dir.join(format!("carga-{i}.mp4"));
        let ts = Instant::now();
        let r = rec.handle().save(10, &out);
        let lat = ts.elapsed();
        latencies.push(lat);
        let verdict = match r {
            Ok(_) => check_clip(&out, 10, FREQ, (stall as i64 + 100) * 10_000),
            Err(e) => Err(e),
        };
        println!("guardado {i}: {lat:?} (bloqueo {stall} ms) → {verdict:?}");
        if lat >= Duration::from_secs(3) || verdict.is_err() {
            failures.push(format!("guardado {i}: {lat:?} {verdict:?}"));
        }
        let _ = std::fs::remove_file(&out);
    }
    burn.store(false, Ordering::Relaxed);
    for b in burners {
        let _ = b.join();
    }
    drop(rec);
    let mut sorted = latencies.clone();
    sorted.sort();
    println!(
        "latencias con {cores} núcleos ocupados: mín {:?}, mediana {:?}, máx {:?}",
        sorted[0],
        sorted[sorted.len() / 2],
        sorted[sorted.len() - 1]
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Reinicios a mitad de grabación: (1) el bucle se para 3,5 s (como tras una
/// suspensión) con un hilo de audio colgado; (2) el bucle se cuelga 8 s y el
/// perro guardián lo abandona. En ambos, guardar durante el reinicio contesta
/// al momento con el mensaje de reinicio y la grabación vuelve sola.
///   cargo test --lib replay::tests::reinicio -- --ignored --nocapture
#[test]
#[ignore]
fn reinicio_contesta_al_momento_y_vuelve_a_grabar() {
    use super::recorder::{Phase, LOOP_STALL, RESTARTING, STARTING};
    mf_init();
    const FREQ: f64 = 40.0;
    let dir = std::env::temp_dir().join(format!("winkit-clips-reinicio-{}", std::process::id()));
    let _cleanup = Cleanup(dir.clone());
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);
    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let cfg = Config {
        fps: 30,
        quality: "low".into(),
        system_audio: true,
        microphone: true,
        segment_secs: 15,
        dir: buffer.clone(),
        monitor: String::new(),
        system_audio_device: String::new(),
        microphone_device: String::new(),
        force_hdr_path: false,
        synthetic_audio: Some(FREQ),
    };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));
    let wait_recording = |what: &str| {
        let t = Instant::now();
        while shared.health.phase() != Phase::Recording {
            assert!(t.elapsed() < Duration::from_secs(20), "no vuelve a grabar ({what})");
            std::thread::sleep(Duration::from_millis(20));
        }
        t.elapsed()
    };
    let is_restart_msg = |e: &str| e == RESTARTING || e == STARTING;
    wait_recording("arranque");
    std::thread::sleep(Duration::from_secs(6));

    // 1) Audio del micrófono colgado 8 s y el bucle parado 3,5 s: salto > 3 s → reinicio.
    *super::audio::STALL.lock().unwrap() = Some((1, Duration::from_secs(8)));
    std::thread::sleep(Duration::from_millis(300));
    *LOOP_STALL.lock().unwrap() = Some(Duration::from_millis(3_500));
    std::thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    let r1 = rec.handle().save(5, &dir.join("a.mp4"));
    let lat1 = t.elapsed();
    println!("guardar con el bucle parado: {lat1:?} → {r1:?}");
    assert!(matches!(&r1, Err(e) if is_restart_msg(e)), "{r1:?}");
    assert!(lat1 < Duration::from_secs(4), "{lat1:?}");
    // Ya reiniciando (con el hilo de audio aún colgado): al momento.
    let t = Instant::now();
    let r2 = rec.handle().save(5, &dir.join("b.mp4"));
    let lat2 = t.elapsed();
    println!("guardar durante el reinicio: {lat2:?} → {r2:?}");
    assert!(matches!(&r2, Err(e) if is_restart_msg(e)), "{r2:?}");
    assert!(lat2 < Duration::from_millis(200), "{lat2:?}");
    let back = wait_recording("tras el salto");
    println!("vuelve a grabar {back:?} después");
    std::thread::sleep(Duration::from_secs(6));
    let out = dir.join("c.mp4");
    let t = Instant::now();
    rec.handle().save(4, &out).expect("guardar tras el reinicio");
    println!("guardar tras el reinicio: {:?} → {:?}", t.elapsed(), check_clip(&out, 4, FREQ, 1_000_000));
    check_clip(&out, 4, FREQ, 1_000_000).unwrap();

    // 2) Bucle colgado 8 s: el perro guardián (5 s) lo abandona y arranca otro.
    *LOOP_STALL.lock().unwrap() = Some(Duration::from_secs(8));
    let stalled = Instant::now();
    // Mientras está colgado (aún «grabando»), guardar espera… pero no más de 8 s.
    std::thread::sleep(Duration::from_millis(1_000));
    let t = Instant::now();
    let r3 = rec.handle().save(5, &dir.join("d0.mp4"));
    println!("guardar con el bucle colgado: {:?} → {r3:?}", t.elapsed());
    assert!(matches!(&r3, Err(e) if is_restart_msg(e)), "{r3:?}");
    let detected = stalled.elapsed();
    println!("el perro guardián actúa a los {detected:?} del cuelgue");
    assert!(detected >= Duration::from_secs(5) && detected < Duration::from_millis(6_500), "{detected:?}");
    let t = Instant::now();
    let r4 = rec.handle().save(5, &dir.join("d.mp4"));
    let lat4 = t.elapsed();
    println!("guardar justo después del perro guardián: {lat4:?} → {r4:?}");
    assert!(matches!(&r4, Err(e) if is_restart_msg(e)), "{r4:?}");
    assert!(lat4 < Duration::from_millis(200), "{lat4:?}");
    let back = wait_recording("tras el perro guardián");
    println!("vuelve a grabar {back:?} después");
    std::thread::sleep(Duration::from_secs(6));
    let out = dir.join("e.mp4");
    let t = Instant::now();
    rec.handle().save(4, &out).expect("guardar tras el perro guardián");
    println!("guardar tras el perro guardián: {:?} → {:?}", t.elapsed(), check_clip(&out, 4, FREQ, 1_000_000));
    check_clip(&out, 4, FREQ, 1_000_000).unwrap();

    let t = Instant::now();
    drop(rec);
    println!("parar: {:?}", t.elapsed());
    assert!(t.elapsed() < Duration::from_secs(8));
    // El hilo abandonado termina solo al despertar; que no deje nada atrás.
    std::thread::sleep(Duration::from_secs(3));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Graba de un micrófono concreto que no es el predeterminado (si este equipo
/// tiene alguno) y comprueba que se ha usado ese, que la pista dura lo que
/// toca y que tiene señal. También prueba el medidor de nivel de Ajustes.
///   cargo test --lib replay::tests::microfono_elegido -- --ignored --nocapture
#[test]
#[ignore]
fn microfono_elegido_no_predeterminado() {
    mf_init();
    let inputs = super::audio::endpoints(true);
    for d in &inputs {
        println!("entrada: {}{}", d.name, if d.is_default { " (predeterminada)" } else { "" });
    }
    let Some(chosen) = inputs.iter().find(|d| !d.is_default) else {
        println!("este equipo no tiene micrófonos aparte del predeterminado: nada que probar");
        return;
    };
    println!("se elige: {}", chosen.name);

    // Medidor: picos durante 1 s.
    let mut peaks = Vec::new();
    for _ in 0..10 {
        peaks.push(super::audio::mic_level(&chosen.id));
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("medidor (picos cada 100 ms): {:?}", peaks.iter().map(|p| format!("{p:.4}")).collect::<Vec<_>>());

    let dir = std::env::temp_dir().join(format!("winkit-clips-micro-{}", std::process::id()));
    let _cleanup = Cleanup(dir.clone());
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);
    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let cfg = Config {
        fps: 30,
        quality: "low".into(),
        system_audio: false,
        microphone: true,
        segment_secs: 15,
        dir: buffer.clone(),
        monitor: String::new(),
        system_audio_device: String::new(),
        microphone_device: chosen.id.clone(),
        force_hdr_path: false,
        synthetic_audio: None,
    };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));
    let t0 = Instant::now();
    while shared.info.lock().unwrap().state != "recording" {
        assert!(t0.elapsed() < Duration::from_secs(60), "no arranca");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(8));
    let info = shared.info.lock().unwrap().clone();
    let opened = info.microphone_device.as_ref().map(|d| d.lock().unwrap().clone()).unwrap_or_default();
    let fallback = info.microphone_fallback.as_ref().is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst));
    let out = dir.join("micro.mp4");
    rec.handle().save(6, &out).expect("guardar");
    drop(rec);

    let p = remux::probe(&out).expect("abrir el clip");
    let (pcm, missing) = remux::decode_audio(&out).expect("decodificar");
    let peak = pcm.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);
    let rms = (pcm.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / pcm.len().max(1) as f64).sqrt();
    println!(
        "clip {:.2} s con {} pista(s) de audio ({:?}); micro: {:.2} s, pico {peak}, RMS {rms:.1}, {missing} muestras perdidas",
        p.duration as f64 / 1e7,
        p.audio_streams,
        p.kinds,
        pcm.len() as f64 / 96_000.0
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(opened, chosen.id, "se ha abierto otro micrófono");
    assert!(!fallback, "ha caído al predeterminado");
    assert_eq!(p.audio_streams, 1);
    assert!(((pcm.len() as f64 / 96_000.0) - p.duration as f64 / 1e7).abs() < 0.2, "la pista del micro no dura lo que el clip");
    assert_eq!(missing, 0);
    assert!(peak > 0, "la pista del micro está en silencio absoluto");
}
