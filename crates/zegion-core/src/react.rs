use aisdk::core::{LanguageModel, LanguageModelRequest, Message, Tool};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// The ReAct (Reason + Act) executor: lets the model interleave reasoning and tool
/// calls until it produces a final answer or hits the step cap. Built on aisdk's
/// native tool-calling loop.
pub struct ReactExecutor {
    pub max_steps: usize,
}

impl ReactExecutor {
    pub fn new(max_steps: usize) -> Self {
        Self { max_steps: max_steps.max(1) }
    }

    pub async fn execute<M>(
        &self,
        model: M,
        system: String,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<String>
    where
        M: LanguageModel
            + aisdk::core::capabilities::TextInputSupport
            + aisdk::core::capabilities::ToolCallSupport,
    {
        let mut builder = LanguageModelRequest::builder()
            .model(model)
            .system(system)
            .messages(messages);
        for t in tools {
            builder = builder.with_tool(t);
        }
        let mut req = builder
            .stop_when(aisdk::core::utils::step_count_is(self.max_steps))
            .build();
        let response = req
            .generate_text()
            .await
            .map_err(|e| Error::Provider(e.to_string()))?;
        Ok(response.text().unwrap_or_else(|| "(no response)".into()))
    }
}

/// Structured-output executor: generate text conforming to a JSON schema and
/// deserialize it into a typed value.
pub struct StructuredExecutor;

impl StructuredExecutor {
    pub async fn execute<M, T>(
        model: M,
        system: String,
        prompt: String,
    ) -> Result<T>
    where
        M: LanguageModel
            + aisdk::core::capabilities::StructuredOutputSupport
            + aisdk::core::capabilities::TextInputSupport,
        T: DeserializeOwned + schemars::JsonSchema,
    {
        let mut req = LanguageModelRequest::builder()
            .model(model)
            .system(system)
            .prompt(prompt)
            .schema::<T>()
            .build();
        let response = req
            .generate_text()
            .await
            .map_err(|e| Error::Provider(e.to_string()))?;
        response
            .into_schema::<T>()
            .map_err(|e| Error::Provider(format!("structured parse failed: {e}")))
    }
}
