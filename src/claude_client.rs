use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize)]
pub struct ClaudeRequest {
    pub sender: String,
    pub receiver: String,
    pub message_type: String,
    pub task_id: String,
    pub command: String,
    pub payload: serde_json::Value,
}

#[derive(Deserialize)]
pub struct ClaudeResponse {
    pub status: String,
    pub payload: serde_json::Value,
}

pub struct ClaudeClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl ClaudeClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let client = Client::new();
        ClaudeClient {
            client,
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
        }
    }

    pub fn send_request(&self, req: &ClaudeRequest) -> Result<ClaudeResponse, Box<dyn Error>> {
        let url = format!("{}/agent", self.base_url);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", self.api_key))?);

        let response = self.client.post(&url)
            .headers(headers)
            .json(req)
            .send()?
            .error_for_status()?;

        let claude_response = response.json::<ClaudeResponse>()?;
        Ok(claude_response)
    }
}