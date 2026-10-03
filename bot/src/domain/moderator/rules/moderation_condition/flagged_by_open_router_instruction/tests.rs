use super::*;

fn err_of(condition: &mut impl Condition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

fn instructed(api_key: &str, model: &str, instruction: &str) -> FlaggedByOpenRouterInstruction {
    FlaggedByOpenRouterInstruction {
        retry: Default::default(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        instruction: instruction.to_string(),
        context_messages: 0,
    }
}

#[test]
fn test_openrouter_instruction_is_accepted_with_key_and_instruction_trimmed() {
    let mut condition = instructed(
        " sk-proj-abc\n",
        "openai/gpt-4o-mini",
        "\n  Block crypto ads.  \n",
    );
    condition.normalize_and_validate().unwrap();
    assert_eq!(
        condition,
        instructed("sk-proj-abc", "openai/gpt-4o-mini", "Block crypto ads.")
    );
}

#[test]
fn test_openrouter_instruction_accepts_every_listed_model_and_nothing_else() {
    for model in OPENROUTER_INSTRUCTION_MODELS {
        let mut condition = instructed("sk-proj-abc", model, "Block ads.");
        condition.normalize_and_validate().unwrap();
    }
    for model in [
        "openai/gpt-5",
        "gpt-4o-mini",
        "OpenAI/gpt-4o-mini",
        " openai/gpt-4o-mini",
        "",
    ] {
        let err = err_of(&mut instructed("sk-proj-abc", model, "Block ads."));
        assert!(
            err.contains("cannot use the model") && err.contains("openai/gpt-4o-mini"),
            "{model:?}: unexpected error: {err}"
        );
    }
}

#[test]
fn test_api_retry_settings_are_capped() {
    use crate::domain::moderator::ports::ApiRetry;
    for (max_attempts, retry_delay_seconds) in [(1, 0), (5, 10), (3, 1)] {
        let mut condition = instructed("sk-proj-abc", "openai/gpt-4o-mini", "Block ads.");
        condition.retry = ApiRetry {
            max_attempts,
            retry_delay_seconds,
        };
        condition.normalize_and_validate().unwrap();
    }
    for (max_attempts, retry_delay_seconds, expected) in [
        (0, 1, "between 1 and 5 attempts"),
        (6, 1, "between 1 and 5 attempts"),
        (3, 11, "at most 10 seconds"),
    ] {
        let mut condition = instructed("sk-proj-abc", "openai/gpt-4o-mini", "Block ads.");
        condition.retry = ApiRetry {
            max_attempts,
            retry_delay_seconds,
        };
        let err = err_of(&mut condition);
        assert!(err.contains(expected), "{err}");
    }
}

#[test]
fn test_openrouter_instruction_needs_an_instruction() {
    for instruction in ["", "   ", "\n\t"] {
        let err = err_of(&mut instructed(
            "sk-proj-abc",
            "openai/gpt-4o-mini",
            instruction,
        ));
        assert!(
            err.contains("needs an instruction"),
            "{instruction:?}: {err}"
        );
    }
}

#[test]
fn test_openrouter_instruction_length_is_capped() {
    let longest = "я".repeat(MAX_OPENROUTER_INSTRUCTION_LENGTH);
    let mut condition = instructed("sk-proj-abc", "openai/gpt-4o-mini", &longest);
    condition.normalize_and_validate().unwrap();

    let too_long = "я".repeat(MAX_OPENROUTER_INSTRUCTION_LENGTH + 1);
    let err = err_of(&mut instructed(
        "sk-proj-abc",
        "openai/gpt-4o-mini",
        &too_long,
    ));
    assert!(
        err.contains("instruction is too long"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_openrouter_instruction_key_is_checked_like_every_api_key() {
    let err = err_of(&mut instructed("  ", "openai/gpt-4o-mini", "Block ads."));
    assert!(err.contains("API key"), "unexpected error: {err}");

    let err = err_of(&mut instructed(
        "sk-proj abc",
        "openai/gpt-4o-mini",
        "Block ads.",
    ));
    assert!(err.contains("API key") && !err.contains("sk-proj"), "{err}");
}

#[test]
fn test_openrouter_instruction_describes_itself_without_its_key() {
    assert_eq!(
        instructed("sk-proj-abc", "openai/gpt-4.1-mini", "Block ads.").describe(),
        "flagged by OpenRouter instruction (openai/gpt-4.1-mini)"
    );
}

#[test]
fn test_openrouter_instruction_sends_at_most_the_maximum_of_earlier_messages() {
    let with_context = |n| {
        let mut condition = instructed("sk-proj-abc", "openai/gpt-4o-mini", "Block ads.");
        condition.context_messages = n;
        condition
    };
    for n in [0, 1, MAX_OPENROUTER_CONTEXT_MESSAGES] {
        with_context(n).normalize_and_validate().unwrap();
    }
    let err = err_of(&mut with_context(MAX_OPENROUTER_CONTEXT_MESSAGES + 1));
    assert!(
        err.contains("at most 10 earlier messages"),
        "unexpected error: {err}"
    );
}
