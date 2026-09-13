use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::fs::File;
use std::io::{Read, BufReader};
use tower_lsp::lsp_types::Url;
use crate::context::find_file::find_files;

pub trait Context {

    // Find a document by its URL.
    // Open & save it in memory if needed.
    fn get_document(&self, url: &Url) -> Result<Document, String>;

    // Find a document by its file name.
    // Open & save it in memory if needed.
    fn find_file(&self, name: &str) -> Result<Vec<Document>, String>;
}

pub struct Document {
    pub url: Url,
    pub text: String // TODO : Make it a reference to the internal state
}

#[derive(Debug)]
pub struct Documents {
    documents: Arc<RwLock<HashMap<Url, String>>>
}

impl Documents {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    // Listen document change events to keep updated documents in memory
    pub fn on_document_change(&self, url: Url, text: String) {
        self.documents.write().unwrap().insert(url, text);
    }
}

impl Context for Documents {
    fn get_document(&self, url: &Url) -> Result<Document, String> {
        let mut map = self.documents.write().unwrap();

        let text = match map.get(url) {
            // Return document from memory if it is present
            Some(existing) => existing.to_string(),
            None => {
                // Open & save it otherwise
                let text = open(url)?;
                map.insert(url.clone(), text.to_string());
                text
            }
        };

        Ok(Document { url: url.clone(), text })
    }

    fn find_file(&self, name: &str) -> Result<Vec<Document>, String> {
        let mut res = Vec::new();
        for path in find_files(name).into_iter() {
            let url = url_from(&path)?;
            let doc = self.get_document(&url)?;
            res.push(doc);
        }
        Ok(res)
    }
}

fn url_from(path: &str) -> Result<Url, String> {
    Url::from_file_path(path)
        .map_err(|()| format!("Could not parse url : {}", path))
}

fn open(url: &Url) -> Result<String, String> {
    let path = format!("{}", url.to_file_path()
        .map_err(|()| format!("Failed to parse path from URL : {}", url))?
        .display());

    let file = File::open(&path)
        .map_err(|_| format!("Could not open file : {}", &path))?;

    let mut text = String::new();
    BufReader::new(file)
        .read_to_string(&mut text)
        .map_err(|_| format!("Could not read content of : {}", &path))?;

    Ok(text)
}
