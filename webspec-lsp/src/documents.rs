use dashmap::DashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone, Default)]
pub struct DocumentStore {
    inner: Arc<DashMap<Url, String>>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, uri: &Url, text: &str) {
        self.inner.insert(uri.clone(), text.to_string());
    }

    pub fn get(&self, uri: &Url) -> Option<String> {
        self.inner.get(uri).map(|e| e.clone())
    }

    pub fn remove(&self, uri: &Url) {
        self.inner.remove(uri);
    }
}
