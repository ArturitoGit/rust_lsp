mod documents;
mod goto_method;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Point;

use goto_method::go_to_method;
use documents::Documents;

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: Documents
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                definition_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            }
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    
    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let document = self.documents.get(&uri)
            .expect("Failed to find the document in saved documents");

        // Use tree sitter to find the definition
        let result = go_to_method(&document, to_point(&position));
        let Some(point) = result else {
            self.client
                .log_message(MessageType::INFO, "Could not find definition :(")
                .await;
            return Ok(None);
        };

        // Build response
        let response = GotoDefinitionResponse::Scalar(Location {
            uri: uri,
            range: Range {
                start: to_position(&point),
                end: to_position(&point)
            }
        });

        Ok(Some(response))
    }
    
    async fn did_open(&self, params: DidOpenTextDocumentParams) -> () {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        // Save the document in the memory
        self.documents.set(uri, text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) -> () {
        let uri = params.text_document.uri;
        let changes = params.content_changes;

        if changes.is_empty() {
            return;
        }
        
        // Save the document in the memory
        let first_change = changes.into_iter().next().unwrap();
        self.documents.set(uri, first_change.text);
    }
}

fn to_point(position: &Position) -> Point {
    Point::new(position.line as usize, position.character as usize)
}

fn to_position(point: &Point) -> Position {
    Position::new(point.row as u32, point.column as u32)
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Documents::new()
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
