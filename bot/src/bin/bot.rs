use bot::domain::bot_dm::BotDmApplication;
use bot::domain::bot_dm::ports::{BotDmReceiver, BotMessenger, GroupOperations, Message};
use bot::domain::moderator::ModeratorApplication;
use bot::domain::moderator::ports::{
    GroupMessage, GroupModerator, MessengerGroup, ModerationEngine, ModerationRepository,
};
use bot::infrastructure::adapters::cross_domain_router::CrossDomainRouter;
use bot::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository;
use bot::infrastructure::adapters::simplex_adapter::SimplexAdapter;
use bot::infrastructure::drivers::simplex::{SimpleXConfig, SimplexDriver, SimplexEvent};
use bot::infrastructure::migrations;
use chrono::Local;
use clap::{Arg, Command};
use env_logger::Builder;
use futures::StreamExt;
use log::{LevelFilter, info};
use rusqlite::Connection;
use std::error::Error;
use std::io::Write;
use std::sync::{Arc, Mutex};

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

    let simplex_adapter = Arc::new(SimplexAdapter::new(simplex_driver.clone()));
    let bot_messenger: Arc<dyn BotMessenger> = simplex_adapter.clone();
    let group_moderator: Arc<dyn GroupModerator> = simplex_adapter.clone();

    // ---- moderator bounded context (inbound port) ----
    let moderator_app = Arc::new(ModeratorApplication::new(moderation_repo, group_moderator));
    let moderator_engine: Arc<dyn ModerationEngine> = moderator_app.clone();

    // ---- cross-domain adapter (bot_dm::GroupOperations -> moderator engine) ----
    let group_operations: Arc<dyn GroupOperations> =
        Arc::new(CrossDomainRouter::new(moderator_engine.clone()));

    // ---- bot_dm bounded context ----
    let bot_dm_app: Arc<dyn BotDmReceiver> =
        Arc::new(BotDmApplication::new(bot_messenger, group_operations));

    // ---- driver event loop ----
    let dm_receiver = bot_dm_app.clone();
    let moderator = moderator_engine.clone();
    let polling_task = tokio::spawn(async move {
        let mut stream = Box::pin(simplex_stream);
        while let Some(event) = stream.next().await {
            match event {
                SimplexEvent::Message {
                    user_id,
                    text,
                    reply_message_text,
                    ..
                } => {
                    let _ = dm_receiver
                        .handle_dm(
                            user_id,
                            &Message {
                                text,
                                reply_to_message: reply_message_text,
                            },
                        )
                        .await;
                }
                SimplexEvent::GroupMessage {
                    group_id,
                    group_name,
                    author_id,
                    message_id,
                    text,
                } => {
                    let group_message = GroupMessage {
                        group: MessengerGroup {
                            id: group_id,
                            name: group_name,
                        },
                        message_id,
                        author_id,
                        text,
                    };
                    let _ = moderator.process_group_message(group_message).await;
                }
                SimplexEvent::Connected { user_id } => {
                    let _ = dm_receiver
                        .handle_dm(
                            user_id,
                            &Message {
                                text: "/start".to_string(),
                                reply_to_message: None,
                            },
                        )
                        .await;
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
                        },
                        is_moderator,
                    };
                    let _ = dm_receiver
                        .handle_group_invitation(user_id, &invitation)
                        .await;
                }
                SimplexEvent::RemovedFromGroup { group_id } => {
                    let _ = moderator.remove_group(group_id).await;
                }
            }
        }
    });

    info!("Bot initialized.");

    tokio::signal::ctrl_c().await?;
    polling_task.abort();
    info!("Shutdown signal received.");

    Ok(())
}
