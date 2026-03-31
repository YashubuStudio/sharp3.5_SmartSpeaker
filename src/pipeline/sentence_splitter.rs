/// 日本語テキストの文分割器
///
/// LLMからのストリーミングトークンをバッファに蓄積し、
/// 文境界文字（。！？\n）を検出したら文を確定・送出する。
pub struct SentenceSplitter {
    buffer: String,
    max_buffer_len: usize,
}

impl SentenceSplitter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            max_buffer_len: 200,
        }
    }

    /// トークンを追加し、完成した文があれば返す
    pub fn push(&mut self, token: &str) -> Vec<String> {
        self.buffer.push_str(token);
        self.extract_sentences()
    }

    /// バッファに残っているテキストを最終文として送出
    pub fn flush(&mut self) -> Option<String> {
        let remaining = self.buffer.trim().to_string();
        self.buffer.clear();
        if remaining.is_empty() {
            None
        } else {
            Some(remaining)
        }
    }

    fn extract_sentences(&mut self) -> Vec<String> {
        let mut sentences = Vec::new();

        loop {
            let split_pos = self.find_sentence_boundary();
            match split_pos {
                Some(pos) => {
                    let sentence: String = self.buffer[..pos].trim().to_string();
                    self.buffer = self.buffer[pos..].to_string();
                    if !sentence.is_empty() {
                        sentences.push(sentence);
                    }
                }
                None => {
                    // バッファが長すぎる場合、、やスペースで強制分割
                    if self.buffer.chars().count() > self.max_buffer_len {
                        if let Some(forced) = self.force_split() {
                            if !forced.is_empty() {
                                sentences.push(forced);
                            }
                        }
                    }
                    break;
                }
            }
        }

        sentences
    }

    /// 文境界の位置（バイトオフセット）を探す
    /// 閉じ括弧は直前の文に含める
    fn find_sentence_boundary(&self) -> Option<usize> {
        let terminators = ['。', '！', '？', '!', '?'];
        let closing_brackets = ['」', '』', '）', ')'];

        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

        for (i, &(_byte_pos, ch)) in chars.iter().enumerate() {
            if terminators.contains(&ch) {
                // 終端文字の次の位置を計算
                let mut end_idx = i + 1;

                // 後続の閉じ括弧を文に含める
                while end_idx < chars.len() && closing_brackets.contains(&chars[end_idx].1) {
                    end_idx += 1;
                }

                // バイト位置を取得
                let end_byte = if end_idx < chars.len() {
                    chars[end_idx].0
                } else {
                    self.buffer.len()
                };

                return Some(end_byte);
            }

            if ch == '\n' {
                // 改行の次の位置
                let end_byte = if i + 1 < chars.len() {
                    chars[i + 1].0
                } else {
                    self.buffer.len()
                };
                return Some(end_byte);
            }
        }

        None
    }

    /// バッファが長すぎる場合に、やスペースで強制分割
    fn force_split(&mut self) -> Option<String> {
        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();
        let split_chars = ['、', ',', ' ', '　'];

        // 後ろから探して最後の区切り文字で分割
        for &(byte_pos, ch) in chars.iter().rev() {
            if split_chars.contains(&ch) {
                let next_byte = byte_pos + ch.len_utf8();
                let sentence = self.buffer[..next_byte].trim().to_string();
                self.buffer = self.buffer[next_byte..].to_string();
                return Some(sentence);
            }
        }

        // 区切り文字が見つからなければバッファ全体を送出
        let sentence = self.buffer.trim().to_string();
        self.buffer.clear();
        Some(sentence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_sentence() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("こんにちは。");
        assert_eq!(result, vec!["こんにちは。"]);
    }

    #[test]
    fn test_multiple_sentences() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("今日は良い天気です。明日も晴れるでしょう。");
        assert_eq!(result, vec!["今日は良い天気です。", "明日も晴れるでしょう。"]);
    }

    #[test]
    fn test_incremental_tokens() {
        let mut splitter = SentenceSplitter::new();
        assert!(splitter.push("こんに").is_empty());
        assert!(splitter.push("ちは").is_empty());
        let result = splitter.push("。");
        assert_eq!(result, vec!["こんにちは。"]);
    }

    #[test]
    fn test_closing_brackets_included() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("「こんにちは。」次の文。");
        assert_eq!(result, vec!["「こんにちは。」", "次の文。"]);
    }

    #[test]
    fn test_exclamation_and_question() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("本当ですか？はい！");
        assert_eq!(result, vec!["本当ですか？", "はい！"]);
    }

    #[test]
    fn test_newline_split() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("一行目\n二行目。");
        assert_eq!(result, vec!["一行目", "二行目。"]);
    }

    #[test]
    fn test_flush_remaining() {
        let mut splitter = SentenceSplitter::new();
        splitter.push("残りのテキスト");
        let flushed = splitter.flush();
        assert_eq!(flushed, Some("残りのテキスト".to_string()));
    }

    #[test]
    fn test_flush_empty() {
        let mut splitter = SentenceSplitter::new();
        let flushed = splitter.flush();
        assert_eq!(flushed, None);
    }

    #[test]
    fn test_force_split_on_comma() {
        let mut splitter = SentenceSplitter::new();
        // 200文字超えの長いテキスト（句点なし、読点あり）
        let long_text = "あ".repeat(100) + "、" + &"い".repeat(110);
        let result = splitter.push(&long_text);
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with('、'));
    }

    #[test]
    fn test_nested_brackets() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("『すごい！』と言った。");
        assert_eq!(result, vec!["『すごい！』", "と言った。"]);
    }

    #[test]
    fn test_half_width_punctuation() {
        let mut splitter = SentenceSplitter::new();
        let result = splitter.push("Really? Yes!");
        assert_eq!(result, vec!["Really?", "Yes!"]);
    }
}
