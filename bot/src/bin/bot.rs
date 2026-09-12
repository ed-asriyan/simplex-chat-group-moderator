use bot::domain::bot_dm::BotDmApplication;
use bot::domain::bot_dm::ports::{
    BotDmReceiver, BotMessenger, GroupOperations, Message, ModerationNotificationReceiver,
};
use bot::domain::moderator::ports::{
    GroupAdministration, GroupMessage, GroupModerator, MemberRestoreRepository,
    MemberRestoreRunner, MessengerGroup, ModerationEngine, ModerationNotifier,
    ModerationRepository, UserActivityRepository, UserModerationActivityRepository,
};
use bot::domain::moderator::{
    GroupAdministrationApplication, MemberRestoreApplication, MessageModerationApplication,
};
use bot::infrastructure::adapters::cross_domain_router::CrossDomainRouter;
use bot::infrastructure::adapters::member_restore_repo_sqlite::SqliteMemberRestoreRepository;
use bot::infrastructure::adapters::moderation_notification_router::ModerationNotificationRouter;
use bot::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository;
use bot::infrastructure::adapters::simplex_adapter::SimplexAdapter;
use bot::infrastructure::adapters::user_activity_repo_in_memory::InMemoryUserActivityRepository;
use bot::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;
use bot::infrastructure::drivers::simplex::{SimpleXConfig, SimplexDriver, SimplexEvent};
use bot::infrastructure::migrations;
use chrono::{Local, Utc};
use clap::{Arg, Command};
use env_logger::Builder;
use futures::StreamExt;
use log::{LevelFilter, info};
use rusqlite::Connection;
use std::error::Error;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::interval;

/// How often the bot looks for members whose observer time is up. Restores are
/// coarse by nature — a minute of slack is not worth a tighter loop.
const MEMBER_RESTORE_TICK: Duration = Duration::from_secs(60);

async fn handle_event(
    event: SimplexEvent,
    dm_receiver: Arc<dyn BotDmReceiver>,
    moderator: Arc<dyn ModerationEngine>,
    group_administration: Arc<dyn GroupAdministration>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        SimplexEvent::Message {
            user_id,
            text,
            reply_message_text,
            ..
        } => {
            dm_receiver
                .handle_dm(
                    user_id,
                    &Message {
                        text,
                        reply_to_message: reply_message_text,
                    },
                )
                .await?;
        }
        SimplexEvent::GroupMessage {
            group_id,
            group_name,
            author_id,
            message_id,
            timestamp,
            text,
            author_joined_at,
        } => {
            let group_message = GroupMessage {
                group: MessengerGroup {
                    id: group_id,
                    name: group_name,
                },
                message_id,
                author_id,
                text,
                timestamp,
                author_joined_at,
            };
            moderator.process_group_message(group_message).await?;
        }
        SimplexEvent::Connected { user_id } => {
            dm_receiver
                .handle_dm(
                    user_id,
                    &Message {
                        text: "/start".to_string(),
                        reply_to_message: None,
                    },
                )
                .await?;
        }
        SimplexEvent::Disconnected { user_id } => {
            info!("user disconnected: {}", user_id);
        }
        SimplexEvent::GroupInvitation {
            user_id,
            group_id,
            group_name,
            is_moderator,
        } => {
            let invitation = bot::domain::bot_dm::ports::GroupInvitation {
                group: bot::domain::bot_dm::ports::Group {
                    id: group_id,
                    name: group_name,
                    notifications_enabled: true,
                    dry_mode_enabled: false,
                },
                is_moderator,
            };
            dm_receiver
                .handle_group_invitation(user_id, &invitation)
                .await?;
        }
        SimplexEvent::RemovedFromGroup { group_id } => {
            group_administration.remove_group(group_id).await?;
        }
    }

    Ok(())
}

fn init_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_logger();

    let args = Command::new("simplex-group-moderator-bot")
        .author("Ed Asriyan")
        .arg(
            Arg::new("simplex-uri")
                .long("simplex-uri")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("display-name")
                .long("display-name")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("full-name")
                .long("full-name")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("short-description")
                .long("short-description")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("db-path")
                .long("db-path")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("webeditor-base-url")
                .long("webeditor-base-url")
                .required(true)
                .num_args(1),
        )
        .get_matches();

    let simplex_uri = args
        .get_one::<String>("simplex-uri")
        .ok_or("missing --simplex-uri")?;
    let display_name = args
        .get_one::<String>("display-name")
        .ok_or("missing --display-name")?;
    let full_name = args
        .get_one::<String>("full-name")
        .ok_or("missing --full-name")?;
    let short_description = args
        .get_one::<String>("short-description")
        .ok_or("missing --short-description")?;
    let db_path = args
        .get_one::<String>("db-path")
        .ok_or("missing --db-path")?;
    let webeditor_base_url = args
        .get_one::<String>("webeditor-base-url")
        .ok_or("missing --webeditor-base-url")?
        .clone();

    // ---- drivers ----
    let conn = Arc::new(Mutex::new(Connection::open(db_path)?));
    migrations::run(conn.clone())
        .await
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
    let simplex_config = SimpleXConfig {
        simplex_uri: simplex_uri.clone(),
        display_name: display_name.clone(),
        full_name: full_name.clone(),
        short_description: short_description.clone(),
    };
    let (simplex_driver, simplex_stream) = SimplexDriver::new(simplex_config).await?;
    let simplex_driver = Arc::new(simplex_driver);
    println!(
        "Bot address: {}",
        simplex_driver.get_or_create_bot_address().await?
    );

    // ---- moderator outbound adapters ----
    let moderation_repo = SqliteModerationRepository::new(conn.clone());
    let moderation_repo: Arc<dyn ModerationRepository> = Arc::new(moderation_repo);
    let user_activity_repo: Arc<dyn UserActivityRepository> =
        Arc::new(InMemoryUserActivityRepository::new());
    let user_moderation_activity_repo: Arc<dyn UserModerationActivityRepository> =
        Arc::new(InMemoryUserModerationActivityRepository::new());
    let member_restore_repo: Arc<dyn MemberRestoreRepository> =
        Arc::new(SqliteMemberRestoreRepository::new(conn.clone()));

    let simplex_adapter = Arc::new(SimplexAdapter::new(simplex_driver.clone()));
    let bot_messenger: Arc<dyn BotMessenger> = simplex_adapter.clone();
    let group_moderator: Arc<dyn GroupModerator> = simplex_adapter.clone();

    // ---- notification router (moderator -> bot_dm), receiver wired below ----
    let notification_router = Arc::new(ModerationNotificationRouter::new());
    let moderation_notifier: Arc<dyn ModerationNotifier> = notification_router.clone();

    // ---- moderator bounded context (inbound ports) ----
    let moderator_engine: Arc<dyn ModerationEngine> = Arc::new(MessageModerationApplication::new(
        moderation_repo.clone(),
        group_moderator.clone(),
        moderation_notifier,
        user_activity_repo,
        user_moderation_activity_repo,
        member_restore_repo.clone(),
    ));
    let group_administration: Arc<dyn GroupAdministration> = Arc::new(
        GroupAdministrationApplication::new(moderation_repo, group_moderator.clone()),
    );
    let member_restore_runner: Arc<dyn MemberRestoreRunner> = Arc::new(
        MemberRestoreApplication::new(member_restore_repo, group_moderator),
    );

    // ---- cross-domain adapter (bot_dm::GroupOperations -> group administration) ----
    let group_operations: Arc<dyn GroupOperations> =
        Arc::new(CrossDomainRouter::new(group_administration.clone()));

    // ---- bot_dm bounded context ----
    let bot_dm_app = Arc::new(BotDmApplication::new(
        bot_messenger,
        group_operations,
        webeditor_base_url,
    ));
    let bot_dm_app: Arc<dyn BotDmReceiver> = {
        let notification_receiver: Arc<dyn ModerationNotificationReceiver> = bot_dm_app.clone();
        notification_router.set_receiver(notification_receiver);
        bot_dm_app
    };

    // ---- driver event loop ----
    let dm_receiver = bot_dm_app.clone();
    let moderator = moderator_engine.clone();
    let groups = group_administration.clone();
    let polling_task = tokio::spawn(async move {
        let mut stream = Box::pin(simplex_stream);
        while let Some(event) = stream.next().await {
            if let Err(err) = handle_event(
                event,
                dm_receiver.clone(),
                moderator.clone(),
                groups.clone(),
            )
            .await
            {
                eprintln!("Error handling event: {:#?}", err);
            }
        }
    });

    // ---- restores whose time has come ----
    let restore_task = tokio::spawn(async move {
        let mut ticks = interval(MEMBER_RESTORE_TICK);
        loop {
            ticks.tick().await;
            if let Err(err) = member_restore_runner.run_due_restores(Utc::now()).await {
                eprintln!("Error restoring members: {:#?}", err);
            }
        }
    });

    info!("Bot initialized.");

    tokio::signal::ctrl_c().await?;
    polling_task.abort();
    restore_task.abort();
    info!("Shutdown signal received.");

    Ok(())
}
