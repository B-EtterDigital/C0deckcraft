//! Optional codec policy. Standalone builds enable every original decoder by default.

use crate::{Bytes, Result};

pub fn enabled(codec: &str) -> bool {
    cfg!(feature = "patent-codecs") || !matches!(codec, "H.264" | "HEVC" | "AAC" | "aac")
}

/// Refuse restricted streams before decode or insertion, including mixed-codec containers.
/// Demux only: calling the full probe here could recurse through duration measurement.
pub fn check(bytes: &Bytes) -> Result<()> {
    #[cfg(feature = "patent-codecs")]
    {
        let _ = bytes;
        Ok(())
    }
    #[cfg(not(feature = "patent-codecs"))]
    {
        use crate::{Container, MediaError};
        let refuse = |codec: &str| Err(MediaError::Unsupported(format!("{codec}: this format is not supported here")));
        match Container::sniff(bytes) {
            Container::Adts => return refuse("AAC"),
            Container::Mp4 => {
                if let Ok(file) = deckcraft_isobmff::open(bytes) {
                    for entry in file.tracks.iter().flat_map(|t| &t.entries) {
                        let codec = crate::video::codec_name(&entry.codec);
                        if !enabled(&codec) {
                            return refuse(&codec);
                        }
                    }
                }
            }
            Container::Matroska => {
                if let Ok(file) = deckcraft_matroska::open(bytes) {
                    for track in &file.tracks {
                        let codec = crate::video::mkv_codec_name(&track.codec);
                        if !enabled(&codec) {
                            return refuse(&codec);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
