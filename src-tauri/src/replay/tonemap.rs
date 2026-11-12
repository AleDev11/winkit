//! HDR → SDR en la GPU. Con el HDR de Windows activado, la captura llega en
//! scRGB lineal (FP16, primarios BT.709, 1,0 = 80 nits) y el contenido SDR
//! (escritorio, ventanas) está al «nivel de blanco SDR» del monitor. Aquí:
//! 1. se divide por ese blanco: lo SDR queda en 0–1, igual que se ve;
//! 2. lo que pasa de `KNEE` (brillos HDR) se comprime con un hombro suave
//!    sobre el máximo de R, G y B (conserva el tono) hasta tocar 1,0;
//! 3. se codifica en sRGB y se escribe en la textura BGRA de siempre, así que
//!    el resto (BGRA → NV12 → H.264) no cambia.
//!
//! Un único pase de píxeles por fotograma nuevo; nada pasa por la CPU.

use super::capture::{texture, Gpu};
use windows::core::{Interface, Result, PCSTR};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT;

/// Hasta aquí (en unidades del blanco SDR) la imagen no se toca.
pub const KNEE: f32 = 0.95;

/// Curva de tonos sobre el máximo de los canales, en unidades del blanco SDR.
/// Identidad hasta `KNEE`; por encima, k + (1 − k)·(1 − e^(−(m − k)/(1 − k))):
/// continua, con pendiente 1 en la rodilla, creciente y siempre por debajo de 1.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tone(m: f32) -> f32 {
    if m <= KNEE {
        return m.max(0.0);
    }
    KNEE + (1.0 - KNEE) * (1.0 - (-(m - KNEE) / (1.0 - KNEE)).exp())
}

/// La misma curva en HLSL (más la conversión de entrada y la codificación sRGB).
fn shader_source() -> String {
    format!(
        r#"
cbuffer Params : register(b0) {{ float inv_white; float3 pad; }};
Texture2D<float4> src : register(t0);

float4 vs(uint id : SV_VertexID) : SV_Position {{
    // Un triángulo que cubre toda la pantalla.
    float2 uv = float2((id << 1) & 2, id & 2);
    return float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
}}

float3 srgb(float3 c) {{
    return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}}

float4 ps(float4 pos : SV_Position) : SV_Target {{
    // Los colores fuera de BT.709 llegan con componentes negativas: se recortan.
    float3 c = max(src.Load(int3(pos.xy, 0)).rgb * inv_white, 0);
    float m = max(max(c.r, c.g), c.b);
    const float k = {KNEE:.6};
    if (m > k) {{
        float t = k + (1 - k) * (1 - exp(-(m - k) / (1 - k)));
        c *= t / m;
    }}
    return float4(srgb(saturate(c)), 1);
}}
"#
    )
}

fn compile(src: &str, entry: &str, target: &str) -> Result<ID3DBlob> {
    let entry = format!("{entry}\0");
    let target = format!("{target}\0");
    let mut code = None;
    let mut errors = None;
    let r = unsafe {
        D3DCompile(
            src.as_ptr() as *const _,
            src.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr()),
            PCSTR(target.as_ptr()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(e) = r {
        if let Some(err) = errors {
            let msg = unsafe { std::slice::from_raw_parts(err.GetBufferPointer() as *const u8, err.GetBufferSize()) };
            crate::log_error!("clips", "error del shader: {}", String::from_utf8_lossy(msg));
        }
        return Err(e);
    }
    Ok(code.unwrap())
}

fn bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()) }
}

pub struct ToneMapper {
    /// Aquí se copia cada fotograma FP16 de la captura.
    pub input: ID3D11Texture2D,
    srv: ID3D11ShaderResourceView,
    rtv: ID3D11RenderTargetView,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cb: ID3D11Buffer,
    width: u32,
    height: u32,
}

impl ToneMapper {
    /// `target`: la textura BGRA que luego se convierte a NV12. `sdr_white`:
    /// blanco SDR del monitor en unidades scRGB (1,0 = 80 nits).
    pub fn new(gpu: &Gpu, width: u32, height: u32, target: &ID3D11Texture2D, sdr_white: f32) -> Result<Self> {
        unsafe {
            let input = texture(gpu, width, height, DXGI_FORMAT_R16G16B16A16_FLOAT, D3D11_BIND_SHADER_RESOURCE)?;
            let mut srv = None;
            gpu.device.CreateShaderResourceView(&input, None, Some(&mut srv))?;
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(target, None, Some(&mut rtv))?;
            let src = shader_source();
            let vs_code = compile(&src, "vs", "vs_4_0")?;
            let ps_code = compile(&src, "ps", "ps_4_0")?;
            let mut vs = None;
            gpu.device.CreateVertexShader(bytes(&vs_code), None, Some(&mut vs))?;
            let mut ps = None;
            gpu.device.CreatePixelShader(bytes(&ps_code), None, Some(&mut ps))?;
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let params = [1.0 / sdr_white.max(0.1), 0.0, 0.0, 0.0f32];
            let data = D3D11_SUBRESOURCE_DATA { pSysMem: params.as_ptr() as *const _, ..Default::default() };
            let mut cb = None;
            gpu.device.CreateBuffer(&desc, Some(&data), Some(&mut cb))?;
            Ok(Self { input, srv: srv.unwrap(), rtv: rtv.unwrap(), vs: vs.unwrap(), ps: ps.unwrap(), cb: cb.unwrap(), width, height })
        }
    }

    /// Cambia el blanco SDR (el usuario ha movido el control en Configuración).
    pub fn set_white(&self, gpu: &Gpu, sdr_white: f32) {
        let params = [1.0 / sdr_white.max(0.1), 0.0, 0.0, 0.0f32];
        unsafe { gpu.context.UpdateSubresource(&self.cb, 0, None, params.as_ptr() as *const _, 0, 0) };
    }

    /// `input` → curva de tonos → textura BGRA de destino.
    pub fn render(&self, gpu: &Gpu) {
        let ctx = &gpu.context;
        // Media Foundation comparte el contexto inmediato: que nadie lo toque a la vez.
        let mt = gpu.device.cast::<ID3D11Multithread>().ok();
        unsafe {
            if let Some(m) = &mt {
                m.Enter();
            }
            ctx.OMSetRenderTargets(Some(&[Some(self.rtv.clone())]), None);
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: self.width as f32,
                Height: self.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            ctx.IASetInputLayout(None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps, None);
            ctx.PSSetShaderResources(0, Some(&[Some(self.srv.clone())]));
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            ctx.Draw(3, 0);
            ctx.PSSetShaderResources(0, Some(&[None]));
            ctx.OMSetRenderTargets(None, None);
            if let Some(m) = &mt {
                m.Leave();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_curva_de_tonos_es_creciente_y_suave() {
        let mut prev = -1.0f32;
        for i in 0..=20_000 {
            let m = i as f32 * 0.001; // 0 a 20 veces el blanco SDR
            let t = tone(m);
            assert!(t.is_finite(), "{m} → {t}");
            assert!(t >= prev, "no es creciente en {m}");
            assert!(t <= 1.0, "{m} → {t} se sale de SDR");
            prev = t;
        }
        // Lo SDR no se toca; el blanco SDR queda casi en blanco.
        assert_eq!(tone(0.5), 0.5);
        assert_eq!(tone(KNEE), KNEE);
        assert!(tone(1.0) > 0.98 && tone(1.0) < 1.0, "{}", tone(1.0));
        // Los brillos se comprimen por debajo de 1 pero conservan el orden.
        assert!(tone(1.5) < 1.0 && tone(1.5) > tone(1.0));
        // Muy por encima del blanco la curva llega a 1,0 (en f32 ya no se distingue).
        assert!(tone(1.6) < 1.0 && tone(4.0) <= 1.0);
        // Pendiente continua en la rodilla (≈ 1 a ambos lados).
        let d = (tone(KNEE + 1e-3) - tone(KNEE)) / 1e-3;
        assert!((d - 1.0).abs() < 0.03, "{d}");
        // Extremos y valores raros.
        assert_eq!(tone(0.0), 0.0);
        assert_eq!(tone(-1.0), 0.0);
        assert!(tone(1e6).is_finite() && tone(f32::MAX).is_finite());
    }

    #[test]
    fn el_shader_compila() {
        let src = shader_source();
        assert!(compile(&src, "vs", "vs_4_0").is_ok());
        assert!(compile(&src, "ps", "ps_4_0").is_ok());
    }
}
