use std::{
    path::Path,
    time::Duration,
};

use gstreamer as gst;
use gst::prelude::*;

use crate::{
    PixelFormat,
    VideoError,
    VideoFrame,
    VideoInfo,
    SeekMode,
};

const APP_SINK_NAME: &str = "mata_video_sink";

pub(crate) struct PlatformDecoder {
    pipeline: gst::Pipeline,
    sink: gst::Element,
    capsfilter: gst::Element,
    original_size: (u32, u32),
    info: VideoInfo,
    pending_frame: Option<VideoFrame>,
    ended: bool,
}

impl PlatformDecoder {
    pub(crate) fn open(path: &Path) -> Result<Self, VideoError> {
        gst::init().map_err(|error| {
            VideoError::new(format!("failed to initialize GStreamer: {error}"))
        })?;

        if !path.exists() {
            return Err(VideoError::new(format!(
                "video file does not exist: {}",
                path.display(),
            )));
        }

        let uri = gst::glib::filename_to_uri(path, None).map_err(|error| {
            VideoError::new(format!(
                "failed to convert video path to a file URI: {error}",
            ))
        })?;

        // Prefer available hardware decoders, retaining software fallback.
        // GStreamer uses factory ranks for decodebin autoplugging. This preference
        // applies process-wide; it never disables software decoder factories.
        if std::env::var_os("MATA_VIDEO_DISABLE_GPU").is_none() {
            prefer_hardware_decoder_factories();
        }

        // Keep the Linux dependency surface to the single `gstreamer` crate.
        // `appsink` is created by GStreamer itself and its action signals are
        // called dynamically, so the separate gstreamer-app Rust crate is not
        // required.
        let description = format!(
            "uridecodebin uri=\"{}\" name=mata_decode \
             mata_decode. ! queue ! videoconvert ! videoscale ! \
             capsfilter name=mata_resolution caps=video/x-raw,format=BGRA ! \
             appsink name={APP_SINK_NAME} sync=false max-buffers=2 drop=false",
            escape_pipeline_string(uri.as_str()),
        );

        let element = gst::parse::launch(&description).map_err(|error| {
            VideoError::new(format!("failed to build GStreamer pipeline: {error}"))
        })?;

        let pipeline = element.downcast::<gst::Pipeline>().map_err(|_| {
            VideoError::new("GStreamer did not create a pipeline")
        })?;

        let capsfilter = pipeline.by_name("mata_resolution").ok_or_else(|| {
            VideoError::new("GStreamer resolution capsfilter was not created")
        })?;
        let sink = pipeline.by_name(APP_SINK_NAME).ok_or_else(|| {
            VideoError::new("GStreamer appsink was not created")
        })?;

        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| {
                VideoError::new(format!(
                    "failed to preroll GStreamer pipeline: {error:?}",
                ))
            })?;

        let preroll: Option<gst::Sample> =
            sink.emit_by_name("pull-preroll", &[]);

        let preroll = preroll.ok_or_else(|| {
            pipeline.set_state(gst::State::Null).ok();
            VideoError::new("video contains no decodable video frames")
        })?;

        let mut info = info_from_sample(&preroll)?;
        info.duration = pipeline
            .query_duration::<gst::ClockTime>()
            .map(clock_time_to_duration);

        let pending_frame = Some(frame_from_sample(&preroll, &info)?);

        pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| {
                VideoError::new(format!(
                    "failed to start GStreamer decoder: {error:?}",
                ))
            })?;

        let original_size = (info.width, info.height);
        Ok(Self {
            pipeline,
            sink,
            capsfilter,
            original_size,
            info,
            pending_frame,
            ended: false,
        })
    }

    #[inline]
    pub(crate) fn info(&self) -> VideoInfo {
        self.info
    }

    pub(crate) fn seek(&mut self, position: Duration, mode: SeekMode) -> Result<(), VideoError> {
        let nanos = u64::try_from(position.as_nanos())
            .map_err(|_| VideoError::new("seek position exceeds GStreamer range"))?;
        // KEY_UNIT uses the demuxer's index to jump straight to a nearby
        // keyframe. ACCURATE additionally prerolls the local GOP to the
        // exact timestamp inside GStreamer; neither scans from time zero.
        let flags = match mode {
            SeekMode::Accurate => gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            SeekMode::Keyframe => gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT
                | gst::SeekFlags::SNAP_NEAREST,
        };
        self.pipeline.seek_simple(flags, gst::ClockTime::from_nseconds(nanos))
            .map_err(|error| VideoError::new(format!("GStreamer seek failed: {error}")))?;
        self.pending_frame = None;
        self.ended = false;
        Ok(())
    }

    pub(crate) fn set_output_resolution(&mut self, resolution: Option<(u32, u32)>) -> Result<(), VideoError> {
        let (width, height) = resolution.unwrap_or(self.original_size);
        let width = i32::try_from(width).map_err(|_| VideoError::new("output width too large"))?;
        let height = i32::try_from(height).map_err(|_| VideoError::new("output height too large"))?;
        // videoscale operates before appsink, so this saves CPU->Rust copies
        // and buffer memory. GStreamer can still decode at its native size.
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRA")
            .field("width", width)
            .field("height", height)
            .build();
        self.capsfilter.set_property("caps", caps);
        self.pending_frame = None;
        // Flush old-size samples; the public decoder will also guarantee the
        // final requested dimensions if a driver renegotiates asynchronously.
        let position = self.pipeline.query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        self.pipeline.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position)
            .map_err(|error| VideoError::new(format!("GStreamer resolution renegotiation failed: {error}")))?;
        self.ended = false;
        Ok(())
    }

    pub(crate) fn next_frame(&mut self) -> Result<Option<VideoFrame>, VideoError> {
        self.next_frame_at_or_after(Duration::ZERO).map(|(frame, _)| frame)
    }

    /// Skip earlier native samples before allocating/converting ARGB pixels.
    pub(crate) fn next_frame_at_or_after(&mut self, min_pts: Duration)
        -> Result<(Option<VideoFrame>, u64), VideoError>
    {
        let mut skipped = 0u64;
        if let Some(frame) = self.pending_frame.take() {
            if frame.timestamp >= min_pts {
                return Ok((Some(frame), skipped));
            }
            skipped += 1;
        }
        if self.ended { return Ok((None, skipped)); }

        loop {
            // The appsink signal yields a GstSample without copying its image.
            let sample: Option<gst::Sample> = self.sink.emit_by_name("pull-sample", &[]);
            match sample {
                Some(sample) => {
                    let pts = sample.buffer().and_then(|buf| buf.pts())
                        .map(clock_time_to_duration);
                    // Missing PTS: pass the frame through rather than risk
                    // discarding an unknown number of legitimate samples.
                    if pts.is_some_and(|timestamp| timestamp < min_pts) {
                        skipped = skipped.saturating_add(1);
                        continue;
                    }
                    let frame = frame_from_sample(&sample, &self.info)?;
                    self.info.width = frame.width;
                    self.info.height = frame.height;
                    return Ok((Some(frame), skipped));
                },
                None => {
                    self.ended = true;
                    return Ok((None, skipped));
                },
            }
        }
    }

}

impl Drop for PlatformDecoder {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn info_from_sample(sample: &gst::Sample) -> Result<VideoInfo, VideoError> {
    let caps = sample.caps().ok_or_else(|| {
        VideoError::new("decoded GStreamer sample has no caps")
    })?;

    let structure = caps.structure(0).ok_or_else(|| {
        VideoError::new("decoded GStreamer sample has empty caps")
    })?;

    let width = structure
        .get::<i32>("width")
        .map_err(|error| VideoError::new(format!("missing video width: {error}")))?;
    let height = structure
        .get::<i32>("height")
        .map_err(|error| VideoError::new(format!("missing video height: {error}")))?;

    if width <= 0 || height <= 0 {
        return Err(VideoError::new("GStreamer returned invalid frame dimensions"));
    }

    let (frame_rate_numerator, frame_rate_denominator) = structure
        .get::<gst::Fraction>("framerate")
        .map(|fraction| {
            let numerator = fraction.numer();
            let denominator = fraction.denom();

            if numerator > 0 && denominator > 0 {
                (numerator as u32, denominator as u32)
            } else {
                (0, 0)
            }
        })
        .unwrap_or((0, 0));

    Ok(VideoInfo {
        width: width as u32,
        height: height as u32,
        frame_rate_numerator,
        frame_rate_denominator,
        duration: None,
        pixel_format: PixelFormat::Argb32,
    })
}

fn frame_from_sample(
    sample: &gst::Sample,
    fallback_info: &VideoInfo,
) -> Result<VideoFrame, VideoError> {
    let caps = sample.caps();
    let structure = caps.as_ref().and_then(|caps| caps.structure(0));

    let width = structure
        .and_then(|structure| structure.get::<i32>("width").ok())
        .filter(|value| *value > 0)
        .map(|value| value as u32)
        .unwrap_or(fallback_info.width);

    let height = structure
        .and_then(|structure| structure.get::<i32>("height").ok())
        .filter(|value| *value > 0)
        .map(|value| value as u32)
        .unwrap_or(fallback_info.height);

    let buffer = sample.buffer().ok_or_else(|| {
        VideoError::new("decoded GStreamer sample has no buffer")
    })?;

    let timestamp = buffer
        .pts()
        .map(clock_time_to_duration)
        .unwrap_or(Duration::ZERO);
    let duration = buffer.duration().map(clock_time_to_duration);

    let map = buffer.map_readable().map_err(|error| {
        VideoError::new(format!("failed to map decoded GStreamer frame: {error}"))
    })?;

    let bytes = map.as_slice();
    let width_usize = width as usize;
    let height_usize = height as usize;
    let row_bytes = width_usize
        .checked_mul(4)
        .ok_or_else(|| VideoError::new("video row size overflow"))?;
    let required = row_bytes
        .checked_mul(height_usize)
        .ok_or_else(|| VideoError::new("video frame size overflow"))?;

    if bytes.len() < required {
        return Err(VideoError::new(format!(
            "decoded frame is too small: got {} bytes, expected at least {}",
            bytes.len(),
            required,
        )));
    }

    // BGRA is four-byte aligned, but GStreamer is allowed to provide a larger
    // stride. Infer it from the buffer size when possible and copy only the
    // visible width from each row.
    let inferred_stride = if height_usize != 0 && bytes.len() % height_usize == 0 {
        let stride = bytes.len() / height_usize;
        if stride >= row_bytes { stride } else { row_bytes }
    } else {
        row_bytes
    };

    let pixel_count = width_usize
        .checked_mul(height_usize)
        .ok_or_else(|| VideoError::new("video pixel count overflow"))?;
    let mut pixels = Vec::with_capacity(pixel_count);

    for y in 0..height_usize {
        let row_start = y
            .checked_mul(inferred_stride)
            .ok_or_else(|| VideoError::new("video row offset overflow"))?;
        let row_end = row_start
            .checked_add(row_bytes)
            .ok_or_else(|| VideoError::new("video row end overflow"))?;

        if row_end > bytes.len() {
            return Err(VideoError::new("decoded GStreamer frame has an invalid stride"));
        }

        for pixel in bytes[row_start..row_end].chunks_exact(4) {
            pixels.push(u32::from_le_bytes([
                pixel[0], pixel[1], pixel[2], pixel[3],
            ]));
        }
    }

    Ok(VideoFrame {
        width,
        height,
        pixels,
        timestamp,
        duration,
    })
}

#[inline]
fn clock_time_to_duration(time: gst::ClockTime) -> Duration {
    Duration::from_nanos(time.nseconds())
}

fn escape_pipeline_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Prefer hardware decoders when installed. Plugin ranks are process-global,
/// so this is idempotent and never alters existing software decoder ranks.
fn prefer_hardware_decoder_factories() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let hardware = gst::ElementFactoryType::DECODER
            | gst::ElementFactoryType::HARDWARE
            | gst::ElementFactoryType::MEDIA_VIDEO;
        for factory in gst::ElementFactory::factories_with_type(hardware, gst::Rank::NONE) {
            // Some distros intentionally rank problematic GPU decoders below
            // software. Only boost already-enabled plugins (not rank NONE).
            if factory.rank() > gst::Rank::NONE && factory.rank() < gst::Rank::PRIMARY + 1 {
                factory.set_rank(gst::Rank::PRIMARY + 1);
            }
        }
    });
}
