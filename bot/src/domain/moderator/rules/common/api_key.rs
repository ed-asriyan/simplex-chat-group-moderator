//! The API key of a condition that asks a remote AI provider with the owner's
//! own key, OpenAI's or OpenRouter's.

use crate::domain::moderator::ports::Err;

/// Maximum length (in characters) of an API key, OpenAI's or OpenRouter's.
/// Today's keys are under 200 characters; the cap only keeps a pasted
/// paragraph out of the database.
const MAX_API_KEY_LENGTH: usize = 256;

/// The key is trimmed, because it is pasted by hand; the errors never repeat
/// it, because they land in the owner's chat and in the logs. `provider` names
/// whose key it is, as the owner knows it.
pub fn normalize(api_key: &mut String, provider: &str, title: &str) -> Result<(), Err> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(format!("'{title}' needs an {provider} API key").into());
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(
            format!("The {provider} API key must not contain spaces or line breaks").into(),
        );
    }
    let length = trimmed.chars().count();
    if length > MAX_API_KEY_LENGTH {
        return Err(format!(
            "The {provider} API key is too long: {length} characters, maximum is {MAX_API_KEY_LENGTH}"
        )
        .into());
    }
    *api_key = trimmed.to_string();
    Ok(())
}
