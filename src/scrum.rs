use std::collections::HashMap;

use chrono::NaiveDate;
use chrono::{DateTime, Local, Timelike};

use log::info;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::channel::ReactionType;
use serenity::model::id::ChannelId;
use serenity::model::id::MessageId;

use sqlx::SqlitePool;

use crate::error::{Error, InnerError};
use crate::ugocoin::account::{credit_account, get_user_account, Ugocoin};
use crate::user;

pub struct Scrum {
    pub id: i64,
    pub is_open: bool,
    pub scrum_date: String,
    pub message_id: String,
}

impl Scrum {
    pub fn date(&self) -> Result<NaiveDate, Error> {
        Ok(NaiveDate::parse_from_str(&self.scrum_date, "%Y-%m-%d")?)
    }

    pub fn message_id(&self) -> Result<MessageId, Error> {
        let id_int: u64 = self
            .message_id
            .parse()
            .map_err(|err| Error::from(InnerError::IdParseError(err)))?;
        Ok(MessageId(id_int))
    }
}

pub fn date_to_scrum_db_format(date: DateTime<Local>) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub async fn create_scrum_row(
    db: &SqlitePool,
    datetime: DateTime<Local>,
    message_id: MessageId,
) -> Result<(), Error> {
    let date_str = date_to_scrum_db_format(datetime);
    let message_str = message_id.to_string();

    sqlx::query!(
        "
        INSERT INTO scrums (scrum_date, is_open, message_id)
        VALUES (?, true, ?)
    ",
        date_str,
        message_str
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_scrum_from_message(
    db: &SqlitePool,
    message_id: MessageId,
) -> Result<Option<Scrum>, Error> {
    let message_str = message_id.to_string();

    let result = sqlx::query_as!(
        Scrum,
        "SELECT id, is_open, scrum_date, message_id from scrums WHERE message_id = ?",
        message_str
    )
    .fetch_optional(db)
    .await?;

    Ok(result)
}

pub async fn get_scrum_for_date(
    db: &SqlitePool,
    datetime: DateTime<Local>,
) -> Result<Option<Scrum>, Error> {
    let date_str = date_to_scrum_db_format(datetime);

    Ok(sqlx::query_as!(
        Scrum,
        "SELECT id, is_open, scrum_date, message_id FROM scrums WHERE scrum_date = ?",
        date_str
    )
    .fetch_optional(db)
    .await?)
}

fn is_past_scrum_notification_time(datetime: DateTime<Local>) -> bool {
    datetime.hour() >= 3
}

pub fn should_create_scrum(datetime: DateTime<Local>, today_scrum: Option<&Scrum>) -> bool {
    is_past_scrum_notification_time(datetime) && today_scrum.is_none()
}

const SCRUM_NOTIFY_STRING: &str = "@everyone
UGO-BOT SCRUMMONS: React if you are available for scrum!";

const SCRUM_ACCEPT_EMOJI: &str = "👍";
const SCRUM_DECLINE_EMOJI: &str = "👎";

pub async fn notify_scrum(
    db: &SqlitePool,
    datetime: DateTime<Local>,
    ctx: &Context,
    channel_id: ChannelId,
) -> Result<(), Error> {
    let message = channel_id
        .send_message(&ctx.http, |message| message.content(SCRUM_NOTIFY_STRING))
        .await?;

    message
        .react(
            &ctx.http,
            ReactionType::Unicode(SCRUM_ACCEPT_EMOJI.to_string()),
        )
        .await?;
    message
        .react(
            &ctx.http,
            ReactionType::Unicode(SCRUM_DECLINE_EMOJI.to_string()),
        )
        .await?;

    let result = create_scrum_row(db, datetime, message.id).await;

    // Roll back the message on databse failure, so we can re-try next time this job runs
    if let Err(err) = result {
        // If the delete fails, just throw up our hands and give up
        message.delete(&ctx.http).await?;
        return Err(err);
    }

    Ok(())
}

#[derive(Debug)]
pub enum ScrumReact {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug)]
pub struct ParsedScrumReacts {
    pub availability: HashMap<user::User, ScrumReact>,
    pub num_available: u8,
    pub num_unavailable: u8,
    pub num_unknown: u8,
}

pub async fn parse_scrum_reactions(
    db: &SqlitePool,
    ctx: &Context,
    message: &Message,
) -> Result<ParsedScrumReacts, Error> {
    let mut user_availability: HashMap<user::User, ScrumReact> = HashMap::new();

    let all_users = user::get_all_users(db).await?;
    for u in all_users {
        user_availability.insert(u, ScrumReact::Unknown);
    }

    // It's technically inefficient to refetch all the users that react, but we're going to have like four total,
    // so whatever.
    for unavail_discord_user in message
        .reaction_users(
            &ctx.http,
            ReactionType::Unicode(SCRUM_DECLINE_EMOJI.to_string()),
            None,
            None,
        )
        .await?
    {
        let unavail_user = match user::get_user(db, &unavail_discord_user.id).await {
            Ok(user) => Ok(user),
            Err(Error {
                error: InnerError::UserNotFound,
                ..
            }) => continue,
            Err(other) => Err(other),
        }?;
        user_availability.insert(unavail_user, ScrumReact::Unavailable);
    }

    for avail_discord_user in message
        .reaction_users(
            &ctx.http,
            ReactionType::Unicode(SCRUM_ACCEPT_EMOJI.to_string()),
            None,
            None,
        )
        .await?
    {
        let avail_user = match user::get_user(db, &avail_discord_user.id).await {
            Ok(user) => Ok(user),
            Err(Error {
                error: InnerError::UserNotFound,
                ..
            }) => continue,
            Err(other) => Err(other),
        }?;
        user_availability.insert(avail_user, ScrumReact::Available);
    }

    let num_available = user_availability
        .values()
        .filter(|v| matches!(v, ScrumReact::Available))
        .count();

    let num_unavailable = user_availability
        .values()
        .filter(|v| matches!(v, ScrumReact::Unavailable))
        .count();

    let num_unknown = user_availability.len() - num_available - num_unavailable;

    // If we have more than 255 users in each category, I guess I'll change this.
    Ok(ParsedScrumReacts {
        availability: user_availability,
        num_available: num_available.try_into().unwrap(),
        num_unavailable: num_unavailable.try_into().unwrap(),
        num_unknown: num_unknown.try_into().unwrap(),
    })
}

fn is_past_scrum_close_time(datetime: DateTime<Local>) -> bool {
    // Auto close scrums after 4 PM
    datetime.hour() >= 12 + 4
}

pub fn should_force_close_scrum(
    datetime: DateTime<Local>,
    today_scrum: Option<&Scrum>,
) -> Option<&Scrum> {
    today_scrum.filter(|today_scrum| today_scrum.is_open && is_past_scrum_close_time(datetime))
}
#[derive(Debug)]
pub enum ScrumStatus {
    Possible,
    Impossible,
    Unknown,
}

fn format_scrum_close_notif(reactions: &ParsedScrumReacts, scrum_status: ScrumStatus) -> String {
    let header_msg = match scrum_status {
        ScrumStatus::Possible => "SCRUM POSSIBLE",
        _ => "SCRUM FAILED",
    };

    let mut close_message: String = format!(
        "@everyone {}: {}/{} available for scrum.\n\nNot available:\n",
        header_msg,
        reactions.num_available,
        reactions.availability.len()
    );

    for (u, avail) in &reactions.availability {
        match avail {
            ScrumReact::Available => {}
            _ => close_message += &format!("{}\n", u.display_name),
        }
    }

    close_message
}

pub fn scrum_status(reactions: &ParsedScrumReacts) -> ScrumStatus {
    if reactions.num_available >= 3 {
        ScrumStatus::Possible
    } else if reactions.num_unavailable >= 2 {
        ScrumStatus::Impossible
    } else {
        ScrumStatus::Unknown
    }
}

fn calculate_scrum_reward(streak: i64) -> Ugocoin {
    Ugocoin::from_ugocoin(1 + streak / 7)
}

const SCRUM_CLOSED_MESSAGE: &str = "Voting is closed for this scrum.";

pub async fn close_scrum(
    db: &SqlitePool,
    ctx: &Context,
    scrum: &Scrum,
    reactions: &ParsedScrumReacts,
    channel_id: ChannelId,
    scrum_status: ScrumStatus,
) -> Result<(), Error> {
    sqlx::query!("UPDATE scrums SET is_open = false WHERE id = ?", scrum.id)
        .execute(db)
        .await?;

    let mut message = channel_id.message(&ctx.http, scrum.message_id()?).await?;

    message
        .edit(&ctx.http, |edited| edited.content(SCRUM_CLOSED_MESSAGE))
        .await?;

    let msg = format_scrum_close_notif(reactions, scrum_status);

    channel_id
        .send_message(&ctx.http, |message| message.content(msg))
        .await?;

    for (user, avail) in &reactions.availability {
        match avail {
            ScrumReact::Available | ScrumReact::Unavailable => {
                user::increment_streak(db, user.id).await?;
                info!(
                    "New streak for {} is {}",
                    user.display_name,
                    user.streak + 1
                );
                // Account for the updated streak when calculating the reward
                let reward = calculate_scrum_reward(user.streak + 1);
                info!("Crediting scrum reward of {}", reward);

                let user_account = get_user_account(db, user).await?;
                let memo = format!("Scrum reward for {}", scrum.date()?.format("%Y-%m-%d"));

                credit_account(db, &user_account, reward, &memo).await?;
            }
            ScrumReact::Unknown => {
                user::clear_streak(db, user.id).await?;
            }
        }
    }

    Ok(())
}
