extern crate dotenv;

use std::env;
use std::str::FromStr;
use std::time::Duration;

use dotenv::dotenv;
use scrum::should_create_scrum;
use serenity::builder::CreateApplicationCommand;
use serenity::model::application::interaction::Interaction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, GuildId};
use serenity::{async_trait, prelude::*};

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use chrono::prelude::*;

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

async fn job_poll_fn(db: &SqlitePool, ctx: Context) {
    let now = Local::now();

    match should_create_scrum(&db, now).await {
        Ok(true) => {
            let channel_id = ChannelId(BOT_CHANNEL_ID);
            if let Err(why) = scrum::notify_scrum(db, now, &ctx, channel_id).await {
                println!("Failed to notify scrum: {:?}", why);
            }
        }
        Ok(false) => (),
        Err(err) => println!("Failed to check scrum creation: {:?}", err),
    };
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let discord_id = command.member.as_ref().unwrap().user.id.to_string();
            let user = user::get_user(&self.db, &discord_id).await.unwrap();

            let content: String = format!("Hello {}", user.display_name);

            let resp_result = command
                .create_interaction_response(ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await;

            if let Err(why) = resp_result {
                println!("Could not respond to interaction! {:?}", why);
            }
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
                job_poll_fn(&db, ctx.clone()).await;
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

    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(handler)
        .await
        .expect("Failed to create Discord client!");

    if let Err(why) = client.start().await {
        panic!("Failed to start client {:?}", why);
    }
}
