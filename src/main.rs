pub mod goto_definition;
pub mod helpers;
pub mod hover;
pub mod semantic_token;

use dashmap::DashMap;
use glicol::EngineError;
use goto_definition::goto_definition;
use pest::error::LineColLocation;
use ropey::{Rope, RopeSlice};
use semantic_token::{Highlighter, LEGEND_TYPE};
use std::borrow::Cow;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Parser, Point, Tree};

struct Backend {
    client: Client,
    parser: Mutex<Parser>,
    documents: DashMap<Url, (Tree, Rope, Mutex<Highlighter>)>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("glicol".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: LEGEND_TYPE.into(),
                                    token_modifiers: vec![],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "glicol lsp server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut parser = self.parser.lock().await;

        let new_tree = parser.parse(&params.text_document.text, None);

        if let Some(new_tree) = new_tree {
            self.documents.insert(
                params.text_document.uri,
                (
                    new_tree,
                    Rope::from_str(&params.text_document.text),
                    Mutex::new(Default::default()),
                ),
            );
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut parser = self.parser.lock().await;
        let data = self.documents.get_mut(&params.text_document.uri);

        if let Some(mut data) = data {
            let (tree, rope, _) = data.value_mut();

            for change in params.content_changes {
                if let Some(range) = change.range {
                    let mut start = rope.line_to_char(range.start.line as usize);

                    start += range.start.character as usize;

                    let mut end = rope.line_to_char(range.end.line as usize);

                    end += range.end.character as usize;

                    let old_end_byte = rope.char_to_byte(end);
                    let new_end_char = start + change.text.len();
                    let new_end_byte = rope.char_to_byte(new_end_char);

                    rope.remove(start..end);

                    rope.insert(start, &change.text);

                    let new_end_line = rope.char_to_line(start + change.text.len());

                    tree.edit(&InputEdit {
                        start_byte: rope.char_to_byte(start),
                        old_end_byte,
                        new_end_byte,
                        start_position: Point {
                            row: range.start.line as usize,
                            column: range.start.character as usize,
                        },
                        old_end_position: Point {
                            row: range.end.line as usize,
                            column: range.end.character as usize,
                        },
                        new_end_position: Point {
                            row: new_end_line,
                            column: new_end_char - rope.line_to_char(new_end_line),
                        },
                    });
                }

                let new_tree = parser.parse(format!("{}", rope), Some(tree));

                *tree = new_tree.unwrap();

                self.client
                    .log_message(MessageType::INFO, change.text)
                    .await;
            }

            self.diagnostics(
                &params.text_document.uri,
                rope.byte_slice(..),
                params.text_document.version,
            )
            .await;
        }
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {}

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let data = self
            .documents
            .get(&params.text_document_position_params.text_document.uri);

        let data = if let Some(data) = data {
            data
        } else {
            return Ok(None);
        };

        let (tree, rope, _) = data.value();

        goto_definition(
            tree,
            rope.byte_slice(..),
            params.text_document_position_params.position.line as usize,
            params.text_document_position_params.position.character as usize,
        )
        .map(|byte_range| {
            let byte_to_position = |byte| {
                let start_line = rope.byte_to_line(byte);
                let start_line_char = rope.line_to_char(start_line);
                let start_char = rope.byte_to_char(byte) - start_line_char;

                Position {
                    line: start_line as u32,
                    character: start_char as u32,
                }
            };

            GotoDefinitionResponse::Scalar(Location {
                uri: params.text_document_position_params.text_document.uri,
                range: Range {
                    start: byte_to_position(byte_range.start),
                    end: byte_to_position(byte_range.end),
                },
            })
        })
        .map(Ok)
        .transpose()
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let data = self
            .documents
            .get(&params.text_document_position_params.text_document.uri);

        let data = if let Some(data) = data {
            data
        } else {
            return Ok(None);
        };

        let (tree, rope, _) = data.value();

        Ok(hover::hover(
            tree,
            rope.byte_slice(..),
            params.text_document_position_params.position.line as usize,
            params.text_document_position_params.position.character as usize,
        )
        .map(|raw| Hover {
            contents: HoverContents::Scalar(MarkedString::String(raw)),
            range: None,
        }))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let data = self.documents.get(&params.text_document.uri);

        let data = if let Some(data) = data {
            data
        } else {
            return Ok(None);
        };

        let (_tree, rope, highlighter) = data.value();

        let data = highlighter
            .lock()
            .await
            .semantic_tokens(rope.byte_slice(..));

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let data = self.documents.get(&params.text_document.uri);

        let data = if let Some(data) = data {
            data
        } else {
            return Ok(None);
        };

        let (_tree, rope, highlighter) = data.value();

        let data = highlighter
            .lock()
            .await
            .semantic_tokens(rope.byte_slice(..));

        Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}

const BLOCK_SIZE: usize = 128;

impl Backend {
    async fn diagnostics<'a>(&self, uri: &Url, rope_slice: RopeSlice<'a>, version: i32) {
        let parse_result = match std::panic::catch_unwind(|| {
            // TODO: keep the Engine around
            let mut engine = glicol::Engine::<BLOCK_SIZE>::new();

            let code: Cow<str> = rope_slice.into();
            engine.update_with_code(&code);
            engine.parse()
        }) {
            Ok(no_panic) => no_panic,
            Err(_panic) => {
                self.client
                    .publish_diagnostics(uri.clone(), vec![], Some(version))
                    .await;

                return;
            }
        };

        if let Err(error) = parse_result {
            match error {
                EngineError::ParsingError(error) => {
                    let (start, end) = match &error.line_col {
                        LineColLocation::Pos((line, col)) => {
                            let pos = Position {
                                line: *line as u32 - 1,
                                character: *col as u32 - 1,
                            };
                            (pos, pos)
                        }
                        LineColLocation::Span((line1, col1), (line2, col2)) => (
                            Position {
                                line: *line1 as u32 - 1,
                                character: *col1 as u32 - 1,
                            },
                            Position {
                                line: *line2 as u32 - 1,
                                character: *col2 as u32 - 1,
                            },
                        ),
                    };

                    self.client
                        .publish_diagnostics(
                            uri.clone(),
                            vec![Diagnostic {
                                range: Range { start, end },
                                severity: Some(DiagnosticSeverity::ERROR),
                                code: Some(NumberOrString::String(error.line().to_string())),
                                code_description: None,
                                source: Some("glicol engine".to_string()),
                                message: error.variant.message().to_string(),
                                related_information: None,
                                tags: None,
                                data: None,
                            }],
                            Some(version),
                        )
                        .await;
                }
                EngineError::NonExistReference(_) => log::error!("unimplemented"),
                EngineError::NonExsitSample(_) => log::error!("unimplemented"),
            }
        } else {
            self.client
                .publish_diagnostics(uri.clone(), vec![], Some(version))
                .await;
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let mut parser = Parser::new();

    parser
        .set_language(tree_sitter_glicol::language())
        .expect("Error loading grammar");

    let (service, socket) = LspService::new(|client| Backend {
        client,
        parser: Mutex::new(parser),
        documents: DashMap::new(),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
