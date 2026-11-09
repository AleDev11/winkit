//! Captura del monitor principal con Windows.Graphics.Capture y conversión
//! BGRA → NV12 en la GPU con el procesador de vídeo de D3D11. Los fotogramas
//! no pasan nunca por la CPU: la textura NV12 va directa al codificador.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::{factory, Interface, Result};
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub winrt: IDirect3DDevice,
}

pub fn create_gpu() -> Result<Gpu> {
    unsafe {
        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT;
        let mut device = None;
        let mut context = None;
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1];
        let mut r = D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), flags, Some(&levels), D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context));
        if r.is_err() {
            // Drivers antiguos rechazan 11_1 en la lista con E_INVALIDARG.
            r = D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), flags, Some(&levels[1..]), D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context));
        }
        r?;
        let device = device.unwrap();
        let context = context.unwrap();
        // Media Foundation usa el dispositivo desde sus propios hilos.
        if let Ok(mt) = device.cast::<ID3D11Multithread>() {
            let _ = mt.SetMultithreadProtected(true);
        }
        let winrt: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&device.cast::<IDXGIDevice>()?)?.cast()?;
        Ok(Gpu { device, context, winrt })
    }
}

pub struct Capture {
    _item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    pub monitor: HMONITOR,
    pub width: u32,
    pub height: u32,
    closed: Arc<AtomicBool>,
}

impl Capture {
    /// `fp16`: captura en scRGB lineal (R16G16B16A16Float), para monitores HDR.
    pub fn start(gpu: &Gpu, monitor: HMONITOR, fps: u32, fp16: bool) -> Result<Self> {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(monitor)? };
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &gpu.winrt,
            if fp16 { DirectXPixelFormat::R16G16B16A16Float } else { DirectXPixelFormat::B8G8R8A8UIntNormalized },
            2,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(true);
        // Sin el borde amarillo (Windows 11) y sin más fotogramas de los que se usan (24H2+).
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetMinUpdateInterval(TimeSpan { Duration: 10_000_000 / fps.max(1) as i64 });
        let closed = Arc::new(AtomicBool::new(false));
        let flag = closed.clone();
        item.Closed(&TypedEventHandler::new(move |_, _| {
            flag.store(true, Ordering::SeqCst);
            Ok(())
        }))?;
        session.StartCapture()?;
        Ok(Self { _item: item, pool, session, monitor, width: size.Width.max(2) as u32, height: size.Height.max(2) as u32, closed })
    }

    /// El monitor se ha desconectado o la captura se ha perdido.
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Copia el fotograma más reciente (si hay uno nuevo) en `dst` y devuelve su
    /// tamaño. Los anteriores se descartan.
    pub fn grab_latest(&self, gpu: &Gpu, dst: &ID3D11Texture2D) -> Result<Option<(u32, u32)>> {
        let mut latest = None;
        loop {
            match self.pool.TryGetNextFrame() {
                Ok(f) => {
                    if let Some(old) = latest.replace(f) {
                        let _ = old.Close();
                    }
                }
                Err(e) if e.code().is_ok() => break, // null: no hay fotograma nuevo
                Err(e) => return Err(e),
            }
        }
        let Some(frame) = latest else { return Ok(None) };
        let size = frame.ContentSize()?;
        let (w, h) = (size.Width as u32, size.Height as u32);
        if w == self.width && h == self.height {
            let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
            let tex: ID3D11Texture2D = unsafe { access.GetInterface()? };
            unsafe { gpu.context.CopyResource(dst, &tex) };
        }
        let _ = frame.Close();
        Ok(Some((w, h)))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

/// Convierte una textura BGRA fija (`source`) a texturas NV12 de un pequeño
/// grupo. Cada textura NV12 queda ocupada hasta que el codificador suelta la
/// muestra que la envuelve (`free` la recupera).
pub struct Converter {
    vctx: ID3D11VideoContext,
    vp: ID3D11VideoProcessor,
    input: ID3D11VideoProcessorInputView,
    pub source: ID3D11Texture2D,
    pub targets: Vec<ID3D11Texture2D>,
    outputs: Vec<ID3D11VideoProcessorOutputView>,
    pub free: Arc<Mutex<Vec<usize>>>,
}

impl Converter {
    pub fn new(gpu: &Gpu, in_w: u32, in_h: u32, out_w: u32, out_h: u32, fps: u32, pool: usize) -> Result<Self> {
        unsafe {
            let vdev: ID3D11VideoDevice = gpu.device.cast()?;
            let vctx: ID3D11VideoContext = gpu.context.cast()?;
            let rate = DXGI_RATIONAL { Numerator: fps, Denominator: 1 };
            let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                InputFrameRate: rate,
                InputWidth: in_w,
                InputHeight: in_h,
                OutputFrameRate: rate,
                OutputWidth: out_w,
                OutputHeight: out_h,
                Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
            };
            let en = vdev.CreateVideoProcessorEnumerator(&desc)?;
            let vp = vdev.CreateVideoProcessor(&en, 0)?;

            let source = texture(gpu, in_w, in_h, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE)?;
            // Negro hasta que llegue el primer fotograma.
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(&source, None, Some(&mut rtv))?;
            if let Some(rtv) = rtv {
                gpu.context.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 1.0]);
            }
            let in_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                FourCC: 0,
                ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: 0 } },
            };
            let mut input = None;
            vdev.CreateVideoProcessorInputView(&source, &en, &in_desc, Some(&mut input))?;

            let out_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
            };
            let mut targets = Vec::new();
            let mut outputs = Vec::new();
            for _ in 0..pool {
                let t = texture(gpu, out_w, out_h, DXGI_FORMAT_NV12, D3D11_BIND_RENDER_TARGET)?;
                let mut view = None;
                vdev.CreateVideoProcessorOutputView(&t, &en, &out_desc, Some(&mut view))?;
                targets.push(t);
                outputs.push(view.unwrap());
            }

            // Escritorio RGB completo (0–255) → YUV BT.709 de rango limitado, lo estándar en vídeo.
            if let Ok(vctx1) = vctx.cast::<ID3D11VideoContext1>() {
                vctx1.VideoProcessorSetStreamColorSpace1(&vp, 0, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709);
                vctx1.VideoProcessorSetOutputColorSpace1(&vp, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709);
            } else {
                vctx.VideoProcessorSetStreamColorSpace(&vp, 0, &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 0 });
                // YCbCr_Matrix = BT.709 (bit 2), Nominal_Range = 16–235 (bits 4–5 = 1).
                vctx.VideoProcessorSetOutputColorSpace(&vp, &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: (1 << 2) | (1 << 4) });
            }
            vctx.VideoProcessorSetStreamAutoProcessingMode(&vp, 0, false);
            vctx.VideoProcessorSetStreamFrameFormat(&vp, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);

            let free = Arc::new(Mutex::new((0..pool).rev().collect()));
            Ok(Self { vctx, vp, input: input.unwrap(), source, targets, outputs, free })
        }
    }

    /// Reserva una textura NV12 libre y convierte en ella `source`.
    /// None si el codificador aún tiene todas ocupadas.
    pub fn convert(&self) -> Result<Option<usize>> {
        let Some(idx) = self.free.lock().unwrap().pop() else { return Ok(None) };
        let stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: std::mem::ManuallyDrop::new(Some(self.input.clone())),
            ..Default::default()
        };
        let streams = [stream];
        let r = unsafe { self.vctx.VideoProcessorBlt(&self.vp, &self.outputs[idx], 0, &streams) };
        let [mut stream] = streams;
        unsafe { std::mem::ManuallyDrop::drop(&mut stream.pInputSurface) };
        if let Err(e) = r {
            self.free.lock().unwrap().push(idx);
            return Err(e);
        }
        Ok(Some(idx))
    }
}

pub fn texture(gpu: &Gpu, w: u32, h: u32, format: DXGI_FORMAT, bind: D3D11_BIND_FLAG) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: bind.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut t = None;
    unsafe { gpu.device.CreateTexture2D(&desc, None, Some(&mut t))? };
    Ok(t.unwrap())
}
