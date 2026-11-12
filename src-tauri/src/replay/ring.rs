//! Búfer circular en disco: lista de segmentos MP4 ya cerrados, borrado de
//! los que se quedan fuera de la ventana y elección de los que cubren los
//! últimos N segundos al guardar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Segment {
    pub path: PathBuf,
    /// Cada arranque del grabador es una época: los segmentos de épocas
    /// distintas pueden tener otra resolución y no se mezclan.
    pub epoch: u64,
    /// Inicio y duración en unidades de 100 ns del reloj de su época.
    pub start: i64,
    pub duration: i64,
    /// Cuándo se cerró (para caducarlo).
    pub ended: Instant,
}

impl Segment {
    pub fn end(&self) -> i64 {
        self.start + self.duration
    }
}

#[derive(Default)]
pub struct Ring {
    segments: Vec<Segment>,
    retention: Duration,
    /// Mientras se guarda un clip no se borra nada; lo caducado espera aquí.
    pins: usize,
    doomed: Vec<PathBuf>,
}

impl Ring {
    pub fn new(retention: Duration) -> Self {
        Self { retention, ..Default::default() }
    }

    pub fn set_retention(&mut self, retention: Duration) {
        self.retention = retention;
        self.prune(Instant::now());
    }

    pub fn push(&mut self, s: Segment) {
        self.segments.push(s);
        self.prune(Instant::now());
    }

    /// Caduca los segmentos que terminaron hace más que la retención.
    pub fn prune(&mut self, now: Instant) {
        let retention = self.retention;
        let (old, keep): (Vec<_>, Vec<_>) =
            self.segments.drain(..).partition(|s| now.saturating_duration_since(s.ended) > retention);
        self.segments = keep;
        self.doomed.extend(old.into_iter().map(|s| s.path));
        if self.pins == 0 {
            for p in self.doomed.drain(..) {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    /// Bloquea el borrado y devuelve una copia de la lista.
    pub fn pin(&mut self) -> Vec<Segment> {
        self.pins += 1;
        self.segments.clone()
    }

    pub fn unpin(&mut self) {
        self.pins = self.pins.saturating_sub(1);
        self.prune(Instant::now());
    }

    pub fn clear(&mut self) {
        self.segments.clear();
        self.doomed.clear();
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }
}

/// Segmentos de la última época que cubren los últimos `keep` (100 ns), y el
/// instante (reloj de esa época) desde el que empieza el clip.
pub fn select(segments: &[Segment], keep: i64) -> Option<(Vec<Segment>, i64)> {
    let epoch = segments.iter().map(|s| s.epoch).max()?;
    let mut list: Vec<Segment> = segments.iter().filter(|s| s.epoch == epoch).cloned().collect();
    list.sort_by_key(|s| s.start);
    let end = list.last()?.end();
    let cut = (end - keep).max(list[0].start);
    list.retain(|s| s.end() > cut);
    Some((list, cut))
}

/// Vacía la carpeta del búfer (al arrancar, al parar y al salir).
pub fn wipe(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let _ = std::fs::create_dir_all(dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: i64 = 10_000_000;

    fn seg(epoch: u64, start_s: i64, dur_s: i64, ended: Instant) -> Segment {
        Segment {
            path: PathBuf::from(format!("seg-{epoch}-{start_s}.mp4")),
            epoch,
            start: start_s * S,
            duration: dur_s * S,
            ended,
        }
    }

    #[test]
    fn elige_los_segmentos_que_cubren_la_ventana() {
        let now = Instant::now();
        let segs: Vec<_> = (0..6).map(|i| seg(1, i * 15, 15, now)).collect(); // 0–90 s
        let (sel, cut) = select(&segs, 40 * S).unwrap();
        assert_eq!(cut, 50 * S);
        assert_eq!(sel.iter().map(|s| s.start / S).collect::<Vec<_>>(), vec![45, 60, 75]);
    }

    #[test]
    fn con_menos_historia_que_la_pedida_devuelve_todo() {
        let now = Instant::now();
        let segs = vec![seg(1, 0, 15, now), seg(1, 15, 7, now)];
        let (sel, cut) = select(&segs, 300 * S).unwrap();
        assert_eq!(cut, 0);
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn corte_justo_en_el_limite_no_incluye_el_segmento_anterior() {
        let now = Instant::now();
        let segs: Vec<_> = (0..4).map(|i| seg(1, i * 10, 10, now)).collect(); // 0–40 s
        let (sel, cut) = select(&segs, 20 * S).unwrap();
        assert_eq!(cut, 20 * S);
        assert_eq!(sel.iter().map(|s| s.start / S).collect::<Vec<_>>(), vec![20, 30]);
    }

    #[test]
    fn solo_usa_la_ultima_epoca() {
        let now = Instant::now();
        let segs = vec![seg(1, 0, 15, now), seg(1, 15, 15, now), seg(2, 0, 15, now), seg(2, 15, 4, now)];
        let (sel, cut) = select(&segs, 60 * S).unwrap();
        assert!(sel.iter().all(|s| s.epoch == 2));
        assert_eq!(sel.len(), 2);
        assert_eq!(cut, 0);
        assert!(select(&[], 60 * S).is_none());
    }

    #[test]
    fn caduca_los_viejos_salvo_mientras_se_guarda() {
        let dir = std::env::temp_dir().join(format!("winkit-ring-test-{}", std::process::id()));
        wipe(&dir);
        let t0 = Instant::now();
        let mut ring = Ring::new(Duration::from_secs(60));
        for i in 0..4 {
            let mut s = seg(1, i * 30, 30, t0 + Duration::from_secs(30 * (i as u64 + 1)));
            s.path = dir.join(format!("{i}.mp4"));
            std::fs::write(&s.path, b"x").unwrap();
            ring.segments.push(s);
        }
        // A los 150 s: el que acabó a los 30 s (hace 120) y el de los 60 s (hace 90) caducan.
        let pinned = ring.pin();
        assert_eq!(pinned.len(), 4);
        ring.prune(t0 + Duration::from_secs(150));
        assert_eq!(ring.len(), 2);
        assert!(dir.join("0.mp4").exists(), "no se borra nada mientras se guarda");
        ring.pins = 0;
        ring.prune(t0 + Duration::from_secs(150));
        assert!(!dir.join("0.mp4").exists());
        assert!(!dir.join("1.mp4").exists());
        assert!(dir.join("2.mp4").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
