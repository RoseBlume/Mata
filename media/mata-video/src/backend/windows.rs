use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr,
    time::Duration,
};

use windows::{
    core::{GUID, PCWSTR},
    Win32::{
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11CreateDevice, D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
                ID3D11Device,
            },
        },
        Media::MediaFoundation::{
            MFCreateDXGIDeviceManager,
            IMFDXGIDeviceManager,
            MF_SOURCE_READER_D3D_MANAGER,
            MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
            IMFMediaBuffer,
            IMFMediaType,
            IMFSourceReader,
            MFCreateAttributes,
            MFCreateMediaType,
            MFCreateSourceReaderFromURL,
            MFShutdown,
            MFStartup,
            MFMediaType_Video,
            MFVideoFormat_ARGB32,
            MFVideoFormat_RGB32,
            MFVideoFormat_NV12,
            MFVideoFormat_YUY2,
            MFSTARTUP_FULL,
            MF_VERSION,
            MF_MT_DEFAULT_STRIDE,
            MF_MT_FRAME_RATE,
            MF_MT_FRAME_SIZE,
            MF_PD_DURATION,
            MF_SOURCE_READER_MEDIASOURCE,
            MF_MT_MAJOR_TYPE,
            MF_MT_SUBTYPE,
            MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
            MF_SOURCE_READERF_ENDOFSTREAM,
            MF_SOURCE_READER_ALL_STREAMS,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM,
            MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
        },
        System::Com::{
            CoInitializeEx,
            CoUninitialize,
            COINIT_MULTITHREADED,
        },
    },
};

use crate::{
    PixelFormat,
    VideoError,
    VideoFrame,
    VideoInfo,
    SeekMode,
};

const HUNDRED_NANOSECONDS_PER_SECOND: u64 = 10_000_000;

// Media Foundation's RGB32 contains an unused high byte, whereas ARGB32
// contains an alpha channel. Both store B,G,R,[X/A] in memory on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Rgb32,
    Argb32,
    Nv12,
    Yuy2,
}

impl OutputFormat {
    fn subtype(self) -> GUID {
        match self {
            Self::Rgb32 => MFVideoFormat_RGB32,
            Self::Argb32 => MFVideoFormat_ARGB32,
            Self::Nv12 => MFVideoFormat_NV12,
            Self::Yuy2 => MFVideoFormat_YUY2,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Rgb32 => "RGB32",
            Self::Argb32 => "ARGB32",
            Self::Nv12 => "NV12",
            Self::Yuy2 => "YUY2",
        }
    }

    fn min_row_bytes(self, width: usize) -> Option<usize> {
        match self {
            Self::Rgb32 | Self::Argb32 => width.checked_mul(4),
            Self::Nv12 => width.checked_add(1).map(|x| x & !1),
            Self::Yuy2 => width.checked_add(1).and_then(|x| (x & !1).checked_mul(2)),
        }
    }
}


pub(crate) struct PlatformDecoder {
    reader: IMFSourceReader,
    info: VideoInfo,
    stride: i32,
    output_format: OutputFormat,
    com_initialized: bool,
    mf_initialized: bool,
    ended: bool,
    original_size: (u32, u32),
    // Keep the D3D device and DXGI manager alive for the source reader.
    _hardware: Option<HardwareDevice>,
}

struct HardwareDevice {
    _device: ID3D11Device,
    _manager: IMFDXGIDeviceManager,
}

/// GPU use is opportunistic: unsupported adapters/codecs retain software decode.
fn setup_hardware(attributes: &windows::Win32::Media::MediaFoundation::IMFAttributes)
    -> Option<HardwareDevice>
{
    use windows::Win32::Foundation::HMODULE;
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        ).ok()?;
        let device = device?;
        let mut reset_token = 0u32;
        let mut manager = None;
        MFCreateDXGIDeviceManager(&mut reset_token, &mut manager).ok()?;
        let manager = manager?;
        manager.ResetDevice(&device, reset_token).ok()?;
        attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &manager).ok()?;
        attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).ok()?;
        // Device context is safe to drop: the manager owns a reference to the device.
        Some(HardwareDevice { _device: device, _manager: manager })
    }
}

impl PlatformDecoder {
    pub(crate) fn open(path: &Path) -> Result<Self, VideoError> {
        if !path.exists() {
            return Err(VideoError::new(format!(
                "video file does not exist: {}",
                path.display(),
            )));
        }

        let mut com_initialized = false;
        let mut mf_initialized = false;

        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_ok() {
                com_initialized = true;
            } else if hr.0 != windows::Win32::Foundation::RPC_E_CHANGED_MODE.0 {
                return Err(VideoError::new(format!(
                    "failed to initialize COM: {hr:?}",
                )));
            }

            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                if com_initialized {
                    CoUninitialize();
                }
                return Err(VideoError::new(format!(
                    "failed to initialize Media Foundation: {error}",
                )));
            }
            mf_initialized = true;

            // H.264/HEVC decoders often output NV12, not ARGB32. Without
            // this flag the Source Reader may refuse RGB formats with
            // MF_E_INVALIDMEDIATYPE (0xC00D36B4). Enabling its converter
            // allows decoded YUV frames to be converted to RGB32.
            let mut attributes = None;
            if let Err(error) = MFCreateAttributes(&mut attributes, 1) {
                cleanup_runtime(com_initialized, mf_initialized);
                return Err(VideoError::new(format!(
                    "failed to create Media Foundation reader attributes: {error}",
                )));
            }
            let attributes = match attributes {
                Some(attributes) => attributes,
                None => {
                    cleanup_runtime(com_initialized, mf_initialized);
                    return Err(VideoError::new("Media Foundation returned no reader attributes"));
                }
            };
            if let Err(error) = attributes.SetUINT32(
                &MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1,
            ) {
                cleanup_runtime(com_initialized, mf_initialized);
                return Err(VideoError::new(format!(
                    "failed to enable Media Foundation video conversion: {error}",
                )));
            }

            // Keep the manager alive for the lifetime of Source Reader.
            // If GPU initialization fails, retain the existing software path.
            let hardware = if std::env::var_os("MATA_VIDEO_DISABLE_GPU").is_some() {
                None
            } else {
                setup_hardware(&attributes)
            };
            let wide_path = wide_null(path.as_os_str());
            let reader = match MFCreateSourceReaderFromURL(
                PCWSTR(wide_path.as_ptr()),
                &attributes,
            ) {
                Ok(reader) => reader,
                Err(error) => {
                    cleanup_runtime(com_initialized, mf_initialized);
                    return Err(VideoError::new(format!(
                        "failed to open video with Media Foundation: {error}",
                    )));
                }
            };

            let all_streams = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
            let video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

            if let Err(error) = reader.SetStreamSelection(all_streams, false) {
                cleanup_runtime(com_initialized, mf_initialized);
                return Err(VideoError::new(format!(
                    "failed to disable Media Foundation streams: {error}",
                )));
            }

            if let Err(error) = reader.SetStreamSelection(video_stream, true) {
                cleanup_runtime(com_initialized, mf_initialized);
                return Err(VideoError::new(format!(
                    "failed to select the video stream: {error}",
                )));
            }

            // RGB32 is Media Foundation's documented RGB conversion target.
            // The library still returns 0xAARRGGBB; its opaque alpha is
            // inserted while copying. If a decoder cannot convert to RGB,
            // decode NV12/YUY2 and convert them in this library as a fallback.
            let output_format = match negotiate_output_format(&reader, video_stream) {
                Ok(format) => format,
                Err(error) => {
                    cleanup_runtime(com_initialized, mf_initialized);
                    return Err(error);
                }
            };

            let current_type = match reader.GetCurrentMediaType(video_stream) {
                Ok(media_type) => media_type,
                Err(error) => {
                    cleanup_runtime(com_initialized, mf_initialized);
                    return Err(VideoError::new(format!(
                        "failed to read Media Foundation video format: {error}",
                    )));
                }
            };

            let (mut info, stride, actual_format) = match layout_from_media_type(&current_type) {
                Ok(layout) => layout,
                Err(error) => {
                    cleanup_runtime(com_initialized, mf_initialized);
                    return Err(error);
                }
            };

            // Container duration is stored in a PROPVARIANT with 100ns ticks.
            // Not all sources report a duration (for example, live streams).
            info.duration = reader.GetPresentationAttribute(
                MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION,
            ).ok().and_then(|value| i64::try_from(&value).ok())
                .and_then(hns_i64_to_duration);

            // It is important to use the *actual* output subtype: the
            // negotiated layout determines the byte counts and channel order.
            debug_assert_eq!(output_format, actual_format);
            let original_size = (info.width, info.height);
            Ok(Self {
                reader,
                info,
                stride,
                output_format: actual_format,
                com_initialized,
                mf_initialized,
                ended: false,
                original_size,
                _hardware: hardware,
            })
        }
    }

    #[inline]
    pub(crate) fn info(&self) -> VideoInfo {
        self.info
    }

    pub(crate) fn seek(&mut self, position: Duration, _mode: SeekMode) -> Result<(), VideoError> {
        // Source Reader seeks through the source's native index. Media
        // Foundation does not expose a KEY_UNIT flag equivalent to GStreamer
        // here; it is already permitted to resume from an earlier keyframe.
        let ticks = position.as_nanos() / 100;
        let ticks = i64::try_from(ticks).map_err(|_| VideoError::new("seek position exceeds Media Foundation range"))?;
        let position = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::from(ticks);
        unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &position) }
            .map_err(|error| VideoError::new(format!("Media Foundation seek failed: {error}")))?;
        self.ended = false;
        Ok(())
    }

    pub(crate) fn set_output_resolution(&mut self, resolution: Option<(u32, u32)>) -> Result<(), VideoError> {
        let (width, height) = resolution.unwrap_or(self.original_size);
        // Advanced video processing may resize before copying to CPU memory.
        // A driver can reject it; the public decoder's scaling fallback
        // guarantees the requested output size in that case.
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let media_type = create_output_media_type(self.output_format)?;
        let frame_size = (u64::from(width) << 32) | u64::from(height);
        if unsafe { media_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size) }.is_ok()
            && unsafe { self.reader.SetCurrentMediaType(stream, None, &media_type) }.is_ok()
        {
            if let Ok(current) = unsafe { self.reader.GetCurrentMediaType(stream) } {
                if let Ok((mut info, stride, format)) = layout_from_media_type(&current) {
                    info.duration = self.info.duration;
                    self.info = info;
                    self.stride = stride;
                    self.output_format = format;
                }
            }
        }
        self.ended = false;
        Ok(())
    }

    pub(crate) fn next_frame(&mut self) -> Result<Option<VideoFrame>, VideoError> {
        self.next_frame_at_or_after(Duration::ZERO).map(|(frame, _)| frame)
    }

    /// Read native samples and skip preroll *before* creating/copying pixels.
    pub(crate) fn next_frame_at_or_after(&mut self, min_pts: Duration)
        -> Result<(Option<VideoFrame>, u64), VideoError>
    {
        if self.ended { return Ok((None, 0)); }
        let mut skipped = 0u64;
        let video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

        loop {
            let mut stream_flags = 0u32;
            let mut timestamp_100ns = 0i64;
            let mut sample = None;

            unsafe {
                self.reader
                    .ReadSample(
                        video_stream,
                        0,
                        None,
                        Some(&mut stream_flags),
                        Some(&mut timestamp_100ns),
                        Some(&mut sample),
                    )
                    .map_err(|error| {
                        VideoError::new(format!(
                            "Media Foundation failed to decode a video sample: {error}",
                        ))
                    })?;
            }

            if stream_flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                self.ended = true;
                return Ok((None, skipped));
            }

            if stream_flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                let current_type = unsafe {
                    self.reader.GetCurrentMediaType(video_stream)
                }
                .map_err(|error| {
                    VideoError::new(format!(
                        "failed to read changed Media Foundation video format: {error}",
                    ))
                })?;
                let (info, stride, output_format) = layout_from_media_type(&current_type)?;
                let mut info = info;
                info.duration = self.info.duration;
                self.info = info;
                self.stride = stride;
                self.output_format = output_format;
            }

            let Some(sample) = sample else {
                // Stream ticks and format-change notifications may legitimately
                // arrive without an accompanying sample.
                continue;
            };

            // The Source Reader returns the presentation timestamp alongside
            // the sample. Dropping earlier samples here avoids expensive
            // ConvertToContiguousBuffer + YUV/RGB pixel conversion.
            let timestamp = hns_i64_to_duration(timestamp_100ns)
                .unwrap_or(Duration::ZERO);
            if timestamp < min_pts {
                skipped = skipped.saturating_add(1);
                continue;
            }
            let sample_duration = unsafe { sample.GetSampleDuration().ok() }
                .and_then(hns_i64_to_duration);

            let contiguous = unsafe { sample.ConvertToContiguousBuffer() }
                .map_err(|error| {
                    VideoError::new(format!(
                        "failed to obtain contiguous Media Foundation frame data: {error}",
                    ))
                })?;

            let pixels = copy_media_buffer(
                &contiguous,
                self.info.width,
                self.info.height,
                self.stride,
                self.output_format,
            )?;

            return Ok((Some(VideoFrame {
                width: self.info.width,
                height: self.info.height,
                pixels,
                timestamp,
                duration: sample_duration,
            }), skipped));
        }
    }
}

impl Drop for PlatformDecoder {
    fn drop(&mut self) {
        cleanup_runtime(self.com_initialized, self.mf_initialized);
    }
}

fn cleanup_runtime(com_initialized: bool, mf_initialized: bool) {
    unsafe {
        if mf_initialized {
            let _ = MFShutdown();
        }
        if com_initialized {
            CoUninitialize();
        }
    }
}

fn create_output_media_type(format: OutputFormat) -> Result<IMFMediaType, VideoError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(|error| {
        VideoError::new(format!("failed to create Media Foundation media type: {error}"))
    })?;

    unsafe { media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video) }
        .map_err(|error| {
            VideoError::new(format!("failed to set Media Foundation major type: {error}"))
        })?;
    unsafe { media_type.SetGUID(&MF_MT_SUBTYPE, &format.subtype()) }
        .map_err(|error| {
            VideoError::new(format!("failed to request Media Foundation {}: {error}", format.name()))
        })?;

    Ok(media_type)
}

fn negotiate_output_format(reader: &IMFSourceReader, stream: u32) -> Result<OutputFormat, VideoError> {
    let mut failures = Vec::new();
    for format in [
        OutputFormat::Rgb32,
        OutputFormat::Argb32,
        OutputFormat::Nv12,
        OutputFormat::Yuy2,
    ] {
        let media_type = create_output_media_type(format)?;
        match unsafe { reader.SetCurrentMediaType(stream, None, &media_type) } {
            Ok(()) => return Ok(format),
            Err(error) => failures.push(format!("{}: {error}", format.name())),
        }
    }
    Err(VideoError::new(format!(
        "Media Foundation could not negotiate any decoded video format ({})",
        failures.join("; "),
    )))
}

fn output_format_from_media_type(media_type: &IMFMediaType) -> Result<OutputFormat, VideoError> {
    let subtype = unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) }.map_err(|error| {
        VideoError::new(format!("Media Foundation did not report output subtype: {error}"))
    })?;
    for format in [
        OutputFormat::Rgb32,
        OutputFormat::Argb32,
        OutputFormat::Nv12,
        OutputFormat::Yuy2,
    ] {
        if subtype == format.subtype() {
            return Ok(format);
        }
    }
    Err(VideoError::new(format!(
        "Media Foundation returned an unsupported decoded video subtype: {subtype:?}",
    )))
}

fn layout_from_media_type(media_type: &IMFMediaType) -> Result<(VideoInfo, i32, OutputFormat), VideoError> {
    let output_format = output_format_from_media_type(media_type)?;
    let frame_size = unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }.map_err(|error| {
        VideoError::new(format!("Media Foundation did not report frame size: {error}"))
    })?;
    let width = (frame_size >> 32) as u32;
    let height = frame_size as u32;

    if width == 0 || height == 0 {
        return Err(VideoError::new("Media Foundation returned zero-sized video frames"));
    }

    let (frame_rate_numerator, frame_rate_denominator) = unsafe { media_type
        .GetUINT64(&MF_MT_FRAME_RATE) }
        .map(|rate| ((rate >> 32) as u32, rate as u32))
        .unwrap_or((0, 0));

    let fallback_stride = output_format.min_row_bytes(width as usize)
        .ok_or_else(|| VideoError::new("video frame stride overflow"))?;
    if fallback_stride > i32::MAX as usize {
        return Err(VideoError::new("video frame stride exceeds supported size"));
    }
    let stride = unsafe { media_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
        .map(|value| value as i32)
        .unwrap_or(fallback_stride as i32);

    Ok((VideoInfo {
        width,
        height,
        frame_rate_numerator,
        frame_rate_denominator,
        duration: None,
        pixel_format: PixelFormat::Argb32,
    }, stride, output_format))
}

fn copy_media_buffer(
    buffer: &IMFMediaBuffer,
    width: u32,
    height: u32,
    stride: i32,
    output_format: OutputFormat,
) -> Result<Vec<u32>, VideoError> {
    let width = width as usize;
    let height = height as usize;

    let mut raw = ptr::null_mut();
    let mut current_length = 0u32;

    unsafe {
        buffer
            .Lock(&mut raw, None, Some(&mut current_length))
            .map_err(|error| {
                VideoError::new(format!(
                    "failed to lock Media Foundation frame buffer: {error}",
                ))
            })?;
    }

    struct UnlockGuard<'a>(&'a IMFMediaBuffer);
    impl Drop for UnlockGuard<'_> {
        fn drop(&mut self) {
            unsafe {
                let _ = self.0.Unlock();
            }
        }
    }
    let _unlock = UnlockGuard(buffer);

    if raw.is_null() {
        return Err(VideoError::new("Media Foundation returned a null frame buffer"));
    }

    let bytes = unsafe { std::slice::from_raw_parts(raw, current_length as usize) };
    copy_frame_bytes(bytes, width, height, stride, output_format)
}

fn copy_frame_bytes(
    bytes: &[u8],
    width: usize,
    height: usize,
    stride: i32,
    output_format: OutputFormat,
) -> Result<Vec<u32>, VideoError> {
    let row_bytes = output_format.min_row_bytes(width)
        .ok_or_else(|| VideoError::new("video row size overflow"))?;
    let stride_abs = if stride == i32::MIN {
        return Err(VideoError::new("Media Foundation returned an invalid frame stride"));
    } else {
        stride.unsigned_abs() as usize
    };
    if stride_abs < row_bytes {
        return Err(VideoError::new(format!(
            "Media Foundation frame stride {stride_abs} is shorter than its row size {row_bytes}",
        )));
    }

    let plane_span = stride_abs.checked_mul(height.saturating_sub(1))
        .and_then(|value| value.checked_add(row_bytes))
        .ok_or_else(|| VideoError::new("video frame stride overflow"))?;
    if bytes.len() < plane_span {
        return Err(VideoError::new(format!(
            "Media Foundation frame is too small for its stride: got {} bytes, need {plane_span}",
            bytes.len(),
        )));
    }

    let pixel_count = width.checked_mul(height)
        .ok_or_else(|| VideoError::new("video pixel count overflow"))?;
    let mut pixels = Vec::with_capacity(pixel_count);

    match output_format {
        OutputFormat::Rgb32 | OutputFormat::Argb32 => {
            for y in 0..height {
                let source_y = if stride < 0 { height - 1 - y } else { y };
                let begin = source_y * stride_abs;
                for px in bytes[begin..begin + row_bytes].chunks_exact(4) {
                    // RGB32's top byte is *unused*, not an alpha channel.
                    // Keep real ARGB32 alpha unless it is zero (decoders often
                    // leave the alpha byte zero even for opaque video).
                    let alpha = if output_format == OutputFormat::Rgb32 || px[3] == 0 {
                        0xFF
                    } else {
                        px[3]
                    };
                    pixels.push(u32::from_le_bytes([px[0], px[1], px[2], alpha]));
                }
            }
        }
        OutputFormat::Nv12 => {
            if stride < 0 {
                return Err(VideoError::new("Media Foundation returned an invalid negative NV12 stride"));
            }
            // NV12: full-size Y plane followed by interleaved UV at half
            // horizontal and vertical resolution. Both planes share stride.
            let y_plane_len = stride_abs.checked_mul(height)
                .ok_or_else(|| VideoError::new("NV12 Y plane overflow"))?;
            let uv_height = height / 2 + height % 2;
            let uv_span = stride_abs.checked_mul(uv_height)
                .ok_or_else(|| VideoError::new("NV12 UV plane overflow"))?;
            let required = y_plane_len.checked_add(uv_span)
                .ok_or_else(|| VideoError::new("NV12 frame size overflow"))?;
            if bytes.len() < required {
                return Err(VideoError::new(format!(
                    "NV12 frame is too short: got {} bytes, need {required}", bytes.len(),
                )));
            }
            for y in 0..height {
                let y_offset = y * stride_abs;
                let uv_offset = y_plane_len + (y / 2) * stride_abs;
                for x in 0..width {
                    let uv = uv_offset + (x / 2) * 2;
                    pixels.push(yuv_to_argb(
                        bytes[y_offset + x], bytes[uv], bytes[uv + 1],
                    ));
                }
            }
        }
        OutputFormat::Yuy2 => {
            if width % 2 != 0 {
                return Err(VideoError::new("YUY2 frames must have an even width"));
            }
            for y in 0..height {
                let source_y = if stride < 0 { height - 1 - y } else { y };
                let begin = source_y * stride_abs;
                for chunk in bytes[begin..begin + row_bytes].chunks_exact(4) {
                    pixels.push(yuv_to_argb(chunk[0], chunk[1], chunk[3]));
                    pixels.push(yuv_to_argb(chunk[2], chunk[1], chunk[3]));
                }
            }
        }
    }
    Ok(pixels)
}

#[inline]
fn yuv_to_argb(y: u8, u: u8, v: u8) -> u32 {
    // BT.601 limited-range YUV: common fallback for native NV12/YUY2 frames.
    // RGB32 conversion is preferred because Media Foundation then applies
    // the stream's own colour metadata.
    let c = (i32::from(y) - 16).max(0);
    let d = i32::from(u) - 128;
    let e = i32::from(v) - 128;
    let r = ((298 * c + 409 * e + 128) >> 8).clamp(0, 255) as u32;
    let g = ((298 * c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u32;
    let b = ((298 * c + 516 * d + 128) >> 8).clamp(0, 255) as u32;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

#[inline]
fn hns_i64_to_duration(value: i64) -> Option<Duration> {
    if value < 0 {
        return None;
    }

    let value = value as u64;
    let seconds = value / HUNDRED_NANOSECONDS_PER_SECOND;
    let remaining_100ns = value % HUNDRED_NANOSECONDS_PER_SECOND;
    let nanoseconds = remaining_100ns * 100;

    Some(Duration::new(seconds, nanoseconds as u32))
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(core::iter::once(0)).collect()
}


#[cfg(test)]
mod tests {
    use super::{copy_frame_bytes, OutputFormat};

    #[test]
    fn rgb32_ignores_unused_alpha_byte() {
        assert_eq!(
            copy_frame_bytes(&[1, 2, 3, 42, 5, 6, 7, 0], 2, 1, 8, OutputFormat::Rgb32).unwrap(),
            vec![0xFF03_0201, 0xFF07_0605],
        );
    }

    #[test]
    fn argb32_preserves_nonzero_alpha() {
        assert_eq!(
            copy_frame_bytes(&[1, 2, 3, 128], 1, 1, 4, OutputFormat::Argb32).unwrap(),
            vec![0x8003_0201],
        );
    }

    #[test]
    fn packed_rgb_negative_stride_is_bottom_up() {
        let data = [10, 20, 30, 0, 1, 2, 3, 0];
        assert_eq!(
            copy_frame_bytes(&data, 1, 2, -4, OutputFormat::Rgb32).unwrap(),
            vec![0xFF03_0201, 0xFF1E_140A],
        );
    }

    #[test]
    fn nv12_black_and_white_frames() {
        let data = [16, 235, 16, 235, 128, 128];
        assert_eq!(
            copy_frame_bytes(&data, 2, 2, 2, OutputFormat::Nv12).unwrap(),
            vec![0xFF00_0000, 0xFFFF_FFFF, 0xFF00_0000, 0xFFFF_FFFF],
        );
    }

    #[test]
    fn yuy2_grayscale_pair() {
        let data = [16, 128, 235, 128];
        assert_eq!(
            copy_frame_bytes(&data, 2, 1, 4, OutputFormat::Yuy2).unwrap(),
            vec![0xFF00_0000, 0xFFFF_FFFF],
        );
    }

    #[test]
    fn short_nv12_frames_are_rejected() {
        assert!(copy_frame_bytes(&[0; 5], 2, 2, 2, OutputFormat::Nv12).is_err());
    }
}
