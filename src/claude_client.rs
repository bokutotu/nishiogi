use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeRequest {
    pub sender: String,
    pub receiver: String,
    pub message_type: String,
    pub task_id: String,
    pub command: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeResponse {
    pub status: String,
    pub payload: serde_json::Value,
}

pub struct ClaudeClient {
    endpoint: String,
    api_key: String,
    client: Client,
}

impl ClaudeClient {
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        ClaudeClient {
            endpoint: endpoint.to_string(),
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    pub fn send_request(&self, request: &ClaudeRequest) -> Result<ClaudeResponse, Box<dyn Error>> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key)?,
        );

        let response = self.client
            .post(&self.endpoint)
            .headers(headers)
            .json(request)
            .send()?;

        let response: ClaudeResponse = response.json()?;
        Ok(response)
    }
}