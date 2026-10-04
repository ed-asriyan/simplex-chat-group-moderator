use async_trait::async_trait;
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::domain::moderator::rules::KeyUse;

use crate::domain::moderator::ports::{
    Err, Group, GroupAdministration, GroupConfig, GroupId, GroupMode, GroupModerator, KeyCheck,
    MessengerGroup, MessengerGroupId, ModerationRepository, ModerationRule, OpenAi, OpenRouter,
    UserId,
};

#[cfg(test)]
mod tests;

pub struct GroupAdministrationApplication {
    repository: Arc<dyn ModerationRepository>,
    group_moderator: Arc<dyn GroupModerator>,
    /// Asks OpenAI about every OpenAI key a saved rule set carries, before it
    /// is stored.
    openai: Arc<dyn OpenAi>,
    /// The same for every OpenRouter key and the model it is to ask.
    openrouter: Arc<dyn OpenRouter>,
}

impl GroupAdministrationApplication {
    pub fn new(
        repository: Arc<dyn ModerationRepository>,
        group_moderator: Arc<dyn GroupModerator>,
        openai: Arc<dyn OpenAi>,
        openrouter: Arc<dyn OpenRouter>,
    ) -> Self {
        Self {
            repository,
            group_moderator,
            openai,
            openrouter,
        }
    }

    async fn check_ownership(&self, user_id: UserId, group_id: GroupId) -> Result<(), Err> {
        match self.repository.get_owner_by_id(&group_id).await? {
            None => Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                Err(format!("User {} is not the owner of group {}", user_id, group_id).into())
            }
            Some(_) => Ok(()),
        }
    }

    /// Ask the provider about every distinct key in `rules`, wherever in a
    /// tree it sits: OpenAI for the moderation endpoint, OpenRouter for each
    /// model a key is to ask. The first check that does not pass stops the
    /// save.
    async fn verify_api_keys(&self, rules: &[ModerationRule]) -> Result<(), Err> {
        let checks: BTreeSet<KeyUse> = rules
            .iter()
            .flat_map(|rule| rule.condition.key_uses())
            .collect();
        for key_use in &checks {
            let check = match key_use {
                KeyUse::OpenAiModeration { api_key } => self.openai.verify(api_key).await,
                KeyUse::OpenRouterModel { api_key, model } => {
                    self.openrouter.verify_model(api_key, model).await
                }
            };
            if check != KeyCheck::Valid {
                return Err(key_check_error(key_use, check).into());
            }
        }
        Ok(())
    }
}

/// What to tell the owner about a key the provider did not accept. The key is
/// named by its last characters only: the message goes to the chat and the
/// logs.
fn key_check_error(key_use: &KeyUse, check: KeyCheck) -> String {
    let key = match key_use {
        KeyUse::OpenAiModeration { api_key } | KeyUse::OpenRouterModel { api_key, .. } => api_key,
    };
    let tail: String = {
        let mut tail: Vec<char> = key.chars().rev().take(4).collect();
        tail.reverse();
        tail.into_iter().collect()
    };
    let key = format!("…{tail}");
    match key_use {
        KeyUse::OpenAiModeration { .. } => openai_key_check_error(&key, check),
        KeyUse::OpenRouterModel { model, .. } => openrouter_key_check_error(&key, model, check),
    }
}

fn openai_key_check_error(key: &str, check: KeyCheck) -> String {
    match check {
        KeyCheck::Valid => format!("The OpenAI API key {key} works"),
        KeyCheck::Rejected => format!(
            "OpenAI rejected the API key {key}. Check that it was copied whole and has not been revoked."
        ),
        KeyCheck::Forbidden => format!(
            "The OpenAI API key {key} may not use Moderations. Allow Moderations in the key's permissions on the OpenAI platform."
        ),
        KeyCheck::QuotaExceeded => format!(
            "OpenAI refused the API key {key}: the account's quota is exhausted or its billing is not set up."
        ),
        KeyCheck::ModelUnavailable => format!(
            "The OpenAI API key {key} cannot use the moderation model. Check that the key's project allows it."
        ),
        KeyCheck::Unreachable => format!(
            "Could not reach OpenAI to check the API key {key}. Send the link again in a minute."
        ),
    }
}

fn openrouter_key_check_error(key: &str, model: &str, check: KeyCheck) -> String {
    match check {
        KeyCheck::Valid => format!("The OpenRouter API key {key} works with {model}"),
        KeyCheck::Rejected => format!(
            "OpenRouter rejected the API key {key}. Check that it was copied whole and has not been deleted or disabled at openrouter.ai/settings/keys."
        ),
        KeyCheck::Forbidden => format!(
            "The OpenRouter API key {key} may not use {model}. Check the key's guardrail at openrouter.ai/settings/keys."
        ),
        KeyCheck::QuotaExceeded => format!(
            "OpenRouter refused the API key {key}: the account has no credits left, or the key has spent its own credit limit. Add credits at openrouter.ai/settings/credits."
        ),
        KeyCheck::ModelUnavailable => format!(
            "The OpenRouter API key {key} cannot have {model} answer: no provider of that model currently takes a strict JSON answer without collecting data. Pick another model."
        ),
        KeyCheck::Unreachable => format!(
            "Could not reach OpenRouter to check the API key {key}. Send the link again in a minute."
        ),
    }
}

#[async_trait]
impl GroupAdministration for GroupAdministrationApplication {
    async fn try_join_group(&self, owner_id: UserId, group: &MessengerGroup) -> Result<Group, Err> {
        let messenger_group_id = group.id;
        let existing_owner = self
            .repository
            .get_owner_by_messenger_id(&messenger_group_id)
            .await?;
        if existing_owner.is_some() {
            return Err(format!("Group {} is already registered", messenger_group_id).into());
        }
        self.group_moderator.join_group(group.id).await?;
        // A group the owner has not configured yet notifies them of every
        // action, so nothing the bot does goes unseen.
        let mode = GroupMode::default();
        let group_id = self
            .repository
            .save_owner(&messenger_group_id, &group.name, &owner_id, mode)
            .await?;
        Ok(Group {
            id: group_id,
            owner_id,
            name: group.name.clone(),
            mode,
        })
    }

    async fn remove_group(&self, messenger_group_id: MessengerGroupId) -> Result<(), Err> {
        self.repository.delete_group_data(&messenger_group_id).await
    }

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err> {
        self.repository.get_groups_by_owner_id(owner_id).await
    }

    async fn get_group_config(
        &self,
        user_id: UserId,
        group_id: GroupId,
    ) -> Result<GroupConfig, Err> {
        self.check_ownership(user_id, group_id).await?;
        self.repository.get_group_config(&group_id).await
    }

    async fn set_group_config(
        &self,
        user_id: UserId,
        group_id: GroupId,
        config: GroupConfig,
    ) -> Result<(), Err> {
        self.check_ownership(user_id, group_id).await?;

        let mut config = config;
        for rule in &mut config.rules {
            rule.normalize_and_validate()?;
        }
        self.verify_api_keys(&config.rules).await?;

        self.repository.set_group_config(&group_id, &config).await
    }
}
