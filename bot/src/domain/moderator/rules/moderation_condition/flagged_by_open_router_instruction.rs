//! `FlaggedByOpenRouterInstruction`: a model on OpenRouter, given the owner's
//! instruction, answers that the message is what the instruction describes.

use super::instruction_context;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::{ApiRetry, Err};
use crate::domain::moderator::rules::common::api_key::normalize as normalize_api_key;
use crate::domain::moderator::rules::common::api_retry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const TITLE: &str = "Flagged by OpenRouter Instruction";

/// The OpenRouter models `FlaggedByOpenRouterInstruction` may ask. Every one
/// takes `temperature` 0 and a strict JSON schema (OpenRouter's
/// `structured_outputs`) and answers without reasoning first, which is what
/// keeps the answer a fast, repeatable boolean. Mirrored by the `model`
/// field's `oneOf` in `rules-schema.json`.
pub const OPENROUTER_INSTRUCTION_MODELS: [&str; 10] = [
    "google/gemini-2.5-flash-lite",
    "mistralai/mistral-small-3.2-24b-instruct",
    "meta-llama/llama-3.3-70b-instruct",
    "meta-llama/llama-4-maverick",
    "openai/gpt-4.1-nano",
    "openai/gpt-4o-mini",
    "openai/gpt-4.1-mini",
    "anthropic/claude-haiku-4.5",
    "openai/gpt-4o",
    "openai/gpt-4.1",
];

/// Maximum length (in characters) of an instruction. It travels in the editor
/// link and in every request, so it is billed on every message it checks.
pub const MAX_OPENROUTER_INSTRUCTION_LENGTH: usize = 4000;

/// Maximum number of earlier messages sent along with the one judged. Each
/// is billed on every message checked, and is one more member's text handed
/// to a third party. Mirrored by the field's `maximum` in `rules-schema.json`.
pub const MAX_OPENROUTER_CONTEXT_MESSAGES: u32 = 10;

/// Maximum length (in characters) of a model's reason as the owner is shown
/// it. The schema asks for one short sentence; this holds a model that ignores
/// that to a notification's worth.
const MAX_MODEL_REASON_LENGTH: usize = 200;

/// A `model` on OpenRouter, asked with the owner's own OpenRouter
/// `api_key` and given the owner's `instruction`, answers that the message
/// is what the instruction describes. Sends the message text to OpenRouter
/// and the provider it routes to, together with up to `context_messages`
/// of the group's messages that came before it, for the model to read but
/// not judge; a message with no text is never sent and never matches.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlaggedByOpenRouterInstruction {
    pub api_key: String,
    pub model: String,
    pub instruction: String,
    #[serde(default)]
    pub context_messages: u32,
    #[serde(flatten)]
    pub retry: ApiRetry,
}

#[async_trait]
impl Condition for FlaggedByOpenRouterInstruction {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        normalize_api_key(&mut self.api_key, "OpenRouter", TITLE)?;
        api_retry::validate(&self.retry, TITLE)?;
        if self.context_messages > MAX_OPENROUTER_CONTEXT_MESSAGES {
            return Err(format!(
                "'{TITLE}' can send at most {MAX_OPENROUTER_CONTEXT_MESSAGES} earlier messages, got {}",
                self.context_messages
            )
            .into());
        }
        if !OPENROUTER_INSTRUCTION_MODELS.contains(&self.model.as_str()) {
            return Err(format!(
                "'{TITLE}' cannot use the model '{}'. Pick one of: {}",
                self.model,
                OPENROUTER_INSTRUCTION_MODELS.join(", ")
            )
            .into());
        }
        let trimmed = self.instruction.trim();
        if trimmed.is_empty() {
            return Err(format!("'{TITLE}' needs an instruction").into());
        }
        let length = trimmed.chars().count();
        if length > MAX_OPENROUTER_INSTRUCTION_LENGTH {
            return Err(format!(
                "The instruction is too long: {length} characters, maximum is {MAX_OPENROUTER_INSTRUCTION_LENGTH}"
            )
            .into());
        }
        self.instruction = trimmed.to_string();
        Ok(())
    }

    fn describe(&self) -> String {
        format!("flagged by OpenRouter instruction ({})", self.model)
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        let text = &ctx.group_message.text;
        if text.trim().is_empty() {
            return Ok(None);
        }
        let context = if self.context_messages == 0 {
            Vec::new()
        } else {
            match ctx
                .message_history
                .messages_before(
                    &ctx.group_message.group.id,
                    &ctx.group_message.message_id,
                    self.context_messages,
                    ctx.group_message.timestamp,
                )
                .await
            {
                Ok(earlier) => instruction_context::for_model(&earlier),
                // Asked without the context the owner configured, the
                // model could give a different answer; no verdict is safer
                // than a different one.
                Err(_) => return Ok(None),
            }
        };
        match ctx
            .openrouter
            .matches_instruction(
                &self.api_key,
                &self.model,
                &self.instruction,
                &ctx.group_message.author_name,
                text,
                &context,
                &self.retry,
            )
            .await
        {
            Ok(verdict) if verdict.matches => Ok(Some(openrouter_instruction_reason(
                &self.model,
                &verdict.reason,
            ))),
            Ok(_) | Err(_) => Ok(None),
        }
    }
}

/// The model's own reason, on one line and cut to size, after the model's name
/// so the owner knows whose judgement it is.
fn openrouter_instruction_reason(model: &str, reason: &str) -> String {
    let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if reason.is_empty() {
        return format!("{model} says it matches the instruction");
    }
    let mut shown: String = reason.chars().take(MAX_MODEL_REASON_LENGTH).collect();
    if reason.chars().count() > MAX_MODEL_REASON_LENGTH {
        shown.push('…');
    }
    format!("{model}: {shown}")
}
