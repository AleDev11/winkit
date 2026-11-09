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
    let buffer = dir.join("buffer");
    ring::wipe(&buffer);
    let shared = Arc::new(Shared::new(Duration::from_secs(120)));
    let t0 = Instant::now();
    let cpu0 = cpu_time();
    let cfg = Config { fps: 60, quality: "high".into(), system_audio: true, microphone: true, segment_secs: 10, dir: buffer.clone(), monitor: String::new(), force_hdr_path: false, synthetic_audio: None };
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
        force_hdr_path: true,
        synthetic_audio: Some(FREQ),
    };
    let rec = Recorder::start(cfg, shared.clone(), Arc::new(|| {}));
    let t0 = Instant::now();
    while shared.info.lock().unwrap().state != "recording" {
        assert!(t0.elapsed() < Duration::from_secs(20), "no arranca");
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
