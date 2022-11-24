extern crate dotenv;

use std::env;
use std::str::FromStr;

use dotenv::dotenv;
use serenity::builder::CreateApplicationCommand;
use serenity::model::application::interaction::Interaction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::{async_trait, prelude::*};

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

mod user;

struct Handler {
    db: SqlitePool,
}

fn test_command_register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("test")
        .description("A test command for the UGO bot")
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
            .set_application_commands(ctx.http, |commands| {
                commands.create_application_command(|command| test_command_register(command))
            })
            .await;

        if let Err(why) = create_result {
            println!("Failed to create commands! {:?}", why);
        }
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
