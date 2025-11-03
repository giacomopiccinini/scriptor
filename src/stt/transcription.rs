use super::model::SegmentTranscription;

/// Granularity level for timestamp generation
#[derive(Debug, Clone, Default, PartialEq)]
pub enum TimestampGranularity {
    /// Token-level timestamps (most detailed)
    #[default]
    Token,
    /// Word-level timestamps
    Word,
    /// Segment-level timestamps (sentences/phrases)
    Segment,
}

/// Raw result from model with token-level timestamps
#[derive(Debug, Clone)]
pub struct TimestampedResult {
    pub text: String,
    pub timestamps: Vec<f32>,
    pub tokens: Vec<String>,
}

/// Token with timing information
#[derive(Debug, Clone, PartialEq)]
struct Token {
    text: String,
    t_start: f32,
    t_end: f32,
}

/// Word composed of multiple tokens
#[derive(Debug, Clone, PartialEq)]
struct Word {
    text: String,
    t_start: f32,
    t_end: f32,
    tokens: Vec<Token>,
}

/// Segment composed of multiple words
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    text: String,
    t_start: f32,
    t_end: f32,
    words: Vec<Word>,
}

/// Convert token-level timestamps to requested granularity
pub fn convert_timestamps(
    result: &TimestampedResult,
    granularity: TimestampGranularity,
) -> Vec<SegmentTranscription> {
    match granularity {
        TimestampGranularity::Token => convert_to_token_segments(result),
        TimestampGranularity::Word => convert_to_word_segments(result),
        TimestampGranularity::Segment => convert_to_sentence_segments(result),
    }
}

/// Convert to raw token-level segments
fn convert_to_token_segments(result: &TimestampedResult) -> Vec<SegmentTranscription> {
    let mut segments = Vec::new();

    for (i, (token, &timestamp)) in result
        .tokens
        .iter()
        .zip(result.timestamps.iter())
        .enumerate()
    {
        let end_timestamp = result
            .timestamps
            .get(i + 1)
            .copied()
            .unwrap_or(timestamp + 0.05);

        segments.push(SegmentTranscription {
            start: timestamp,
            end: end_timestamp,
            text: token.clone(),
        });
    }

    segments
}

/// Convert to word-level segments
fn convert_to_word_segments(result: &TimestampedResult) -> Vec<SegmentTranscription> {
    let tokens = create_tokens(result);
    let words = group_into_words(&tokens);

    words
        .into_iter()
        .filter(|w| !w.text.trim().is_empty())
        .map(|word| SegmentTranscription {
            start: word.t_start,
            end: word.t_end,
            text: word.text,
        })
        .collect()
}

/// Convert to sentence-level segments
fn convert_to_sentence_segments(result: &TimestampedResult) -> Vec<SegmentTranscription> {
    let tokens = create_tokens(result);
    let words = group_into_words(&tokens);
    let segments = group_into_segments(&words);

    segments
        .into_iter()
        .filter(|s| !s.text.trim().is_empty())
        .map(|segment| SegmentTranscription {
            start: segment.t_start,
            end: segment.t_end,
            text: segment.text,
        })
        .collect()
}

/// Create token structs from timestamped result
fn create_tokens(result: &TimestampedResult) -> Vec<Token> {
    let mut tokens = Vec::new();

    for (i, (token_text, &timestamp)) in result
        .tokens
        .iter()
        .zip(result.timestamps.iter())
        .enumerate()
    {
        let t_end = result
            .timestamps
            .get(i + 1)
            .copied()
            .unwrap_or(timestamp + 0.05);

        tokens.push(Token {
            text: token_text.clone(),
            t_start: timestamp,
            t_end,
        });
    }

    tokens
}

/// Group tokens into words
fn group_into_words(tokens: &[Token]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current_word_tokens = Vec::new();

    for token in tokens {
        if token.text.trim().is_empty() {
            continue;
        }

        // Detect word boundaries (space prefix or SentencePiece marker)
        let starts_new_word = token.text.starts_with(' ')
            || token.text.starts_with('▁')
            || current_word_tokens.is_empty();

        if starts_new_word && !current_word_tokens.is_empty() {
            words.push(create_word(&current_word_tokens));
            current_word_tokens.clear();
        }

        current_word_tokens.push(token.clone());
    }

    if !current_word_tokens.is_empty() {
        words.push(create_word(&current_word_tokens));
    }

    words
}

/// Create word from tokens
fn create_word(tokens: &[Token]) -> Word {
    if tokens.is_empty() {
        return Word {
            text: String::new(),
            t_start: 0.0,
            t_end: 0.0,
            tokens: Vec::new(),
        };
    }

    let t_start = tokens.first().unwrap().t_start;
    let t_end = tokens.last().unwrap().t_end;

    // Combine tokens, removing word boundary markers
    let text = tokens
        .iter()
        .map(|t| {
            t.text
                .strip_prefix('▁')
                .or_else(|| t.text.strip_prefix(' '))
                .unwrap_or(&t.text)
        })
        .collect::<String>()
        .trim()
        .to_string();

    Word {
        text,
        t_start,
        t_end,
        tokens: tokens.to_vec(),
    }
}

/// Group words into segments based on punctuation
fn group_into_segments(words: &[Word]) -> Vec<Segment> {
    if words.is_empty() {
        return Vec::new();
    }

    let segment_separators = ['.', '?', '!'];
    let mut segments = Vec::new();
    let mut current_segment_words = Vec::new();

    for (i, word) in words.iter().enumerate() {
        current_segment_words.push(word.clone());

        let ends_segment =
            word.text.chars().any(|c| segment_separators.contains(&c)) || i == words.len() - 1;

        if ends_segment {
            segments.push(create_segment(&current_segment_words));
            current_segment_words.clear();
        }
    }

    // If no punctuation found, create one segment
    if segments.is_empty() && !words.is_empty() {
        segments.push(create_segment(words));
    }

    segments
}

/// Create segment from words
fn create_segment(words: &[Word]) -> Segment {
    if words.is_empty() {
        return Segment {
            text: String::new(),
            t_start: 0.0,
            t_end: 0.0,
            words: Vec::new(),
        };
    }

    let t_start = words.first().unwrap().t_start;
    let t_end = words.last().unwrap().t_end;

    let text = words
        .iter()
        .map(|w| w.text.as_str())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    Segment {
        text,
        t_start,
        t_end,
        words: words.to_vec(),
    }
}
