use std::fs::File;
use std::io::{Read, BufReader};
use crate::find_file::find_files;
use crate::documents::{Documents, Document};
use tower_lsp::lsp_types::Url;

pub trait Context {
    fn get_document(&self, url: &Url) -> Option<Document>;
    fn find_file(&self, name: &str) -> Result<Vec<Document>, String>;
}

pub struct RealContext<'docs> {
    pub documents: &'docs Documents
}

impl<'docs> Context for RealContext<'docs> {

    fn get_document(&self, url: &Url) -> Option<Document> {
        self.documents.get(url)
    }

    // TODO : Do not open files that are already in memory
    fn find_file(&self, name: &str) -> Result<Vec<Document>, String> {
        let mut res = Vec::new();
        for path in find_files(name).into_iter() {
            let (url, text) = open_file(&path)?;
            self.documents.set(url.clone(), text.clone()); // Save document in memory
            res.push(Document { url, text });
        }
        Ok(res)
    }
}

pub fn context_for<'docs>(documents: &'docs Documents) -> RealContext<'docs> {
    RealContext { documents }
}

fn open_file(path: &str) -> Result<(Url, String), String> {

    // Parse URL
    let url = Url::from_file_path(path)
        .map_err(|()| format!("Could not parse url : {}", path))?;

    // Read File
    let mut text = String::new();
    let file = File::open(path).map_err(|_| format!("Could not open file : {}", path))?;
    BufReader::new(file)
        .read_to_string(&mut text)
        .map_err(|_| String::from("Could not read file content"))?;

    Ok((url, text))
}
