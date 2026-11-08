//! Optional runtime GPU stages: whole-bitstream PNG/VP8L for selected profiles, JPEG inverse-DCT, PNG colour expansion,
//! WebP VP8 YUV reconstruction / VP8L subtract-green, and large resizes.
//!
//! Unsupported entropy bitstream profiles are parsed on the CPU. Independent reconstruction
//! stages can run on the GPU, as can large output image resizes. Drivers
//! are discovered *at runtime*: compilation never requires a graphics adapter,
//! OpenCL SDK, or CUDA/ROCm toolkit. Failure to initialize or execute is always
//! a CPU fallback, not a decoding failure.

use crate::{DecodeBackend, Image, ImageFormat};

use std::sync::atomic::{AtomicU64, Ordering};

// Diagnostics for the GPU auto-profiler. Only successful kernel dispatches
// count; silently falling back to CPU does not masquerade as acceleration.
static INTEL_JPEG_BATCHES: AtomicU64 = AtomicU64::new(0);
static AMD_JPEG_BATCHES: AtomicU64 = AtomicU64::new(0);
static NVIDIA_JPEG_BATCHES: AtomicU64 = AtomicU64::new(0);
fn jpeg_counter(backend: DecodeBackend) -> Option<&'static AtomicU64> {
    match backend {
        DecodeBackend::Cpu => None,
        DecodeBackend::IntelGpu => Some(&INTEL_JPEG_BATCHES),
        DecodeBackend::AmdGpu => Some(&AMD_JPEG_BATCHES),
        DecodeBackend::NvidiaGpu => Some(&NVIDIA_JPEG_BATCHES),
    }
}

pub(crate) fn jpeg_idct_dispatch_count(backend: DecodeBackend) -> u64 {
    jpeg_counter(backend).map_or(0, |counter| counter.load(Ordering::Relaxed))
}


// Successful pixel-reconstruction dispatches, not mere device availability.
static INTEL_PNG: AtomicU64 = AtomicU64::new(0);
static AMD_PNG: AtomicU64 = AtomicU64::new(0);
static NVIDIA_PNG: AtomicU64 = AtomicU64::new(0);
static INTEL_WEBP: AtomicU64 = AtomicU64::new(0);
static AMD_WEBP: AtomicU64 = AtomicU64::new(0);
static NVIDIA_WEBP: AtomicU64 = AtomicU64::new(0);
fn pixel_counter(backend: DecodeBackend, png: bool) -> Option<&'static AtomicU64> {
    match (backend, png) {
        (DecodeBackend::IntelGpu, true) => Some(&INTEL_PNG),
        (DecodeBackend::AmdGpu, true) => Some(&AMD_PNG),
        (DecodeBackend::NvidiaGpu, true) => Some(&NVIDIA_PNG),
        (DecodeBackend::IntelGpu, false) => Some(&INTEL_WEBP),
        (DecodeBackend::AmdGpu, false) => Some(&AMD_WEBP),
        (DecodeBackend::NvidiaGpu, false) => Some(&NVIDIA_WEBP),
        _ => None,
    }
}
pub(crate) fn png_dispatch_count(backend: DecodeBackend) -> u64 {
    pixel_counter(backend, true).map_or(0, |c| c.load(Ordering::Relaxed))
}
pub(crate) fn webp_dispatch_count(backend: DecodeBackend) -> u64 {
    pixel_counter(backend, false).map_or(0, |c| c.load(Ordering::Relaxed))
}
pub(crate) const MIN_GPU_DECODE_PIXELS: usize = 2_000_000;

/// PNG: CPU DEFLATE + unfilter, optional GPU colour expansion.
pub(crate) fn png_unpack_available(backend: DecodeBackend) -> bool {
    backend != DecodeBackend::Cpu && opencl::pixel_kernels_available(backend)
}
pub(crate) fn png_unpack(raw: &[u8], channels: u32, backend: DecodeBackend) -> Option<Vec<u32>> {
    if backend == DecodeBackend::Cpu || !(1..=4).contains(&channels) ||
        raw.len() % channels as usize != 0 ||
        raw.len() / (channels as usize) < MIN_GPU_DECODE_PIXELS { return None; }
    let result = opencl::png_unpack(raw, channels, backend);
    if result.is_some() {
        if let Some(c) = pixel_counter(backend, true) { c.fetch_add(1, Ordering::Relaxed); }
    }
    result
}
/// WebP VP8: CPU entropy/reconstruction, optional GPU YUV420 -> packed RGBA.
pub(crate) fn webp_yuv420(
    y: &[u8], u: &[u8], v: &[u8], a: Option<&[u8]>, width: usize,
    height: usize, y_stride: usize, uv_stride: usize, backend: DecodeBackend,
) -> Option<Vec<u32>> {
    if backend == DecodeBackend::Cpu || width.checked_mul(height)? < MIN_GPU_DECODE_PIXELS { return None; }
    let result = opencl::webp_yuv420(y, u, v, a, width, height, y_stride, uv_stride, backend);
    if result.is_some() {
        if let Some(c) = pixel_counter(backend, false) { c.fetch_add(1, Ordering::Relaxed); }
    }
    result
}
/// WebP VP8L: independently parallel lossless subtract-green + RGBA packing.
pub(crate) fn webp_subgreen(pixels: &[u32], backend: DecodeBackend) -> Option<Vec<u32>> {
    if backend == DecodeBackend::Cpu || pixels.len() < MIN_GPU_DECODE_PIXELS { return None; }
    let result = opencl::webp_subgreen(pixels, backend);
    if result.is_some() {
        if let Some(c) = pixel_counter(backend, false) { c.fetch_add(1, Ordering::Relaxed); }
    }
    result
}

/// Minimum output pixel count before host/device transfers and kernel launch
/// have a chance to pay off. This is a conservative heuristic, not a guarantee.
const MIN_GPU_OUTPUT_PIXELS: usize = 2_000_000;

/// Compression/container formats for which large decoded output resampling can
/// be worthwhile. Simple scalar codecs and SVG/ICO keep their CPU path.
pub(crate) const fn gpu_rescale_candidate(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP |
        ImageFormat::Gif | ImageFormat::Bmp | ImageFormat::Fits |
        ImageFormat::Tiff | ImageFormat::Avif
    )
}

/// Resizes with OpenCL only if suitable, returning whether *every* frame used
/// the GPU. If any stage fails, remaining frames are rescaled on the CPU.
pub(crate) fn rescale(image: &mut Image, width: u32, height: u32, backend: DecodeBackend) -> bool {
    if image.width == width && image.height == height { return false; }
    let output_len = (width as usize).checked_mul(height as usize);
    let input_len = (image.width as usize).checked_mul(image.height as usize);
    if backend == DecodeBackend::Cpu ||
        !matches!(output_len, Some(n) if n >= MIN_GPU_OUTPUT_PIXELS && n <= u32::MAX as usize) ||
        !matches!(input_len, Some(n) if n > 0 && n <= u32::MAX as usize)
    {
        image.rescale(width, height);
        return false;
    }

    let old_w = image.width;
    let old_h = image.height;
    // Collect the output before touching the original image. A failed GPU call
    // cannot leave a partially mutated image; all-or-nothing CPU fallback.
    let mut outputs = Vec::with_capacity(image.frames.len() + 1);
    for frame in std::iter::once(image.pixels.as_slice())
        .chain(image.frames.iter().map(|f| f.pixels.as_slice()))
    {
        match opencl::resize(frame, old_w, old_h, width, height, backend) {
            Some(result) => outputs.push(result),
            None => {
                image.rescale(width, height);
                return false;
            }
        }
    }
    image.pixels = outputs.remove(0);
    for (frame, result) in image.frames.iter_mut().zip(outputs) { frame.pixels = result; }
    image.width = width;
    image.height = height;
    true
}

/// True if the vendor's JPEG IDCT kernel was successfully compiled. Other
/// formats can still decode with the CPU without a graphics driver.
pub(crate) fn jpeg_idct_available(backend: DecodeBackend) -> bool {
    if backend == DecodeBackend::Cpu { return false; }
    opencl::jpeg_idct_available(backend)
}

/// Inverse DCT for dequantized baseline JPEG coefficient blocks. The input
/// has 64 i32 values per block in natural 8x8 order; the output has 64 u8
/// samples per block. CPU fallback is handled by the JPEG decoder itself.
pub(crate) fn jpeg_idct(coefficients: &[i32], backend: DecodeBackend) -> Option<Vec<u8>> {
    if backend == DecodeBackend::Cpu || coefficients.is_empty() || coefficients.len() % 64 != 0 {
        return None;
    }
    let output = opencl::jpeg_idct(coefficients, backend);
    if output.is_some() {
        if let Some(counter) = jpeg_counter(backend) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
    output
}

/// Full compressed-pixel decode on the chosen OpenCL GPU, returning only after
/// the kernel completed successfully. Metadata is validated separately.
pub(crate) fn full_decode_opencl(
    input: &[u8], pixel_count: usize, width: u32, height: u32,
    offset: u32, mode: u32, format: ImageFormat, backend: DecodeBackend,
) -> Option<Vec<u32>> {
    if backend == DecodeBackend::Cpu { return None; }
    opencl::full_decode(input,pixel_count,width,height,offset,mode,format,backend)
}

/// Decodes the supported VP8L bitstream profile entirely on the device:
/// Huffman literals/LZ77, optional small colour cache, and RGBA packing.
pub(crate) fn webp_lossless_opencl(vp8l:&[u8],w:u32,h:u32,backend:DecodeBackend)->Option<Vec<u32>> {
    if backend==DecodeBackend::Cpu {return None;}
    opencl::webp_lossless(vp8l,w,h,backend)
}

/// Decodes a supported PNG zlib stream completely on the OpenCL device:
/// DEFLATE, Adler-32, scanline filters, and colour reconstruction.
pub(crate) fn png_zlib_opencl(zlib: &[u8], width: u32, height: u32,
    channels: u32, backend: DecodeBackend) -> Option<Vec<u32>> {
    if backend == DecodeBackend::Cpu {return None;}
    opencl::png_zlib(zlib,width,height,channels,backend)
}

pub(crate) fn available(backend: DecodeBackend) -> bool {
    if backend == DecodeBackend::Cpu { return true; }
    opencl::available(backend)
}

#[cfg(all(feature = "gpu-opencl", any(target_os = "windows", target_os = "linux")))]
mod opencl {
    use super::{DecodeBackend, ImageFormat};
    use std::ffi::{c_char, c_void, CStr, CString};
    #[cfg(target_os = "linux")]
    use std::ffi::c_int;
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock};

    type ClHandle = *mut c_void;
    type ClInt = i32;
    type ClUint = u32;
    type ClUlong = u64;
    type ClSize = usize;
    type ClBool = u32;
    type ClDeviceType = u64;
    type ClMemFlags = u64;
    const CL_SUCCESS: ClInt = 0;
    const CL_DEVICE_TYPE_GPU: ClDeviceType = 1 << 2;
    const CL_DEVICE_VENDOR_ID: ClUint = 0x1001;
    const CL_MEM_READ_ONLY: ClMemFlags = 1 << 2;
    const CL_MEM_READ_WRITE: ClMemFlags = 1;
    const CL_MEM_WRITE_ONLY: ClMemFlags = 1 << 1;
    const CL_MEM_COPY_HOST_PTR: ClMemFlags = 1 << 5;
    const CL_TRUE: ClBool = 1;

    // OpenCL uses the system calling convention on Windows (including x86).
    type GetPlatformIds = unsafe extern "system" fn(ClUint,*mut ClHandle,*mut ClUint)->ClInt;
    type GetDeviceIds = unsafe extern "system" fn(ClHandle,ClDeviceType,ClUint,*mut ClHandle,*mut ClUint)->ClInt;
    type GetDeviceInfo = unsafe extern "system" fn(ClHandle,ClUint,ClSize,*mut c_void,*mut ClSize)->ClInt;
    type CreateContext = unsafe extern "system" fn(*const isize,ClUint,*const ClHandle,Option<unsafe extern "system" fn(*const c_char,*const c_void,ClSize,*mut c_void)>,*mut c_void,*mut ClInt)->ClHandle;
    type ReleaseContext = unsafe extern "system" fn(ClHandle)->ClInt;
    type CreateCommandQueue = unsafe extern "system" fn(ClHandle,ClHandle,ClUlong,*mut ClInt)->ClHandle;
    type ReleaseCommandQueue = unsafe extern "system" fn(ClHandle)->ClInt;
    type CreateProgramWithSource = unsafe extern "system" fn(ClHandle,ClUint,*const *const c_char,*const ClSize,*mut ClInt)->ClHandle;
    type BuildProgram = unsafe extern "system" fn(ClHandle,ClUint,*const ClHandle,*const c_char,Option<unsafe extern "system" fn(ClHandle,*mut c_void)>,*mut c_void)->ClInt;
    type ReleaseProgram = unsafe extern "system" fn(ClHandle)->ClInt;
    type CreateKernel = unsafe extern "system" fn(ClHandle,*const c_char,*mut ClInt)->ClHandle;
    type ReleaseKernel = unsafe extern "system" fn(ClHandle)->ClInt;
    type CreateBuffer = unsafe extern "system" fn(ClHandle,ClMemFlags,ClSize,*mut c_void,*mut ClInt)->ClHandle;
    type ReleaseMemObject = unsafe extern "system" fn(ClHandle)->ClInt;
    type SetKernelArg = unsafe extern "system" fn(ClHandle,ClUint,ClSize,*const c_void)->ClInt;
    type EnqueueNDRangeKernel = unsafe extern "system" fn(ClHandle,ClHandle,ClUint,*const ClSize,*const ClSize,*const ClSize,ClUint,*const ClHandle,*mut ClHandle)->ClInt;
    type EnqueueReadBuffer = unsafe extern "system" fn(ClHandle,ClHandle,ClBool,ClSize,ClSize,*mut c_void,ClUint,*const ClHandle,*mut ClHandle)->ClInt;

    #[cfg(target_os = "windows")]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const c_char) -> ClHandle;
        fn GetProcAddress(module: ClHandle, symbol: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: ClHandle) -> i32;
    }
    #[cfg(target_os = "linux")]
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> ClHandle;
        fn dlsym(handle: ClHandle, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: ClHandle) -> c_int;
    }

    struct Library(ClHandle);
    impl Library {
        fn load() -> Option<Self> {
            #[cfg(target_os = "windows")]
            let handle = unsafe { LoadLibraryA(c"OpenCL.dll".as_ptr()) };
            #[cfg(target_os = "linux")]
            let handle = unsafe {
                let first = dlopen(c"libOpenCL.so.1".as_ptr(), 2);
                if first.is_null() { dlopen(c"libOpenCL.so".as_ptr(), 2) } else { first }
            };
            (!handle.is_null()).then_some(Self(handle))
        }
        unsafe fn symbol<F: Copy>(&self, name: &CStr) -> Option<F> {
            #[cfg(target_os = "windows")]
            let pointer = unsafe { GetProcAddress(self.0, name.as_ptr()) };
            #[cfg(target_os = "linux")]
            let pointer = unsafe { dlsym(self.0, name.as_ptr()) };
            if pointer.is_null() { None } else {
                // Function pointers are looked up by their exact OpenCL ABI names.
                Some(unsafe { std::mem::transmute_copy(&pointer) })
            }
        }
    }
    impl Drop for Library {
        fn drop(&mut self) {
            #[cfg(target_os = "windows")]
            unsafe { FreeLibrary(self.0); }
            #[cfg(target_os = "linux")]
            unsafe { dlclose(self.0); }
        }
    }

    // A session owns its dynamic library, so no function pointer outlives the
    // loaded library. Access to its command queue/kernel is guarded by Mutex.
    struct Api {
        get_platform_ids: GetPlatformIds, get_device_ids: GetDeviceIds,
        get_device_info: GetDeviceInfo, create_context: CreateContext,
        release_context: ReleaseContext, create_command_queue: CreateCommandQueue,
        release_command_queue: ReleaseCommandQueue,
        create_program_with_source: CreateProgramWithSource,
        build_program: BuildProgram, release_program: ReleaseProgram,
        create_kernel: CreateKernel, release_kernel: ReleaseKernel,
        create_buffer: CreateBuffer, release_mem_object: ReleaseMemObject,
        set_kernel_arg: SetKernelArg, enqueue_nd_range_kernel: EnqueueNDRangeKernel,
        enqueue_read_buffer: EnqueueReadBuffer,
    }
    impl Api {
        unsafe fn load(library: &Library) -> Option<Self> {
            macro_rules! fetch { ($s:literal) => { unsafe { library.symbol(CStr::from_bytes_with_nul(concat!($s, "\0").as_bytes()).expect("static OpenCL symbol")) }? }; }
            Some(Self {
                get_platform_ids: fetch!("clGetPlatformIDs"),
                get_device_ids: fetch!("clGetDeviceIDs"),
                get_device_info: fetch!("clGetDeviceInfo"),
                create_context: fetch!("clCreateContext"),
                release_context: fetch!("clReleaseContext"),
                create_command_queue: fetch!("clCreateCommandQueue"),
                release_command_queue: fetch!("clReleaseCommandQueue"),
                create_program_with_source: fetch!("clCreateProgramWithSource"),
                build_program: fetch!("clBuildProgram"),
                release_program: fetch!("clReleaseProgram"),
                create_kernel: fetch!("clCreateKernel"),
                release_kernel: fetch!("clReleaseKernel"),
                create_buffer: fetch!("clCreateBuffer"),
                release_mem_object: fetch!("clReleaseMemObject"),
                set_kernel_arg: fetch!("clSetKernelArg"),
                enqueue_nd_range_kernel: fetch!("clEnqueueNDRangeKernel"),
                enqueue_read_buffer: fetch!("clEnqueueReadBuffer"),
            })
        }
        fn find_device(&self, backend: DecodeBackend) -> Option<ClHandle> {
            let wanted = match backend {
                DecodeBackend::IntelGpu => 0x8086,
                DecodeBackend::AmdGpu => 0x1002,
                DecodeBackend::NvidiaGpu => 0x10de,
                DecodeBackend::Cpu => return None,
            };
            let mut count = 0;
            if unsafe { (self.get_platform_ids)(0, null_mut(), &mut count) } != CL_SUCCESS || count == 0 { return None; }
            // Limit pathological driver-provided counts before allocating.
            if count > 128 { return None; }
            let mut platforms = vec![null_mut(); count as usize];
            if unsafe { (self.get_platform_ids)(count, platforms.as_mut_ptr(), null_mut()) } != CL_SUCCESS { return None; }
            for platform in platforms {
                let mut devices_count = 0;
                if unsafe { (self.get_device_ids)(platform,CL_DEVICE_TYPE_GPU,0,null_mut(),&mut devices_count) } != CL_SUCCESS || devices_count == 0 || devices_count > 128 { continue; }
                let mut devices = vec![null_mut(); devices_count as usize];
                if unsafe { (self.get_device_ids)(platform,CL_DEVICE_TYPE_GPU,devices_count,devices.as_mut_ptr(),null_mut()) } != CL_SUCCESS { continue; }
                for device in devices {
                    let mut vendor_id: ClUint = 0;
                    if unsafe { (self.get_device_info)(device,CL_DEVICE_VENDOR_ID,std::mem::size_of::<ClUint>(),&mut vendor_id as *mut _ as *mut c_void,null_mut()) } == CL_SUCCESS && vendor_id == wanted {
                        return Some(device);
                    }
                }
            }
            None
        }
    }
    fn make_kernel() -> Option<CString> {
        use mata_gpu_intrinsics::{CapabilitySet, Expr, Type};
        let mut builder = mata_gpu_intrinsics::intel::KernelBuilder::new("mata_resize_nearest", CapabilitySet::CORE).ok()?;
        let source = builder.read_only_buffer("src", Type::U32).ok()?;
        let output = builder.read_write_buffer("dst", Type::U32).ok()?;
        for name in ["sw", "sh", "dw", "dh", "count"] { builder.value(name, Type::U32).ok()?; }
        let idx = builder.index_1d("idx").ok()?;
        builder.return_if_out_of_bounds(&idx, &Expr::variable("count",Type::U32).ok()?).ok()?;
        // 64-bit products avoid overflowing u32 when upscaling large images.
        // Integer division exactly matches the CPU's floor-coordinate mapping.
        let index = Expr::from_source_unchecked(
            "(uint)((((ulong)(idx / dw) * (ulong)sh) / (ulong)dh) * (ulong)sw + (((ulong)(idx % dw) * (ulong)sw) / (ulong)dw))",
            Type::U32
        );
        let pixel = source.load(&index).ok()?;
        builder.store(&output,&idx,&pixel).ok()?;
        CString::new(builder.build().ok()?).ok()
    }

    struct JpegProgram { program: ClHandle, kernel: ClHandle }
    struct PixelProgram { program: ClHandle, png: ClHandle, vp8: ClHandle, vp8l: ClHandle }

    struct PngZlibProgram { program: ClHandle, kernel: ClHandle }
    struct WebpFullProgram { program: ClHandle, kernel: ClHandle }

    struct FullProgram { program: ClHandle, qoi: ClHandle, tga: ClHandle, wbmp: ClHandle, bmp: ClHandle }

    struct Session {
        // The library must be dropped after any OpenCL resources.
        api: Api, context: ClHandle, queue: ClHandle,
        program: ClHandle, kernel: ClHandle, jpeg: Option<JpegProgram>, pixels: Option<PixelProgram>, full: Option<FullProgram>, png_zlib: Option<PngZlibProgram>, webp_full: Option<WebpFullProgram>, _library: Library,
    }
    // OpenCL contexts/queues are shareable between host threads; we serialize
    // all kernel argument mutation and submission with the enclosing Mutex.
    unsafe impl Send for Session {}
    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                if let Some(webp) = &self.webp_full {
                    (self.api.release_kernel)(webp.kernel);
                    (self.api.release_program)(webp.program);
                }
                if let Some(png) = &self.png_zlib {
                    (self.api.release_kernel)(png.kernel);
                    (self.api.release_program)(png.program);
                }
                if let Some(full) = &self.full {
                    (self.api.release_kernel)(full.qoi);
                    (self.api.release_kernel)(full.tga);
                    (self.api.release_kernel)(full.wbmp);
                    (self.api.release_kernel)(full.bmp);
                    (self.api.release_program)(full.program);
                }
                if let Some(pixels) = &self.pixels {
                    (self.api.release_kernel)(pixels.png);
                    (self.api.release_kernel)(pixels.vp8);
                    (self.api.release_kernel)(pixels.vp8l);
                    (self.api.release_program)(pixels.program);
                }
                if let Some(jpeg) = &self.jpeg {
                    (self.api.release_kernel)(jpeg.kernel);
                    (self.api.release_program)(jpeg.program);
                }
                (self.api.release_kernel)(self.kernel);
                (self.api.release_program)(self.program);
                (self.api.release_command_queue)(self.queue);
                (self.api.release_context)(self.context);
            }
        }
    }
    impl Session {
        fn create(backend: DecodeBackend) -> Option<Self> {
            let library = Library::load()?;
            let api = unsafe { Api::load(&library) }?;
            let device = api.find_device(backend)?;
            let source = make_kernel()?;
            let mut status = 0;
            let context = unsafe { (api.create_context)(null(),1,&device,None,null_mut(),&mut status) };
            if status != CL_SUCCESS || context.is_null() { return None; }
            let queue = unsafe { (api.create_command_queue)(context,device,0,&mut status) };
            if status != CL_SUCCESS || queue.is_null() {
                unsafe { (api.release_context)(context) }; return None;
            }
            let bytes = source.as_bytes();
            let source_ptr = source.as_ptr();
            let length = bytes.len();
            let program = unsafe { (api.create_program_with_source)(context,1,&source_ptr,&length,&mut status) };
            if status != CL_SUCCESS || program.is_null() {
                unsafe { (api.release_command_queue)(queue); (api.release_context)(context); }
                return None;
            }
            if unsafe { (api.build_program)(program,1,&device,null(),None,null_mut()) } != CL_SUCCESS {
                unsafe { (api.release_program)(program); (api.release_command_queue)(queue); (api.release_context)(context); }
                return None;
            }
            let kernel = unsafe { (api.create_kernel)(program,c"mata_resize_nearest".as_ptr(),&mut status) };
            if status != CL_SUCCESS || kernel.is_null() {
                unsafe { (api.release_program)(program); (api.release_command_queue)(queue); (api.release_context)(context); }
                return None;
            }
            // JPEG kernels are separately compiled: a device can still resize
            // even if its OpenCL compiler rejects the integer-IDCT program.
            let mut jpeg = None;
            let source = include_str!("gpu_jpeg_idct.cl");
            let source_ptr = source.as_ptr() as *const c_char;
            let source_len = source.len();
            let jpeg_program = unsafe { (api.create_program_with_source)(context, 1, &source_ptr, &source_len, &mut status) };
            if status == CL_SUCCESS && !jpeg_program.is_null() {
                if unsafe { (api.build_program)(jpeg_program, 1, &device, null(), None, null_mut()) } == CL_SUCCESS {
                    let jpeg_kernel = unsafe { (api.create_kernel)(jpeg_program, c"mata_jpeg_idct".as_ptr(), &mut status) };
                    if status == CL_SUCCESS && !jpeg_kernel.is_null() {
                        jpeg = Some(JpegProgram { program: jpeg_program, kernel: jpeg_kernel });
                    } else {
                        unsafe { (api.release_program)(jpeg_program); }
                    }
                } else {
                    unsafe { (api.release_program)(jpeg_program); }
                }
            }
            // Optional pixel reconstruction kernels; independent of resize/JPEG.
            let mut pixels = None;
            let source = include_str!("gpu_png_webp.cl");
            let source_ptr = source.as_ptr() as *const c_char;
            let source_len = source.len();
            let pixel_program = unsafe { (api.create_program_with_source)(context, 1, &source_ptr, &source_len, &mut status) };
            if status == CL_SUCCESS && !pixel_program.is_null() {
                if unsafe { (api.build_program)(pixel_program, 1, &device, null(), None, null_mut()) } == CL_SUCCESS {
                    let mut kernels = Vec::new();
                    for name in [c"mata_png_unpack", c"mata_vp8_yuv420", c"mata_vp8l_subgreen"] {
                        let handle = unsafe { (api.create_kernel)(pixel_program, name.as_ptr(), &mut status) };
                        if status != CL_SUCCESS || handle.is_null() { break; }
                        kernels.push(handle);
                    }
                    if kernels.len() == 3 {
                        pixels = Some(PixelProgram { program: pixel_program, png: kernels[0], vp8: kernels[1], vp8l: kernels[2] });
                    } else {
                        for kernel in kernels { unsafe { (api.release_kernel)(kernel); } }
                        unsafe { (api.release_program)(pixel_program); }
                    }
                } else { unsafe { (api.release_program)(pixel_program); } }
            }
            // Whole-image decoders are compiled independently from the other
            // acceleration stages. A device without support keeps CPU fallback.
            let mut full = None;
            let source = include_str!("gpu_full.cl");
            let source_ptr = source.as_ptr() as *const c_char;
            let length = source.len();
            let full_program = unsafe { (api.create_program_with_source)(context,1,&source_ptr,&length,&mut status) };
            if status == CL_SUCCESS && !full_program.is_null() {
                if unsafe { (api.build_program)(full_program,1,&device,null(),None,null_mut()) } == CL_SUCCESS {
                    let mut kernels=Vec::new();
                    for name in [c"mata_full_qoi",c"mata_full_tga",c"mata_full_wbmp",c"mata_full_bmp"] {
                        let k=unsafe{(api.create_kernel)(full_program,name.as_ptr(),&mut status)};
                        if status!=CL_SUCCESS || k.is_null(){break;}
                        kernels.push(k);
                    }
                    if kernels.len()==4 {
                        full=Some(FullProgram{program:full_program,qoi:kernels[0],tga:kernels[1],wbmp:kernels[2],bmp:kernels[3]});
                    } else {
                        for kernel in kernels {unsafe{(api.release_kernel)(kernel);}}
                        unsafe{(api.release_program)(full_program);}
                    }
                } else {unsafe{(api.release_program)(full_program);}}
            }
            let mut png_zlib = None;
            let source = include_str!("gpu_png_full.cl");
            let source_ptr = source.as_ptr() as *const c_char;
            let length = source.len();
            let png_program = unsafe { (api.create_program_with_source)(context,1,&source_ptr,&length,&mut status) };
            if status == CL_SUCCESS && !png_program.is_null() {
                if unsafe { (api.build_program)(png_program,1,&device,null(),None,null_mut()) } == CL_SUCCESS {
                    let png_kernel = unsafe { (api.create_kernel)(png_program,c"mata_png_full".as_ptr(),&mut status) };
                    if status == CL_SUCCESS && !png_kernel.is_null() {
                        png_zlib=Some(PngZlibProgram{program:png_program,kernel:png_kernel});
                    } else { unsafe {(api.release_program)(png_program);} }
                } else {unsafe {(api.release_program)(png_program);} }
            }
            let mut webp_full = None;
            let source = include_str!("gpu_webp_full.cl");
            let source_ptr = source.as_ptr() as *const c_char;
            let length = source.len();
            let webp_program=unsafe{(api.create_program_with_source)(context,1,&source_ptr,&length,&mut status)};
            if status==CL_SUCCESS && !webp_program.is_null(){
                if unsafe{(api.build_program)(webp_program,1,&device,null(),None,null_mut())}==CL_SUCCESS{
                    let wk=unsafe{(api.create_kernel)(webp_program,c"mata_webp_lossless_full".as_ptr(),&mut status)};
                    if status==CL_SUCCESS && !wk.is_null(){webp_full=Some(WebpFullProgram{program:webp_program,kernel:wk});}
                    else {unsafe{(api.release_program)(webp_program);}}
                } else {unsafe{(api.release_program)(webp_program);}}
            }
            Some(Self { api, context, queue, program, kernel, jpeg, pixels, full, png_zlib, webp_full, _library: library })
        }
        fn webp_lossless(&mut self, vp8l:&[u8],w:u32,h:u32)->Option<Vec<u32>> {
            let kernel=self.webp_full.as_ref()?.kernel;
            let count=(w as usize).checked_mul(h as usize)?;
            let result_len=count.checked_mul(4)?;
            if count==0 || count>u32::MAX as usize || vp8l.len()<6 || vp8l.len()>u32::MAX as usize {return None;}
            let src=self.input_buffer(vp8l.as_ptr() as *mut c_void,vp8l.len())?;
            let mut status=0;
            let dst=unsafe{(self.api.create_buffer)(self.context,CL_MEM_WRITE_ONLY,result_len,null_mut(),&mut status)};
            if status!=CL_SUCCESS||dst.is_null(){unsafe{(self.api.release_mem_object)(src);}return None;}
            let mut device_error=0u32;
            let err=unsafe{(self.api.create_buffer)(self.context,CL_MEM_READ_WRITE|CL_MEM_COPY_HOST_PTR,
                4,&mut device_error as *mut _ as *mut c_void,&mut status)};
            if status!=CL_SUCCESS || err.is_null(){unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);}return None;}
            let len=vp8l.len() as u32;
            let args: [(*const c_void,usize);6]=[
                (&src as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
                (&len as *const _ as *const c_void,4),
                (&dst as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
                (&w as *const _ as *const c_void,4),
                (&h as *const _ as *const c_void,4),
                (&err as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
            ];
            let mut ok=true;
            for (i,(pointer,size)) in args.into_iter().enumerate(){
                if unsafe{(self.api.set_kernel_arg)(kernel,i as u32,size,pointer)}!=CL_SUCCESS {ok=false;break;}
            }
            let one=1usize;
            if ok {ok=unsafe{(self.api.enqueue_nd_range_kernel)(self.queue,kernel,1,null(),&one,null(),0,null(),null_mut())}==CL_SUCCESS;}
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,err,CL_TRUE,0,4,&mut device_error as *mut _ as *mut c_void,0,null(),null_mut())}==CL_SUCCESS&&device_error==0;}
            let mut result=vec![0u32;count];
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,dst,CL_TRUE,0,result_len,result.as_mut_ptr() as *mut c_void,0,null(),null_mut())}==CL_SUCCESS;}
            unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);(self.api.release_mem_object)(err);}
            ok.then_some(result)
        }
        fn png_zlib(&mut self, zlib: &[u8], width:u32,height:u32,channels:u32)->Option<Vec<u32>> {
            let kernel=self.png_zlib.as_ref()?.kernel;
            let pixels=(width as usize).checked_mul(height as usize)?;
            let row=(width as usize).checked_mul(channels as usize)?;
            let raw_bytes=row.checked_add(1)?.checked_mul(height as usize)?;
            let out_bytes=pixels.checked_mul(4)?;
            if raw_bytes==0 || raw_bytes>u32::MAX as usize || out_bytes==0 ||
               zlib.is_empty() || zlib.len()>u32::MAX as usize {return None;}
            let src=self.input_buffer(zlib.as_ptr() as *mut c_void,zlib.len())?;
            let mut status=0;
            let dst=unsafe{(self.api.create_buffer)(self.context,CL_MEM_WRITE_ONLY,out_bytes,null_mut(),&mut status)};
            if status!=CL_SUCCESS || dst.is_null() {unsafe{(self.api.release_mem_object)(src);}return None;}
            let raw=unsafe{(self.api.create_buffer)(self.context,CL_MEM_READ_WRITE,raw_bytes,null_mut(),&mut status)};
            if status!=CL_SUCCESS || raw.is_null() {
                unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);}return None;
            }
            let mut device_error=0u32;
            let err=unsafe{(self.api.create_buffer)(self.context,CL_MEM_READ_WRITE|CL_MEM_COPY_HOST_PTR,4,&mut device_error as *mut _ as *mut c_void,&mut status)};
            if status!=CL_SUCCESS || err.is_null() {
                unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);(self.api.release_mem_object)(raw);}return None;
            }
            let len=zlib.len() as u32;
            let args: [(*const c_void,usize);8]=[
                (&src as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
                (&len as *const _ as *const c_void,4),
                (&dst as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
                (&raw as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
                (&width as *const _ as *const c_void,4),
                (&height as *const _ as *const c_void,4),
                (&channels as *const _ as *const c_void,4),
                (&err as *const _ as *const c_void,std::mem::size_of::<ClHandle>()),
            ];
            let mut ok=true;
            for (i,(ptr,size)) in args.into_iter().enumerate() {
                if unsafe{(self.api.set_kernel_arg)(kernel,i as u32,size,ptr)}!=CL_SUCCESS {ok=false;break;}
            }
            let one=1usize;
            if ok {ok=unsafe{(self.api.enqueue_nd_range_kernel)(self.queue,kernel,1,null(),&one,null(),0,null(),null_mut())}==CL_SUCCESS;}
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,err,CL_TRUE,0,4,&mut device_error as *mut _ as *mut c_void,0,null(),null_mut())}==CL_SUCCESS && device_error==0;}
            let mut output=vec![0u32;pixels];
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,dst,CL_TRUE,0,out_bytes,output.as_mut_ptr() as *mut c_void,0,null(),null_mut())}==CL_SUCCESS;}
            unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);(self.api.release_mem_object)(raw);(self.api.release_mem_object)(err);}
            ok.then_some(output)
        }
        fn jpeg_idct(&mut self, coefficients: &[i32]) -> Option<Vec<u8>> {
            let kernel = self.jpeg.as_ref()?.kernel;
            let block_count = coefficients.len().checked_div(64)?;
            if block_count == 0 || coefficients.len() % 64 != 0 || block_count > u32::MAX as usize {
                return None;
            }
            let source_bytes = coefficients.len().checked_mul(std::mem::size_of::<i32>())?;
            let output_bytes = block_count.checked_mul(64)?;
            let mut status = 0;
            let src = unsafe { (self.api.create_buffer)(self.context,
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR, source_bytes,
                coefficients.as_ptr() as *mut c_void, &mut status) };
            if status != CL_SUCCESS || src.is_null() { return None; }
            let dst = unsafe { (self.api.create_buffer)(self.context,
                CL_MEM_WRITE_ONLY, output_bytes, null_mut(), &mut status) };
            if status != CL_SUCCESS || dst.is_null() {
                unsafe { (self.api.release_mem_object)(src); }
                return None;
            }
            let count = block_count as u32;
            let args = [
                (&src as *const ClHandle as *const c_void, std::mem::size_of::<ClHandle>()),
                (&dst as *const ClHandle as *const c_void, std::mem::size_of::<ClHandle>()),
                (&count as *const u32 as *const c_void, std::mem::size_of::<u32>()),
            ];
            let mut worked = true;
            for (i, (value, size)) in args.into_iter().enumerate() {
                if unsafe { (self.api.set_kernel_arg)(kernel, i as u32, size, value) } != CL_SUCCESS {
                    worked = false;
                    break;
                }
            }
            if worked {
                worked = unsafe { (self.api.enqueue_nd_range_kernel)(self.queue, kernel, 1,
                    null(), &block_count, null(), 0, null(), null_mut()) } == CL_SUCCESS;
            }
            let mut output = vec![0u8; output_bytes];
            if worked {
                worked = unsafe { (self.api.enqueue_read_buffer)(self.queue, dst, CL_TRUE, 0,
                    output_bytes, output.as_mut_ptr() as *mut c_void, 0, null(), null_mut()) } == CL_SUCCESS;
            }
            unsafe { (self.api.release_mem_object)(dst); (self.api.release_mem_object)(src); }
            worked.then_some(output)
        }
        fn full_decode(&mut self, input: &[u8], pixel_count:usize, width:u32,height:u32,
            offset:u32,mode:u32,format:ImageFormat)->Option<Vec<u32>> {
            let kernels=self.full.as_ref()?;
            let kernel=match format {
                ImageFormat::Qoi => kernels.qoi,
                ImageFormat::Tga => kernels.tga,
                ImageFormat::Wbmp => kernels.wbmp,
                ImageFormat::Bmp => kernels.bmp,
                _ => return None,
            };
            let src_len=u32::try_from(input.len()).ok()?;
            let dst_bytes=pixel_count.checked_mul(4)?;
            let src=self.input_buffer(input.as_ptr() as *mut c_void,input.len())?;
            let mut status=0;
            let dst=unsafe{(self.api.create_buffer)(self.context,CL_MEM_WRITE_ONLY,dst_bytes,null_mut(),&mut status)};
            if status!=CL_SUCCESS || dst.is_null() {
                unsafe{(self.api.release_mem_object)(src);} return None;
            }
            let mut device_error=0u32;
            let err=unsafe{(self.api.create_buffer)(self.context,
                1 | CL_MEM_COPY_HOST_PTR,4,&mut device_error as *mut _ as *mut c_void,&mut status)};
            if status!=CL_SUCCESS || err.is_null() {
                unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);}return None;
            }
            let mut ok=true;
            // Signature: input, length, output, width, height, offset, mode, error.
            // Argument indices are deliberately explicit to avoid ABI drift.
            for (index,ptr) in [(&src as *const ClHandle as *const c_void),
                (&src_len as *const u32 as *const c_void),
                (&dst as *const ClHandle as *const c_void),
                (&width as *const u32 as *const c_void),
                (&height as *const u32 as *const c_void),
                (&offset as *const u32 as *const c_void),
                (&mode as *const u32 as *const c_void),
                (&err as *const ClHandle as *const c_void)].into_iter().enumerate() {
                let size=if index==0 || index==2 || index==7 {std::mem::size_of::<ClHandle>()}else{4};
                if unsafe{(self.api.set_kernel_arg)(kernel,index as u32,size,ptr)}!=CL_SUCCESS {ok=false;break;}
            }
            // QOI and RLE TGA have cross-pixel dependencies: run one work-item.
            let global=if matches!(format,ImageFormat::Qoi) ||
                (matches!(format,ImageFormat::Tga) && (mode&0x100)!=0) {1usize}else{pixel_count};
            if ok {ok=unsafe{(self.api.enqueue_nd_range_kernel)(self.queue,kernel,1,null(),&global,null(),0,null(),null_mut())}==CL_SUCCESS;}
            let mut output=vec![0u32;pixel_count];
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,err,CL_TRUE,0,4,
                &mut device_error as *mut _ as *mut c_void,0,null(),null_mut())}==CL_SUCCESS && device_error==0;}
            if ok {ok=unsafe{(self.api.enqueue_read_buffer)(self.queue,dst,CL_TRUE,0,dst_bytes,
                output.as_mut_ptr() as *mut c_void,0,null(),null_mut())}==CL_SUCCESS;}
            unsafe{(self.api.release_mem_object)(src);(self.api.release_mem_object)(dst);(self.api.release_mem_object)(err);}
            ok.then_some(output)
        }
        // Buffers always release even on driver errors. Queues and mutable
        // kernel arguments are serialized by the session's mutex.
        fn input_buffer(&self, ptr: *mut c_void, len: usize) -> Option<ClHandle> {
            if len == 0 { return None; }
            let mut status = 0;
            let handle = unsafe { (self.api.create_buffer)(self.context,
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR, len, ptr, &mut status) };
            (status == CL_SUCCESS && !handle.is_null()).then_some(handle)
        }
        fn process_pixels(&mut self, kernel: ClHandle, inputs: &[(ClHandle, usize)],
                          scalars: &[u32], count: usize) -> Option<Vec<u32>> {
            if count == 0 || count > u32::MAX as usize { return None; }
            let bytes = count.checked_mul(4)?;
            let mut status = 0;
            let dst = unsafe { (self.api.create_buffer)(self.context, CL_MEM_WRITE_ONLY,
                bytes, null_mut(), &mut status) };
            if status != CL_SUCCESS || dst.is_null() { return None; }
            let mut arg = 0u32;
            let mut ok = true;
            for &(buf, _) in inputs {
                if unsafe { (self.api.set_kernel_arg)(kernel, arg, std::mem::size_of::<ClHandle>(),
                    &buf as *const _ as *const c_void) } != CL_SUCCESS { ok = false; break; }
                arg += 1;
            }
            if ok {
                ok = unsafe { (self.api.set_kernel_arg)(kernel, arg, std::mem::size_of::<ClHandle>(),
                    &dst as *const _ as *const c_void) } == CL_SUCCESS;
                arg += 1;
            }
            if ok {
                for &scalar in scalars {
                    if unsafe { (self.api.set_kernel_arg)(kernel, arg, 4,
                        &scalar as *const u32 as *const c_void) } != CL_SUCCESS { ok = false; break; }
                    arg += 1;
                }
            }
            if ok {
                ok = unsafe { (self.api.enqueue_nd_range_kernel)(self.queue, kernel, 1,
                    null(), &count, null(), 0, null(), null_mut()) } == CL_SUCCESS;
            }
            let mut result = vec![0u32; count];
            if ok {
                ok = unsafe { (self.api.enqueue_read_buffer)(self.queue, dst, CL_TRUE, 0,
                    bytes, result.as_mut_ptr() as *mut c_void, 0, null(), null_mut()) } == CL_SUCCESS;
            }
            unsafe { (self.api.release_mem_object)(dst); }
            ok.then_some(result)
        }
        fn png_unpack(&mut self, raw: &[u8], channels: u32) -> Option<Vec<u32>> {
            let count = raw.len().checked_div(channels as usize)?;
            let kernel = self.pixels.as_ref()?.png;
            if count > u32::MAX as usize { return None; }
            let src = self.input_buffer(raw.as_ptr() as *mut c_void, raw.len())?;
            let result = self.process_pixels(kernel, &[(src, raw.len())], &[count as u32, channels], count);
            unsafe { (self.api.release_mem_object)(src); }
            result
        }
        fn webp_subgreen(&mut self, source: &[u32]) -> Option<Vec<u32>> {
            let count = source.len();
            if count > u32::MAX as usize { return None; }
            let kernel = self.pixels.as_ref()?.vp8l;
            let src = self.input_buffer(source.as_ptr() as *mut c_void, count.checked_mul(4)?)?;
            let result = self.process_pixels(kernel, &[(src, count * 4)], &[count as u32], count);
            unsafe { (self.api.release_mem_object)(src); }
            result
        }
        fn webp_yuv420(&mut self, y: &[u8], u: &[u8], v: &[u8], a: Option<&[u8]>,
                       width: usize, height: usize, ys: usize, us: usize) -> Option<Vec<u32>> {
            let count = width.checked_mul(height)?;
            let uvw = width.checked_add(1)? / 2;
            let uvh = height.checked_add(1)? / 2;
            if count > u32::MAX as usize || width > u32::MAX as usize ||
                ys > u32::MAX as usize || us > u32::MAX as usize ||
                ys < width || us < uvw ||
                y.len() < ys.checked_mul(height)? ||
                u.len() < us.checked_mul(uvh)? ||
                v.len() < us.checked_mul(uvh)? ||
                a.is_some_and(|alpha| alpha.len() < count) { return None; }
            let kernel = self.pixels.as_ref()?.vp8;
            let alpha_dummy = [255u8];
            let a = a.unwrap_or(&alpha_dummy);
            let data = [y, u, v, a];
            let mut handles = Vec::new();
            for source in data {
                match self.input_buffer(source.as_ptr() as *mut c_void, source.len()) {
                    Some(handle) => handles.push((handle, source.len())),
                    None => break,
                }
            }
            let result = if handles.len() == 4 {
                self.process_pixels(kernel, &handles, &[width as u32, ys as u32, us as u32,
                    u32::from(a.len() != 1), count as u32], count)
            } else { None };
            for (handle, _) in handles { unsafe { (self.api.release_mem_object)(handle); } }
            result
        }
        fn resize(&mut self, input: &[u32], sw: u32, sh: u32, dw: u32, dh: u32) -> Option<Vec<u32>> {
            let in_len = (sw as usize).checked_mul(sh as usize)?;
            let out_len = (dw as usize).checked_mul(dh as usize)?;
            if input.len() != in_len || out_len == 0 || out_len > u32::MAX as usize { return None; }
            let in_bytes = in_len.checked_mul(4)?;
            let out_bytes = out_len.checked_mul(4)?;
            let mut status = 0;
            let src = unsafe { (self.api.create_buffer)(self.context,CL_MEM_READ_ONLY|CL_MEM_COPY_HOST_PTR,in_bytes,input.as_ptr() as *mut c_void,&mut status) };
            if status != CL_SUCCESS || src.is_null() { return None; }
            let dst = unsafe { (self.api.create_buffer)(self.context,CL_MEM_WRITE_ONLY,out_bytes,null_mut(),&mut status) };
            if status != CL_SUCCESS || dst.is_null() {
                unsafe { (self.api.release_mem_object)(src); } return None;
            }
            let mut result = vec![0u32; out_len];
            let count = out_len as u32;
            let params: [*const c_void; 7] = [
                &src as *const _ as *const c_void,
                &dst as *const _ as *const c_void,
                &sw as *const _ as *const c_void,
                &sh as *const _ as *const c_void,
                &dw as *const _ as *const c_void,
                &dh as *const _ as *const c_void,
                &count as *const _ as *const c_void,
            ];
            let mut worked = true;
            for (index, &pointer) in params.iter().enumerate() {
                let size = if index < 2 { std::mem::size_of::<ClHandle>() } else { std::mem::size_of::<u32>() };
                if unsafe { (self.api.set_kernel_arg)(self.kernel,index as u32,size,pointer) } != CL_SUCCESS { worked = false; break; }
            }
            if worked {
                let global = out_len;
                worked = unsafe { (self.api.enqueue_nd_range_kernel)(self.queue,self.kernel,1,null(),&global,null(),0,null(),null_mut()) } == CL_SUCCESS;
            }
            if worked {
                worked = unsafe { (self.api.enqueue_read_buffer)(self.queue,dst,CL_TRUE,0,out_bytes,result.as_mut_ptr() as *mut c_void,0,null(),null_mut()) } == CL_SUCCESS;
            }
            unsafe { (self.api.release_mem_object)(dst); (self.api.release_mem_object)(src); }
            worked.then_some(result)
        }
    }

    static INTEL: OnceLock<Option<Mutex<Session>>> = OnceLock::new();
    static AMD: OnceLock<Option<Mutex<Session>>> = OnceLock::new();
    static NVIDIA: OnceLock<Option<Mutex<Session>>> = OnceLock::new();
    fn session(backend: DecodeBackend) -> Option<&'static Mutex<Session>> {
        let cache = match backend {
            DecodeBackend::Cpu => return None,
            DecodeBackend::IntelGpu => &INTEL,
            DecodeBackend::AmdGpu => &AMD,
            DecodeBackend::NvidiaGpu => &NVIDIA,
        };
        cache.get_or_init(|| Session::create(backend).map(Mutex::new)).as_ref()
    }
    pub(super) fn full_decode(input:&[u8],count:usize,w:u32,h:u32,offset:u32,mode:u32,
        format:ImageFormat,backend:DecodeBackend)->Option<Vec<u32>> {
        session(backend)?.lock().ok()?.full_decode(input,count,w,h,offset,mode,format)
    }
    pub(super) fn webp_lossless(input:&[u8],w:u32,h:u32,backend:DecodeBackend)->Option<Vec<u32>> {
        session(backend)?.lock().ok()?.webp_lossless(input,w,h)
    }
    pub(super) fn png_zlib(input:&[u8],w:u32,h:u32,ch:u32,backend:DecodeBackend)->Option<Vec<u32>> {
        session(backend)?.lock().ok()?.png_zlib(input,w,h,ch)
    }
    pub(super) fn available(backend: DecodeBackend) -> bool { session(backend).is_some() }
    pub(super) fn pixel_kernels_available(backend: DecodeBackend) -> bool {
        session(backend).and_then(|s| s.lock().ok()).is_some_and(|s| s.pixels.is_some())
    }
    pub(super) fn jpeg_idct_available(backend: DecodeBackend) -> bool {
        session(backend).and_then(|s| s.lock().ok()).is_some_and(|s| s.jpeg.is_some())
    }
    pub(super) fn jpeg_idct(coefficients: &[i32], backend: DecodeBackend) -> Option<Vec<u8>> {
        session(backend)?.lock().ok()?.jpeg_idct(coefficients)
    }
    pub(super) fn png_unpack(raw: &[u8], channels: u32, backend: DecodeBackend) -> Option<Vec<u32>> {
        session(backend)?.lock().ok()?.png_unpack(raw, channels)
    }
    pub(super) fn webp_subgreen(pixels: &[u32], backend: DecodeBackend) -> Option<Vec<u32>> {
        session(backend)?.lock().ok()?.webp_subgreen(pixels)
    }
    pub(super) fn webp_yuv420(y: &[u8], u: &[u8], v: &[u8], a: Option<&[u8]>,
        width: usize, height: usize, ys: usize, us: usize, backend: DecodeBackend) -> Option<Vec<u32>> {
        session(backend)?.lock().ok()?.webp_yuv420(y, u, v, a, width, height, ys, us)
    }
    pub(super) fn resize(input: &[u32], sw: u32, sh: u32, dw: u32, dh: u32, backend: DecodeBackend) -> Option<Vec<u32>> {
        let mut state = session(backend)?.lock().ok()?;
        state.resize(input,sw,sh,dw,dh)
    }
}

#[cfg(not(all(feature = "gpu-opencl", any(target_os = "windows", target_os = "linux"))))]
mod opencl {
    use super::{DecodeBackend,ImageFormat};
    pub(super) fn full_decode(_:&[u8],_:usize,_:u32,_:u32,_:u32,_:u32,_:ImageFormat,_:DecodeBackend)->Option<Vec<u32>> {None}
    pub(super) fn webp_lossless(_:&[u8],_:u32,_:u32,_:DecodeBackend)->Option<Vec<u32>> {None}
    pub(super) fn png_zlib(_:&[u8],_:u32,_:u32,_:u32,_:DecodeBackend)->Option<Vec<u32>> {None}
    pub(super) fn available(_backend: DecodeBackend) -> bool { false }
    pub(super) fn pixel_kernels_available(_backend: DecodeBackend) -> bool { false }
    pub(super) fn jpeg_idct_available(_backend: DecodeBackend) -> bool { false }
    pub(super) fn jpeg_idct(_coefficients: &[i32], _backend: DecodeBackend) -> Option<Vec<u8>> { None }
    pub(super) fn png_unpack(_raw: &[u8], _channels: u32, _backend: DecodeBackend) -> Option<Vec<u32>> { None }
    pub(super) fn webp_subgreen(_pixels: &[u32], _backend: DecodeBackend) -> Option<Vec<u32>> { None }
    pub(super) fn webp_yuv420(_y: &[u8], _u: &[u8], _v: &[u8], _a: Option<&[u8]>,
        _width: usize, _height: usize, _ys: usize, _us: usize, _backend: DecodeBackend) -> Option<Vec<u32>> { None }
    pub(super) fn resize(_input: &[u32], _sw:u32, _sh:u32, _dw:u32, _dh:u32, _backend:DecodeBackend) -> Option<Vec<u32>> { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_gpu_request_falls_back_to_exact_cpu_output() {
        let mut image = Image { width: 3, height: 2, pixels: vec![1,2,3,4,5,6], frames: vec![] };
        let mut cpu = image.clone();
        cpu.rescale(7, 5);
        assert!(!rescale(&mut image, 7, 5, DecodeBackend::NvidiaGpu));
        assert_eq!(cpu, image);
    }
    #[test]
    fn gpu_candidates_exclude_simple_codecs() {
        assert!(gpu_rescale_candidate(ImageFormat::Png));
        assert!(gpu_rescale_candidate(ImageFormat::Jpeg));
        assert!(!gpu_rescale_candidate(ImageFormat::Qoi));
        assert!(!gpu_rescale_candidate(ImageFormat::Svg));
    }
}
