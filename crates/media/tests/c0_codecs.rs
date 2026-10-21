use deckcraft_media::c0_codecs;

#[test]
fn feature_keeps_the_original_decoders_by_default() {
    for codec in ["H.264", "HEVC", "AAC"] {
        assert_eq!(c0_codecs::enabled(codec), cfg!(feature = "patent-codecs"));
    }
    for codec in ["AV1", "VP9", "Opus", "MP3", "FLAC", "Vorbis", "PCM"] {
        assert!(c0_codecs::enabled(codec));
    }
}

#[cfg(not(feature = "patent-codecs"))]
#[test]
fn aac_is_refused_before_insertion_or_decode() {
    let bytes = std::sync::Arc::new(vec![0xff, 0xf1, 0x50, 0x80, 0x01, 0x3f, 0xfc, 0, 0]);
    let error = c0_codecs::check(&bytes).unwrap_err().to_string();
    assert!(error.contains("this format is not supported here"), "{error}");
    assert!(deckcraft_media::audio::decode(&bytes).unwrap_err().to_string().contains("not supported here"));
    assert!(deckcraft_media::VideoDecoder::open(bytes).err().unwrap().to_string().contains("not supported here"));
}

#[cfg(not(feature = "patent-codecs"))]
#[test]
fn restricted_mp4_mov_and_raw_aac_are_refused_without_panics() {
    for name in ["clip-h264.mp4", "clip-h264.mov", "clip-hevc.mp4", "tone.m4a", "tone.aac"] {
        let bytes = std::sync::Arc::new(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap());
        for result in [
            c0_codecs::check(&bytes),
            deckcraft_media::probe(&bytes).map(|_| ()),
            deckcraft_media::audio::decode(&bytes).map(|_| ()),
            deckcraft_media::VideoDecoder::open(bytes).map(|_| ()),
        ] {
            assert!(result.unwrap_err().to_string().contains("not supported here"), "{name}");
        }
    }
}

#[cfg(not(feature = "patent-codecs"))]
#[test]
fn muted_restricted_media_has_a_clear_player_error_instead_of_a_silent_blank() {
    use deckcraft_media::{Player, VoiceSpec};
    for name in ["clip-h264.mp4", "clip-hevc.mp4", "tone.m4a"] {
        let bytes = std::sync::Arc::new(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap());
        let mut player = Player::new(None);
        player.play(1, 1, &bytes, VoiceSpec { clip: Default::default(), looping: false, duration: 1.0, audio: false }, None);
        assert!(player.wait_loaded(1, std::time::Duration::from_secs(5)));
        assert!(player.status(1).unwrap().error.unwrap().contains("this format is not supported here"), "{name}");
    }
}

#[test]
fn supported_audio_and_video_still_decode_with_the_feature_off() {
    let fixture =
        |name| std::sync::Arc::new(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap());
    for name in ["tone.mp3", "tone.flac", "tone.ogg", "tone.opus"] {
        let pcm = deckcraft_media::audio::decode(&fixture(name)).unwrap();
        assert!(pcm.frames() > 0, "{name}");
    }
    for name in ["clip-av1.mkv", "clip-vp9.webm"] {
        let mut video = deckcraft_media::VideoDecoder::open(fixture(name)).unwrap();
        assert!(video.next_frame().unwrap().is_some(), "{name}");
    }
    // One mono PCM sample, constructed from the RIFF/WAVE specification.
    let mut wav = b"RIFF".to_vec();
    wav.extend(38u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(8000u32.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(2u32.to_le_bytes());
    wav.extend(0i16.to_le_bytes());
    let pcm = deckcraft_media::audio::decode(&std::sync::Arc::new(wav)).unwrap();
    assert_eq!(pcm.rate, 8000);
    assert_eq!(pcm.frames(), 1);
}

#[test]
fn silent_playback_uses_the_supplied_host_clock() {
    use deckcraft_media::{Player, VoiceSpec};
    let mut player = Player::new(None);
    let bytes = std::sync::Arc::new(Vec::new());
    player.play(1, 1, &bytes, VoiceSpec { clip: Default::default(), looping: false, duration: 2.0, audio: false }, None);
    player.tick(10.0);
    player.tick(10.2);
    assert!((player.status(1).unwrap().position - 0.2).abs() < 1e-6);
    player.pause(1);
    player.tick(10.4);
    assert!((player.status(1).unwrap().position - 0.2).abs() < 1e-6);
}
