use super::*;

pub struct LlmClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.1,
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let completion: ChatCompletion = resp.json().await?;
        Ok(completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
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
