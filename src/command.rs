use std::collections::HashMap;

use serenity::async_trait;
use serenity::builder::CreateApplicationCommand;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::id::GuildId;

use sqlx::SqlitePool;

use crate::error::{Error, InnerError, WithContext};
use crate::ugocoin::{self, Ugocoin};
use crate::user;

#[async_trait]
trait Command {
    fn name(&self) -> &'static str;
    fn register<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand;
    async fn run(
        &self,
        db: &SqlitePool,
        context: &Context,
        command: &ApplicationCommandInteraction,
    ) -> Result<(), Error>;
}

struct TestCommand {}

#[async_trait]
impl Command for TestCommand {
    fn name(&self) -> &'static str {
        "test"
    }

    fn register<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand {
        command
            .name(self.name())
            .description("A test command for the UGO bot")
    }

    async fn run(
        &self,
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
}

struct BalanceInfo {
    name: String,
    balance: Ugocoin,
    streak: i64,
}
struct BalanceCommand {}

#[async_trait]
impl Command for BalanceCommand {
    fn name(&self) -> &'static str {
        "balances"
    }

    fn register<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand {
        command.name(self.name()).description(
            "Display balances for all employees, in the name of financial transparency.",
        )
    }

    async fn run(
        &self,
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

        let mut balance_string: String = String::new();

        let max_name_width = balance_infos.iter().map(|i| i.name.len()).max().unwrap();
        let max_coin_width = balance_infos
            .iter()
            .map(|i| format!("{}", i.balance).len())
            .max()
            .unwrap();

        for info in balance_infos {
            balance_string += &format!(
                "{:<max_name_width$} | {:<max_coin_width$} | (scrum streak {})\n",
                info.name, info.balance, info.streak,
            );
        }

        command
            .create_interaction_response(&context.http, |resp| {
                resp.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|data| {
                        data.content(format!(
                            "Current UGOcoin balances:\n\n```{}```",
                            balance_string
                        ))
                    })
            })
            .await?;

        Ok(())
    }
}

type CommandMap = HashMap<String, Box<dyn Command + 'static + Send + Sync>>;

fn insert_command<C: Command + 'static + Send + Sync>(map: &mut CommandMap, command: C) {
    map.insert(command.name().to_string(), Box::new(command));
}

lazy_static! {
    static ref COMMAND_MAP: CommandMap = {
        let mut m: CommandMap = HashMap::new();
        insert_command(&mut m, TestCommand {});
        insert_command(&mut m, BalanceCommand {});
        m
    };
}

pub async fn register_commands(context: &Context, guild_id: GuildId) -> Result<(), Error> {
    guild_id
        .set_application_commands(&context.http, |commands| {
            for (_, command) in COMMAND_MAP.iter() {
                commands.create_application_command(|command_builder| {
                    command.register(command_builder)
                });
            }
            commands
        })
        .await?;

    Ok(())
}

pub async fn run_command(
    db: &SqlitePool,
    context: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), Error> {
    let command_struct = COMMAND_MAP.get(&command.data.name);

    match command_struct {
        Some(command_struct) => {
            command_struct.run(db, context, command).await?;
            Ok(())
        }
        None => Err(InnerError::CommandNotFound(command.data.name.clone()).into()),
    }
}
