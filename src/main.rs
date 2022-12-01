extern crate dotenv;

use std::env;
use std::str::FromStr;
use std::time::Duration;

use dotenv::dotenv;
use error::WithContext;
use scrum::get_scrum_for_date;
use serenity::builder::CreateApplicationCommand;
use serenity::model::application::interaction::Interaction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::channel::Reaction;
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, GuildId};
use serenity::{async_trait, prelude::*};

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use chrono::prelude::*;

mod error;
mod scrum;
mod user;

struct Handler {
    db: SqlitePool,
}

fn test_command_register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("test")
        .description("A test command for the UGO bot")
}

const GENERAL_CHANNEL_ID: u64 = 822531930384891948;
const BOT_CHANNEL_ID: u64 = 1044762069070774332;

async fn job_poll_fn(db: &SqlitePool, ctx: Context) -> Result<(), error::Error> {
    let now = Local::now();

    let today_scrum = get_scrum_for_date(db, now)
        .await
        .with_context("Getting today's scrum")?;

    if scrum::should_create_scrum(now, today_scrum.as_ref()) {
        let channel_id = ChannelId(GENERAL_CHANNEL_ID);
        scrum::notify_scrum(db, now, &ctx, channel_id)
            .await
            .with_context("Notifying scrum")?;
    }

    if let Some(to_close) = scrum::should_force_close_scrum(now, today_scrum.as_ref()) {
        let channel_id = ChannelId(GENERAL_CHANNEL_ID);

        let message_id = to_close
            .message_id()
            .with_context("Parsing today's scrum message ID")?;

        let message = channel_id
            .message(&ctx.http, message_id)
            .await
            .with_context("Fetching message from today's scrum")?;

        let reactions = scrum::parse_scrum_reactions(db, &ctx, &message)
            .await
            .with_context("Parsing scrum reactions")?;
        let scrum_status = scrum::scrum_status(&reactions);

        if matches!(scrum_status, scrum::ScrumStatus::Unknown) {
            scrum::close_scrum(db, &ctx, &to_close, &reactions, channel_id, scrum_status)
                .await
                .with_context("Force closing scrum")?;
        }
    }

    Ok(())
}

// We need to separately declare these event functions so we can return a Result.
// I'd make a function that takes a closure to clean this up, but async closures are unstable :(
async fn interaction_create(
    db: &SqlitePool,
    ctx: &Context,
    interaction: Interaction,
) -> Result<(), error::Error> {
    if let Interaction::ApplicationCommand(command) = interaction {
        let discord_id = command.member.as_ref().unwrap().user.id;
        let user = user::get_user(db, &discord_id)
            .await
            .with_context("Fetching command user")?;

        let content: String = format!("Hello {}", user.display_name);

        command
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content(content))
            })
            .await
            .with_context("Creating command response")?;
    };

    Ok(())
}

async fn reaction_add(db: &SqlitePool, ctx: &Context, added: Reaction) -> Result<(), error::Error> {
    let now = Local::now();

    let scrum = match scrum::get_scrum_from_message(&db, added.message_id)
        .await
        .with_context("Fetching scrum from message")?
    {
        Some(scrum) => scrum,
        None => return Ok(()),
    };

    // If this is a closed scrum, ignore it.
    if !scrum.is_open {
        return Ok(());
    }

    // If this isn't today's scrum, ignore it.
    if scrum.date()? != now.date_naive() {
        return Ok(());
    }

    let discord_message = added
        .message(&ctx.http)
        .await
        .with_context("Finding Discord message for react")?;

    let reactions = scrum::parse_scrum_reactions(&db, &ctx, &discord_message)
        .await
        .with_context("Parsing scrum reactions")?;

    let scrum_status = scrum::scrum_status(&reactions);

    match scrum_status {
        scrum::ScrumStatus::Possible | scrum::ScrumStatus::Impossible => {
            scrum::close_scrum(
                db,
                ctx,
                &scrum,
                &reactions,
                ChannelId(GENERAL_CHANNEL_ID),
                scrum_status,
            )
            .await
            .with_context("Closing scrum")?;
        }
        // There's still time, do nothing
        scrum::ScrumStatus::Unknown => {}
    }

    Ok(())
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let result = interaction_create(&self.db, &ctx, interaction).await;

        if let Err(why) = result {
            println!("{}", why);
        }
    }

    async fn reaction_add(&self, ctx: Context, added: Reaction) {
        let result = reaction_add(&self.db, &ctx, added).await;

        if let Err(why) = result {
            println!("{}", why);
        }
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        let guild_id = GuildId(
            env::var("GUILD_ID")
                .expect("No GUILD_ID variable in environment!")
                .parse()
                .expect("GUILD_ID is not an integer!"),
        );

        let create_result = guild_id
            .set_application_commands(&ctx.http, |commands| {
                commands.create_application_command(|command| test_command_register(command))
            })
            .await;

        if let Err(why) = create_result {
            println!("Failed to create commands! {:?}", why);
        }

        let db = self.db.clone();
        tokio::spawn(async move {
            loop {
                let result = job_poll_fn(&db, ctx.clone()).await;
                if let Err(why) = result {
                    println!("{}", why);
                }

                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token =
        env::var("DISCORD_TOKEN").expect("No DISCORD_TOKEN found in environment variables!");

    let database_url =
        env::var("DATABASE_URL").expect("NO DATABASE_URL found in environment variables!");

    let connect_options = SqliteConnectOptions::from_str(&database_url)
        .expect("Failed to parse DATABASE_URL!")
        .create_if_missing(true);

    let database = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .expect("Failed to connect to database.");

    let handler = Handler { db: database };

    let intents = GatewayIntents::GUILD_MESSAGE_REACTIONS;

    let mut client = Client::builder(token, intents)
        .event_handler(handler)
        .await
        .expect("Failed to create Discord client!");

    if let Err(why) = client.start().await {
        panic!("Failed to start client {:?}", why);
    }
}
