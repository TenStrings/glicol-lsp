use ropey::RopeSlice;
use std::borrow::Cow;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent};

const HIGHLIGHT_NAMES: &[&str; 5] = &["function", "number", "operator", "comment", "string"];

pub const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::NUMBER,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
];

pub struct Highlighter {
    highlighter: tree_sitter_highlight::Highlighter,
    configuration: HighlightConfiguration,
}

impl Highlighter {
    pub fn new() -> Self {
        let (highlighter, configuration) = {
            let highlighter = tree_sitter_highlight::Highlighter::new();

            let mut configuration = HighlightConfiguration::new(
                tree_sitter_glicol::language(),
                tree_sitter_glicol::HIGHLIGHTS_QUERY,
                tree_sitter_glicol::INJECTIONS_QUERY,
                "",
            )
            .unwrap();

            configuration.configure(HIGHLIGHT_NAMES);

            (highlighter, configuration)
        };

        Self {
            highlighter,
            configuration,
        }
    }

    pub fn semantic_tokens(&mut self, rope: RopeSlice) -> Vec<SemanticToken> {
        let source: Cow<str> = rope.into();

        let mut tokens: Vec<SemanticToken> = vec![];

        let highlights = self.highlighter.highlight(
            &self.configuration,
            source.as_ref().as_bytes(),
            None,
            |_| None,
        );

        let highlights = match highlights {
            Ok(highlights) => highlights,
            Err(_error) => return vec![],
        };

        let mut skip = true;

        for event in highlights {
            let event = match event {
                Ok(event) => event,
                Err(_error) => break,
            };

            match event {
                HighlightEvent::Source { start, end } => {
                    if skip {
                        skip = true;
                        continue;
                    }

                    let start_line = rope.char_to_line(start);

                    let start_line_char = rope.line_to_char(start_line);

                    let semantic_token = tokens.last_mut().unwrap();

                    semantic_token.delta_line = start_line as u32;
                    semantic_token.delta_start = start as u32 - start_line_char as u32;
                    semantic_token.length = end as u32 - start as u32;
                }
                HighlightEvent::HighlightStart(s) => {
                    skip = false;
                    tokens.push(SemanticToken {
                        delta_line: 0,
                        delta_start: 0,
                        length: 0,
                        token_type: s.0 as u32,
                        token_modifiers_bitset: 0,
                    });
                }
                HighlightEvent::HighlightEnd => {
                    skip = true;
                }
            }
        }

        for i in (1..tokens.len()).into_iter().rev() {
            if tokens[i].delta_line == tokens[i - 1].delta_line {
                tokens[i].delta_start -= tokens[i - 1].delta_start;
            }

            tokens[i].delta_line -= tokens[i - 1].delta_line;
        }

        tokens
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_token::Highlighter;
    use ropey::Rope;

    #[test]
    fn test_highlight() {
        let line = r#"
~t1: seq 60 60 60
~t2: seq 60 60 60
"#;

        let rope = Rope::from_str(line);

        let tokens = Highlighter::new().semantic_tokens(rope.byte_slice(..));

        assert_eq!(
            tokens
                .into_iter()
                .map(|token| token.token_type)
                .collect::<Vec<_>>(),
            vec![0, 1, 1, 1, 0, 1, 1, 1]
        );
    }
}
