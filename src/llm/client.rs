use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct LlmClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
    cache: Mutex<HashMap<String, String>>,
    use_cache: bool,
}

impl LlmClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: reqwest::Client::new(),
            cache: Mutex::new(HashMap::new()),
            use_cache: true,
        }
    }

    pub fn with_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    fn hash_prompt(messages: &[ChatMessage], model: &str, base_url: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        base_url.hash(&mut hasher);
        model.hash(&mut hasher);
        for msg in messages {
            msg.role.hash(&mut hasher);
            msg.content.hash(&mut hasher);
        }
        format!("{:x}", hasher.finish())
    }

    fn cache_dir() -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/webspec_llm_cache")
    }

    fn load_disk_cache(key: &str) -> Option<String> {
        let path = Self::cache_dir().join(format!("{}.resp", key));
        if path.exists() {
            std::fs::read_to_string(&path).ok()
        } else {
            None
        }
    }

    fn save_disk_cache(key: &str, content: &str) {
        let dir = Self::cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.resp", key));
        let _ = std::fs::write(&path, content);
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        let key = Self::hash_prompt(&messages, &self.model, &self.base_url);

        if self.use_cache {
            if let Some(cached) = Self::load_disk_cache(&key) {
                eprintln!("  LLM cache hit (disk) for hash {}", &key[..12]);
                return Ok(cached);
            }

            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&key) {
                eprintln!("  LLM cache hit (memory) for hash {}", &key[..12]);
                return Ok(cached.clone());
            }
        }

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.0,
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let completion: ChatCompletion = resp.json().await?;
        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        if self.use_cache {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(key.clone(), content.clone());
            Self::save_disk_cache(&key, &content);
        }

        Ok(content)
    }

    pub async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let resp = self
            .client
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        let list: ModelList = resp.json().await?;
        Ok(list.data.into_iter().map(|m| m.id).collect())
    }
}
