//! La GPU del equipo, por DXGI: para elegir el motor (Vulkan o CPU) y decir
//! en Ajustes dónde corre la IA local.

use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};

#[derive(Clone, Debug)]
pub struct Gpu {
    pub name: String,
    /// VRAM dedicada en bytes.
    pub vram: u64,
}

/// Vendedor 0x1414 = Microsoft (Basic Render Driver, WARP): no es una GPU de verdad.
const MICROSOFT: u32 = 0x1414;

/// La GPU con más VRAM dedicada, si hay alguna de verdad.
pub fn best() -> Option<Gpu> {
    unsafe {
        let factory = CreateDXGIFactory1::<IDXGIFactory1>().ok()?;
        let mut best: Option<Gpu> = None;
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adapter.GetDesc1() else { continue };
            if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 || desc.VendorId == MICROSOFT {
                continue;
            }
            let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
            let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_string();
            let vram = desc.DedicatedVideoMemory as u64;
            if best.as_ref().is_none_or(|b| vram > b.vram) {
                best = Some(Gpu { name, vram });
            }
        }
        // Menos de 1 GB dedicado (gráficos integrados viejos): mejor la CPU.
        best.filter(|g| g.vram >= 1 << 30)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn encuentra_la_gpu_de_este_equipo() {
        // Informativo: en un equipo sin GPU dedicada devuelve None.
        println!("{:?}", super::best());
    }
}
