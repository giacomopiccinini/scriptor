//! Integration tests for STT audio module.

mod common;

use scriptor::stt::audio::{
    convert_to_mono, convert_to_stereo, read_audio, read_audio_file_mono, resample, write_mono_wav,
};

#[test]
fn test_full_audio_pipeline_with_generated_wav() {
    let (_temp_dir, wav_path) = common::create_test_wav().unwrap();
    let (samples, spec) = read_audio(&wav_path).unwrap();
    assert_eq!(spec.sample_rate, 16000);
    assert_eq!(spec.channels, 1);
    assert_eq!(samples.len(), 16000);
}

#[test]
fn test_read_resample_write_roundtrip() {
    let (_temp_dir, wav_path) = common::create_test_wav().unwrap();
    let (samples, _spec) = read_audio(&wav_path).unwrap();

    let resampled = resample(samples, 16000, 8000).unwrap();
    assert_eq!(resampled.len(), 8000);

    let temp_dir = tempfile::tempdir().unwrap();
    let out_path = temp_dir.path().join("resampled.wav");
    write_mono_wav(resampled, 8000, 16, &out_path).unwrap();

    let (read_back, _) = read_audio_file_mono(&out_path).unwrap();
    assert_eq!(read_back.len(), 8000);
}

#[test]
fn test_stereo_wav_mono_conversion() {
    let (_temp_dir, wav_path) = common::create_test_stereo_wav().unwrap();
    let (samples, sr) = read_audio_file_mono(&wav_path).unwrap();
    assert_eq!(sr, 16000);
    assert_eq!(samples.len(), 1600);
}

#[test]
fn test_convert_mono_stereo_roundtrip() {
    let mono: Vec<f32> = vec![0.5, 0.3, -0.2];
    let stereo = convert_to_stereo(mono.clone(), 1);
    assert_eq!(stereo.len(), 6);
    let back_to_mono = convert_to_mono(stereo, 2);
    assert_eq!(back_to_mono.len(), 3);
    for (a, b) in mono.iter().zip(back_to_mono.iter()) {
        assert!((a - b).abs() < 1e-5);
    }
}
