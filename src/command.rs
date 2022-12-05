use serenity::builder::{CreateApplicationCommand, CreateApplicationCommands};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use sqlx::SqlitePool;

use crate::error::{Error, InnerError, WithContext};
use crate::ugocoin::{self, Ugocoin};
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

const BALANCES_COMMAND_NAME: &str = "balances";

fn balances_command_register(
    command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
    command
        .name(BALANCES_COMMAND_NAME)
        .description("Display balances for all employees, in the name of financial transparency.")
}

struct BalanceInfo {
    name: String,
    balance: Ugocoin,
    streak: i64,
}

async fn balances_command_run(
    db: &SqlitePool,
    context: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), Error> {
    let users = user::get_all_users(db).await?;

    let mut balance_infos: Vec<BalanceInfo> = Vec::new();

    for u in users {
        let account = ugocoin::get_user_account(db, &u).await?;
        balance_infos.push(BalanceInfo {
            name: u.display_name,
            balance: account.balance,
            streak: u.streak,
        });
    }

    let central_account = ugocoin::get_central_bank_account(db).await?;
    balance_infos.push(BalanceInfo {
        name: String::from("UGOcoin Central Bank"),
        balance: central_account.balance,
        streak: 0,
    });

    balance_infos.sort_by_cached_key(|info| info.balance);
    balance_infos.reverse();

    let mut content: String = String::from("Current UGOcoin balances:\n\n");

    for info in balance_infos {
        content += &format!(
            "{}: {} (scrum streak {})\n",
            info.name, info.balance, info.streak
        );
    }

    command
        .create_interaction_response(&context.http, |resp| {
            resp.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|data| data.content(content))
        })
        .await?;

    Ok(())
}

pub fn register_commands(
    commands: &mut CreateApplicationCommands,
) -> &mut CreateApplicationCommands {
    commands
        .create_application_command(test_command_register)
        .create_application_command(balances_command_register)
}

pub async fn run_command(
    command_name: &str,
    db: &SqlitePool,
    context: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), Error> {
    match command_name {
        TEST_COMMAND_NAME => test_command_run(db, context, command).await,
        BALANCES_COMMAND_NAME => balances_command_run(db, context, command).await,
        _ => Err(InnerError::CommandNotFound(command_name.to_string()).into()),
    }
}
