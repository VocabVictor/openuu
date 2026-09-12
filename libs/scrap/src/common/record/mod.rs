use crate::CodecFormat;
use base::message_proto::{message, video_frame, EncodedVideoFrame, Message};
#[cfg(feature = "hwcodec")]
use hbb_common::anyhow::anyhow;
use hbb_common::{bail, chrono, log, ResultType};
#[cfg(feature = "hwcodec")]
use hwcodec::mux::{MuxContext, Muxer};
use std::{
    fs::{File, OpenOptions},
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::mpsc::Sender,
    time::Instant,
};
use webm::mux::{self, Segment, Track, VideoTrack, Writer};

const MIN_SECS: u64 = 1;

mod context;
pub use context::*;
mod recorder;

struct WebmRecorder {
    vt: VideoTrack,
    webm: Option<Segment<Writer<File>>>,
    ctx: RecorderContext,
    ctx2: RecorderContext2,
    key: bool,
    written: bool,
    start: Instant,
}

impl RecorderApi for WebmRecorder {
    fn new(ctx: RecorderContext, ctx2: RecorderContext2) -> ResultType<Self> {
        let out = match {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&ctx2.filename)
        } {
            Ok(file) => file,
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => File::create(&ctx2.filename)?,
            Err(e) => return Err(e.into()),
        };
        let mut webm = match mux::Segment::new(mux::Writer::new(out)) {
            Some(v) => v,
            None => bail!("Failed to create webm mux"),
        };
        let vt = webm.add_video_track(
            ctx2.width as _,
            ctx2.height as _,
            None,
            if ctx2.format == CodecFormat::VP9 {
                mux::VideoCodecId::VP9
            } else if ctx2.format == CodecFormat::VP8 {
                mux::VideoCodecId::VP8
            } else {
                mux::VideoCodecId::AV1
            },
        );
        if ctx2.format == CodecFormat::AV1 {
            // [129, 8, 12, 0] in 3.6.0, but zero works
            let codec_private = vec![0, 0, 0, 0];
            if !webm.set_codec_private(vt.track_number(), &codec_private) {
                bail!("Failed to set codec private");
            }
        }
        Ok(WebmRecorder {
            vt,
            webm: Some(webm),
            ctx,
            ctx2,
            key: false,
            written: false,
            start: Instant::now(),
        })
    }

    fn write_video(&mut self, frame: &EncodedVideoFrame) -> bool {
        if frame.key {
            self.key = true;
        }
        if self.key {
            let ok = self
                .vt
                .add_frame(&frame.data, frame.pts as u64 * 1_000_000, frame.key);
            if ok {
                self.written = true;
            }
            ok
        } else {
            false
        }
    }
}

impl Drop for WebmRecorder {
    fn drop(&mut self) {
        let _ = std::mem::replace(&mut self.webm, None).map_or(false, |webm| webm.finalize(None));
        let mut state = RecordState::WriteTail;
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            std::fs::remove_file(&self.ctx2.filename).ok();
            state = RecordState::RemoveFile;
        }
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
    }
}

#[cfg(feature = "hwcodec")]
struct HwRecorder {
    muxer: Option<Muxer>,
    ctx: RecorderContext,
    ctx2: RecorderContext2,
    written: bool,
    key: bool,
    start: Instant,
}

#[cfg(feature = "hwcodec")]
impl RecorderApi for HwRecorder {
    fn new(ctx: RecorderContext, ctx2: RecorderContext2) -> ResultType<Self> {
        let muxer = Muxer::new(MuxContext {
            filename: ctx2.filename.clone(),
            width: ctx2.width,
            height: ctx2.height,
            is265: ctx2.format == CodecFormat::H265,
            framerate: crate::hwcodec::DEFAULT_FPS as _,
        })
        .map_err(|_| anyhow!("Failed to create hardware muxer"))?;
        Ok(HwRecorder {
            muxer: Some(muxer),
            ctx,
            ctx2,
            written: false,
            key: false,
            start: Instant::now(),
        })
    }

    fn write_video(&mut self, frame: &EncodedVideoFrame) -> bool {
        if frame.key {
            self.key = true;
        }
        if self.key {
            let ok = self
                .muxer
                .as_mut()
                .map(|m| m.write_video(&frame.data, frame.key).is_ok())
                .unwrap_or_default();
            if ok {
                self.written = true;
            }
            ok
        } else {
            false
        }
    }
}

#[cfg(feature = "hwcodec")]
impl Drop for HwRecorder {
    fn drop(&mut self) {
        self.muxer.as_mut().map(|m| m.write_tail().ok());
        let mut state = RecordState::WriteTail;
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            // The process cannot access the file because it is being used by another process
            self.muxer = None;
            std::fs::remove_file(&self.ctx2.filename).ok();
            state = RecordState::RemoveFile;
        }
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_filename_component;

    #[test]
    fn sanitize_recording_filename_component() {
        assert_eq!(
            sanitize_filename_component("192.168.1.2:21118"),
            "192.168.1.2_21118"
        );
        assert_eq!(
            sanitize_filename_component("[2001:db8::1]:21118"),
            "[2001_db8__1]_21118"
        );
        assert_eq!(
            sanitize_filename_component("peer/name\\with?bad\nchars"),
            "peer_name_with_bad_chars"
        );
    }
}
