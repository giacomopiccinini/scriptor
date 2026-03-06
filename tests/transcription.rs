//! Integration tests for STT transcription module.

use scriptor::stt::transcription::{TimestampGranularity, TimestampedResult, convert_timestamps};

#[test]
fn test_convert_timestamps_full_pipeline() {
    let result = TimestampedResult {
        text: "Hello world. How are you?".to_string(),
        tokens: vec![
            "▁Hello".to_string(),
            "▁world".to_string(),
            ".".to_string(),
            " ".to_string(),
            "▁How".to_string(),
            "▁are".to_string(),
            "▁you".to_string(),
            "?".to_string(),
        ],
        timestamps: vec![0.0, 0.2, 0.4, 0.5, 0.6, 0.8, 1.0, 1.2],
    };

    let token_segments = convert_timestamps(&result, TimestampGranularity::Token);
    assert_eq!(token_segments.len(), 8);

    let word_segments = convert_timestamps(&result, TimestampGranularity::Word);
    assert!(word_segments.len() >= 2);

    let segment_segments = convert_timestamps(&result, TimestampGranularity::Segment);
    assert_eq!(segment_segments.len(), 2);
    assert_eq!(segment_segments[0].text, "Hello world.");
    assert_eq!(segment_segments[1].text, "How are you?");
}
