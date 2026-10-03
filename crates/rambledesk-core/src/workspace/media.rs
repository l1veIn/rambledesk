//! Bounded local container identification, not transcoding or codec validation.
//! A browser may reject an unsupported codec despite a recognized container.
pub(crate) fn detect_playable_media_type(contents: &[u8], file_name: &str) -> Option<&'static str> {
    let extension = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "wav"
            if contents.len() >= 44
                && contents.starts_with(b"RIFF")
                && contents.get(8..12) == Some(b"WAVE") =>
        {
            Some("audio/wav")
        }
        "mp3" if mp3_frame(contents) => Some("audio/mpeg"),
        "m4a" if mp4(contents) => Some("audio/mp4"),
        "mp4" if mp4(contents) => Some("video/mp4"),
        "webm" if webm(contents) => Some("video/webm"),
        "weba" if webm(contents) => Some("audio/webm"),
        "ogg" | "opus" if ogg(contents, false) => Some("audio/ogg"),
        "ogv" if ogg(contents, true) => Some("video/ogg"),
        _ => None,
    }
}
fn mp4(bytes: &[u8]) -> bool {
    let Some(size) = bytes
        .get(..4)
        .and_then(|size| size.try_into().ok())
        .map(u32::from_be_bytes)
    else {
        return false;
    };
    bytes.len() >= 16
        && bytes.get(4..8) == Some(b"ftyp")
        && size >= 16
        && size as usize <= bytes.len()
}
fn webm(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x1a\x45\xdf\xa3")
        && bytes[..bytes.len().min(4096)]
            .windows(4)
            .any(|part| part == b"webm")
}
fn ogg(bytes: &[u8], video: bool) -> bool {
    if !bytes.starts_with(b"OggS\0") || bytes.len() < 27 {
        return false;
    }
    let header = 27 + bytes[26] as usize;
    let Some(payload) = bytes.get(header..) else {
        return false;
    };
    if video {
        payload.starts_with(b"\x80theora")
    } else {
        payload.starts_with(b"OpusHead") || payload.starts_with(b"\x01vorbis")
    }
}
fn mp3_frame(bytes: &[u8]) -> bool {
    let offset = if bytes.starts_with(b"ID3") {
        let Some(header) = bytes.get(..10) else {
            return false;
        };
        if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
            return false;
        }
        10 + header[6..10]
            .iter()
            .fold(0usize, |size, byte| (size << 7) | *byte as usize)
            + if header[3] == 4 && header[5] & 0x10 != 0 {
                10
            } else {
                0
            }
    } else {
        0
    };
    // ID3 metadata alone is not playable. Require a plausible first MPEG frame;
    // do not infer audio from a random sync-like pattern elsewhere in an upload.
    let Some(frame) = bytes.get(offset..offset.saturating_add(4)) else {
        return false;
    };
    frame[0] == 0xff
        && frame[1] & 0xe0 == 0xe0
        && frame[1] & 0x18 != 0x08
        && frame[1] & 0x06 != 0
        && !matches!(frame[2] >> 4, 0 | 15)
        && frame[2] & 0x0c != 0x0c
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifies_container_bytes_with_a_matching_extension_without_trusting_labels() {
        let mp4 = b"\0\0\0\x18ftypisom\0\0\0\0isommp42";
        assert_eq!(
            detect_playable_media_type(mp4, "sample.mp4"),
            Some("video/mp4")
        );
        assert_eq!(
            detect_playable_media_type(mp4, "sample.m4a"),
            Some("audio/mp4")
        );
        assert_eq!(detect_playable_media_type(mp4, "sample.wav"), None);
        assert_eq!(
            detect_playable_media_type(b"not a movie", "sample.mp4"),
            None
        );
        assert_eq!(
            detect_playable_media_type(b"\xff\xfb\x90\x64xxxx", "clip.MP3"),
            Some("audio/mpeg")
        );
        assert_eq!(
            detect_playable_media_type(b"ID3\0\0\0\0\0\0\0", "clip.mp3"),
            None
        );
        assert_eq!(
            detect_playable_media_type(b"\x1a\x45\xdf\xa3xxxxwebmxxxx", "clip.webm"),
            Some("video/webm")
        );
        assert_eq!(
            detect_playable_media_type(b"\x1a\x45\xdf\xa3xxxxwebmxxxx", "clip.weba"),
            Some("audio/webm")
        );
        assert_eq!(
            detect_playable_media_type(b"\x1a\x45\xdf\xa3", "clip.webm"),
            None
        );
        let mut ogg = b"OggS\0".to_vec();
        ogg.resize(27, 0);
        ogg.extend_from_slice(b"OpusHeadxxxx");
        assert_eq!(
            detect_playable_media_type(&ogg, "clip.opus"),
            Some("audio/ogg")
        );
        assert_eq!(detect_playable_media_type(&ogg, "clip.ogv"), None);
        for extension in ["wav", "mp4", "mp3", "webm", "ogg"] {
            assert_eq!(
                detect_playable_media_type(&[], &format!("empty.{extension}")),
                None
            );
        }
    }
}
