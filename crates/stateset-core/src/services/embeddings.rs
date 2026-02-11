//! Embedding service for generating vector embeddings via OpenAI API

use crate::{CommerceError, Customer, InventoryItem, Order, Product, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Embedding service for generating vector embeddings
pub struct EmbeddingService {
    client: reqwest::blocking::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}

#[derive(Serialize)]
struct OpenAIEmbeddingRequest {
    model: String,
    input: Vec<String>,
    dimensions: Option<usize>,
}

#[derive(Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)]
    model: String,
    usage: EmbeddingUsage,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Deserialize)]
struct EmbeddingUsage {
    #[allow(dead_code)]
    prompt_tokens: u32,
    total_tokens: u32,
}

/// Result of an embedding operation
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    /// The generated embedding vector
    pub embedding: Vec<f32>,
    /// Hash of the input text (for cache invalidation)
    pub text_hash: String,
    /// Tokens used for this embedding
    pub tokens_used: u32,
}

impl EmbeddingService {
    fn build_client(
        timeout_secs: u64,
    ) -> std::result::Result<reqwest::blocking::Client, reqwest::Error> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
    }

    fn build_client_or_default(timeout_secs: u64) -> reqwest::blocking::Client {
        Self::build_client(timeout_secs).unwrap_or_else(|_| reqwest::blocking::Client::new())
    }

    /// Create a new embedding service with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            client: Self::build_client_or_default(DEFAULT_TIMEOUT_SECS),
            api_key,
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        }
    }

    /// Create a new embedding service with fallible client construction
    pub fn try_new(api_key: String) -> Result<Self> {
        let client = Self::build_client(DEFAULT_TIMEOUT_SECS).map_err(|e| {
            CommerceError::ExternalServiceError(format!("Failed to build HTTP client: {}", e))
        })?;
        Ok(Self {
            client,
            api_key,
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        })
    }

    /// Create with custom model and dimensions
    pub fn with_model(api_key: String, model: String, dimensions: usize) -> Self {
        Self {
            client: Self::build_client_or_default(DEFAULT_TIMEOUT_SECS),
            api_key,
            model,
            dimensions,
        }
    }

    /// Create with custom model and dimensions (fallible client construction)
    pub fn try_with_model(api_key: String, model: String, dimensions: usize) -> Result<Self> {
        let client = Self::build_client(DEFAULT_TIMEOUT_SECS).map_err(|e| {
            CommerceError::ExternalServiceError(format!("Failed to build HTTP client: {}", e))
        })?;
        Ok(Self {
            client,
            api_key,
            model,
            dimensions,
        })
    }

    /// Generate embeddings for a batch of texts
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<EmbeddingResult>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate hashes for all texts
        let hashes: Vec<String> = texts.iter().map(|t| Self::hash_text(t)).collect();

        let request = OpenAIEmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
            dimensions: Some(self.dimensions),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| {
                CommerceError::ExternalServiceError(format!("OpenAI API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(CommerceError::ExternalServiceError(format!(
                "OpenAI API error ({}): {}",
                status, body
            )));
        }

        let result: OpenAIEmbeddingResponse = response.json().map_err(|e| {
            CommerceError::ExternalServiceError(format!("Failed to parse OpenAI response: {}", e))
        })?;

        // Calculate tokens per embedding (approximate)
        let tokens_per = result.usage.total_tokens / texts.len() as u32;

        // Map results back in original input order using the response index
        let mut ordered: Vec<Option<EmbeddingResult>> = vec![None; texts.len()];
        for d in result.data {
            if d.index >= texts.len() {
                return Err(CommerceError::ExternalServiceError(format!(
                    "OpenAI response index out of bounds: {}",
                    d.index
                )));
            }
            ordered[d.index] = Some(EmbeddingResult {
                embedding: d.embedding,
                text_hash: hashes[d.index].clone(),
                tokens_used: tokens_per,
            });
        }

        ordered
            .into_iter()
            .map(|maybe| {
                maybe.ok_or_else(|| {
                    CommerceError::ExternalServiceError(
                        "OpenAI response missing embeddings".to_string(),
                    )
                })
            })
            .collect()
    }

    /// Generate embedding for a single text
    pub fn embed(&self, text: &str) -> Result<EmbeddingResult> {
        let results = self.embed_batch(&[text.to_string()])?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| CommerceError::ExternalServiceError("No embedding returned".to_string()))
    }

    /// Hash text for cache invalidation
    pub fn hash_text(text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate searchable text for a product
    pub fn product_text(product: &Product) -> String {
        format!("{} {} {}", product.name, product.description, product.slug)
    }

    /// Generate searchable text for a customer
    pub fn customer_text(customer: &Customer) -> String {
        format!(
            "{} {} {}",
            customer.first_name, customer.last_name, customer.email
        )
    }

    /// Generate searchable text for an order
    pub fn order_text(order: &Order) -> String {
        let mut text = format!("{} {}", order.order_number, order.status);
        if let Some(notes) = &order.notes {
            text.push(' ');
            text.push_str(notes);
        }
        text
    }

    /// Generate searchable text for an inventory item
    pub fn inventory_item_text(item: &InventoryItem) -> String {
        let mut text = format!("{} {}", item.sku, item.name);
        if let Some(desc) = &item.description {
            text.push(' ');
            text.push_str(desc);
        }
        text
    }

    /// Get the configured model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the configured dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_text() {
        let hash1 = EmbeddingService::hash_text("hello world");
        let hash2 = EmbeddingService::hash_text("hello world");
        let hash3 = EmbeddingService::hash_text("different text");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_product_text() {
        let product = Product {
            id: uuid::Uuid::new_v4(),
            name: "Test Product".to_string(),
            slug: "test-product".to_string(),
            description: "A great product".to_string(),
            status: crate::ProductStatus::Active,
            product_type: crate::ProductType::Simple,
            attributes: vec![],
            seo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let text = EmbeddingService::product_text(&product);
        assert!(text.contains("Test Product"));
        assert!(text.contains("A great product"));
        assert!(text.contains("test-product"));
    }
}
