use serenity::builder::{CreateApplicationCommand, CreateApplicationCommands};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use sqlx::SqlitePool;

use crate::error::{Error, InnerError, WithContext};
use crate::user;

// This is kinda trash, but async function pointers are actually quite difficult to do.
// Instead, enjoy a bunch of string constants and a giant match table.

const TEST_COMMAND_NAME: &str = "test";

fn test_command_register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name(TEST_COMMAND_NAME)
        .description("A test command for the UGO bot")
}

async fn test_command_run(
    db: &SqlitePool,
    context: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), Error> {
    let discord_id = command.user.id;
    let user = user::get_user(db, &discord_id)
        .await
        .with_context("Fetching command user")?;

    let content: String = format!("Hello {}", user.display_name);

    command
        .create_interaction_response(&context.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(content))
        })
        .await
        .with_context("Creating command response")?;

    Ok(())
}

pub fn register_commands(
    commands: &mut CreateApplicationCommands,
) -> &mut CreateApplicationCommands {
    commands.create_application_command(|command| test_command_register(command))
}

pub async fn run_command(
    command_name: &str,
    db: &SqlitePool,
    context: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), Error> {
    match command_name {
        TEST_COMMAND_NAME => test_command_run(db, context, command).await,
        _ => Err(InnerError::CommandNotFound(command_name.to_string()).into()),
    }
}
