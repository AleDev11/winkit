//! Audio en crudo: PCM de 16 bits que llega por la red a trozos, archivos WAV
//! (lo que devuelven las voces de Windows) y un remuestreo lineal sencillo
//! para cuando el dispositivo no acepta la frecuencia de la voz.

/// Muestras mono en coma flotante (−1…1) a una frecuencia dada.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl Audio {
    #[cfg(test)]
    pub fn seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / self.sample_rate as f64
        }
    }
}

/// PCM de 16 bits little-endian mono que llega en trozos de cualquier tamaño
/// (un trozo puede cortar una muestra por la mitad).
#[derive(Default)]
pub struct Pcm16Stream {
    pending: Option<u8>,
}

impl Pcm16Stream {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<f32> {
        let mut out = Vec::with_capacity(bytes.len() / 2 + 1);
        let mut rest = bytes;
        if let Some(lo) = self.pending.take() {
            match rest.split_first() {
                Some((hi, tail)) => {
                    out.push(i16::from_le_bytes([lo, *hi]) as f32 / 32768.0);
                    rest = tail;
                }
                None => {
                    self.pending = Some(lo);
                    return out;
                }
            }
        }
        let mut pairs = rest.chunks_exact(2);
        for p in &mut pairs {
            out.push(i16::from_le_bytes([p[0], p[1]]) as f32 / 32768.0);
        }
        if let [b] = pairs.remainder() {
            self.pending = Some(*b);
        }
        out
    }
}

/// Lee un WAV (PCM de 8/16/24/32 bits o coma flotante de 32 bits) y lo deja en mono.
pub fn parse_wav(data: &[u8]) -> Result<Audio, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("no es un WAV".into());
    }
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(data.len());
        let body = &data[body_start..body_end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err("cabecera fmt corta".into());
            }
            let mut tag = u16::from_le_bytes([body[0], body[1]]);
            let channels = u16::from_le_bytes([body[2], body[3]]);
            let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            let bits = u16::from_le_bytes([body[14], body[15]]);
            // WAVE_FORMAT_EXTENSIBLE: el formato real va en el subformato.
            if tag == 0xFFFE && body.len() >= 26 {
                tag = u16::from_le_bytes([body[24], body[25]]);
            }
            fmt = Some((tag, channels.max(1), rate, bits));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = fmt.ok_or("falta la cabecera fmt")?;
            let samples = decode(body, tag, bits)?;
            return Ok(Audio { sample_rate: rate, samples: downmix(&samples, channels as usize) });
        }
        // Los bloques van alineados a 2 bytes.
        pos = body_start + size + (size & 1);
    }
    Err("el WAV no tiene datos".into())
}

fn decode(body: &[u8], tag: u16, bits: u16) -> Result<Vec<f32>, String> {
    Ok(match (tag, bits) {
        (1, 8) => body.iter().map(|b| (*b as f32 - 128.0) / 128.0).collect(),
        (1, 16) => body.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect(),
        (1, 24) => body.chunks_exact(3).map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0).collect(),
        (1, 32) => body.chunks_exact(4).map(|b| i32::from_le_bytes(b.try_into().unwrap()) as f32 / 2_147_483_648.0).collect(),
        (3, 32) => body.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect(),
        _ => return Err(format!("formato WAV no admitido ({tag}, {bits} bits)")),
    })
}

/// Intercalado de `channels` canales → mono (media).
pub fn downmix(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect()
}

/// Remuestreo lineal que conserva la fase entre trozos consecutivos.
pub struct Resampler {
    step: f64,
    pos: f64,
    last: Option<f32>,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self { step: from as f64 / to as f64, pos: 0.0, last: None }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        // La última muestra del trozo anterior va delante (índice 0).
        let mut buf = Vec::with_capacity(input.len() + 1);
        buf.push(self.last.unwrap_or(input[0]));
        buf.extend_from_slice(input);
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 2);
        while self.pos + 1.0 < buf.len() as f64 {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            out.push(buf[i] + (buf[i + 1] - buf[i]) * frac);
            self.pos += self.step;
        }
        self.pos -= (buf.len() - 1) as f64;
        self.last = buf.last().copied();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(tag: u16, channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data.len() as u32 + 10).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&tag.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * channels as u32 * bits as u32 / 8).to_le_bytes());
        v.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        // Un bloque desconocido de tamaño impar antes de los datos.
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn pcm16_por_trozos_impares() {
        let samples: Vec<i16> = vec![0, 16384, -16384, 32767, -32768, 1];
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut st = Pcm16Stream::default();
        let mut out = Vec::new();
        for piece in [&bytes[0..1], &bytes[1..4], &bytes[4..4], &bytes[4..9], &bytes[9..]] {
            out.extend(st.push(piece));
        }
        assert_eq!(out.len(), samples.len());
        assert_eq!(out[1], 0.5);
        assert_eq!(out[2], -0.5);
        assert_eq!(out[4], -1.0);
    }

    #[test]
    fn lee_wav_de_16_bits_estereo() {
        let data: Vec<u8> = [1000i16, 3000, -2000, -4000].iter().flat_map(|s| s.to_le_bytes()).collect();
        let a = parse_wav(&wav(1, 2, 22050, 16, &data)).unwrap();
        assert_eq!(a.sample_rate, 22050);
        assert_eq!(a.samples.len(), 2);
        assert!((a.samples[0] - 2000.0 / 32768.0).abs() < 1e-6);
        assert!((a.samples[1] + 3000.0 / 32768.0).abs() < 1e-6);
    }

    #[test]
    fn lee_wav_float() {
        let data: Vec<u8> = [0.25f32, -0.5].iter().flat_map(|s| s.to_le_bytes()).collect();
        let a = parse_wav(&wav(3, 1, 48000, 32, &data)).unwrap();
        assert_eq!(a.samples, vec![0.25, -0.5]);
        assert!(parse_wav(b"RIFF....WAVEjunk").is_err());
        assert!(parse_wav(&wav(2, 1, 8000, 4, &[0])).is_err());
    }

    #[test]
    fn remuestreo_conserva_duracion() {
        // 1 s a 24 kHz → 48 kHz, en trozos.
        let input: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut r = Resampler::new(24_000, 48_000);
        let mut out = Vec::new();
        for c in input.chunks(1000) {
            out.extend(r.process(c));
        }
        assert!((out.len() as i64 - 48_000).abs() <= 2, "{}", out.len());
        // Sin saltos en las juntas: la señal es suave.
        assert!(out.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01));
        let mut down = Resampler::new(44_100, 16_000);
        let n: usize = (0..10).map(|_| down.process(&vec![0.0; 4410]).len()).sum();
        assert!((n as i64 - 16_000).abs() <= 2, "{n}");
    }
}
