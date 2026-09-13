use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tower_lsp::lsp_types::Url;

#[derive(Debug)]
pub struct Documents {
    documents: Arc<RwLock<HashMap<Url, String>>>
}

pub struct Document {
    pub url: Url,
    pub text: String // TODO : Make it a reference to the internal state
}

impl Documents {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    pub fn get(&self, url: &Url) -> Option<Document> {
        self.documents.read().unwrap()
            .get(url)
            .map(|text| Document { url: url.clone(), text: text.to_string() })
    }

    pub fn set(&self, url: Url, text: String) {
        self.documents.write().unwrap().insert(url, text);
    }
}
