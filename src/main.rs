mod documents;
mod context;
mod find_file;
mod features;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use documents::Documents;
use context::context_for;
use features::definition::goto_definition;

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
        match goto_definition(params, &context_for(&self.documents)) {
            Ok(res) => Ok(res),
            Err(err) => {
                self.client.log_message(MessageType::ERROR, &err.message).await;
                Err(err)
            }
        }
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
