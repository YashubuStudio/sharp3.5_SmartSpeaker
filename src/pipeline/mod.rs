mod sentence_splitter;
mod streaming;

pub use streaming::{process_command_streaming, PipelineClients};

/// ユーザー発話のインテント
pub enum Intent {
    /// ファクト保存（本文を含む）
    SaveFact(String),
    /// 通常の質問（既存フロー）
    Query,
}

/// ファクト保存を示すプレフィックス一覧
const SAVE_PREFIXES: &[&str] = &["覚えて", "メモして", "記録して", "保存して"];

/// ユーザー発話からインテントを判定する
///
/// プレフィックスマッチングによる高速・確実な判定（LLM不要）
pub fn detect_intent(text: &str) -> Intent {
    let trimmed = text.trim();
    for prefix in SAVE_PREFIXES {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let body = rest.trim_start_matches(|c| c == '、' || c == '。' || c == ' ' || c == '\u{3000}');
            if !body.is_empty() {
                return Intent::SaveFact(body.to_string());
            }
        }
    }
    Intent::Query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_intent_save_with_comma() {
        match detect_intent("覚えて、ドンクのパンはクロワッサンモードで3分") {
            Intent::SaveFact(fact) => {
                assert_eq!(fact, "ドンクのパンはクロワッサンモードで3分");
            }
            Intent::Query => panic!("SaveFactを期待"),
        }
    }

    #[test]
    fn test_detect_intent_save_with_space() {
        match detect_intent("メモして テスト情報") {
            Intent::SaveFact(fact) => {
                assert_eq!(fact, "テスト情報");
            }
            Intent::Query => panic!("SaveFactを期待"),
        }
    }

    #[test]
    fn test_detect_intent_save_with_fullwidth_space() {
        match detect_intent("記録して\u{3000}全角スペース区切り") {
            Intent::SaveFact(fact) => {
                assert_eq!(fact, "全角スペース区切り");
            }
            Intent::Query => panic!("SaveFactを期待"),
        }
    }

    #[test]
    fn test_detect_intent_save_no_delimiter() {
        match detect_intent("保存してこれを覚えておいて") {
            Intent::SaveFact(fact) => {
                assert_eq!(fact, "これを覚えておいて");
            }
            Intent::Query => panic!("SaveFactを期待"),
        }
    }

    #[test]
    fn test_detect_intent_save_empty_body() {
        // プレフィックスのみ（本文なし）→ Queryにフォールバック
        match detect_intent("覚えて") {
            Intent::Query => {}
            Intent::SaveFact(_) => panic!("Queryを期待"),
        }
    }

    #[test]
    fn test_detect_intent_save_only_delimiter() {
        // プレフィックス + 区切り文字のみ → Queryにフォールバック
        match detect_intent("覚えて、") {
            Intent::Query => {}
            Intent::SaveFact(_) => panic!("Queryを期待"),
        }
    }

    #[test]
    fn test_detect_intent_query() {
        match detect_intent("バルミューダでパンを焼くには？") {
            Intent::Query => {}
            Intent::SaveFact(_) => panic!("Queryを期待"),
        }
    }

    #[test]
    fn test_detect_intent_trimmed() {
        // 入力全体がtrimされるので末尾空白も除去される
        match detect_intent("  覚えて、前後に空白  ") {
            Intent::SaveFact(fact) => {
                assert_eq!(fact, "前後に空白");
            }
            Intent::Query => panic!("SaveFactを期待"),
        }
    }

    #[test]
    fn test_detect_intent_all_prefixes() {
        for prefix in SAVE_PREFIXES {
            let input = format!("{}、テスト", prefix);
            match detect_intent(&input) {
                Intent::SaveFact(fact) => {
                    assert_eq!(fact, "テスト", "prefix: {}", prefix);
                }
                Intent::Query => panic!("SaveFactを期待: prefix={}", prefix),
            }
        }
    }
}
