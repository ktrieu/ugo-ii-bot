use std::collections::HashMap;

use chrono::NaiveDate;
use chrono::{DateTime, Local, Timelike};

use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::channel::ReactionType;
use serenity::model::id::ChannelId;
use serenity::model::id::MessageId;

use sqlx::SqlitePool;

use crate::error::{Error, InnerError};
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

async fn get_scrum_for_date(
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

pub async fn does_scrum_exist(db: &SqlitePool, datetime: DateTime<Local>) -> Result<bool, Error> {
    let scrum = get_scrum_for_date(db, datetime).await?;

    Ok(scrum.is_some())
}

fn is_past_scrum_notification_time(datetime: DateTime<Local>) -> bool {
    datetime.hour() >= 3
}

pub async fn should_create_scrum(
    db: &SqlitePool,
    datetime: DateTime<Local>,
) -> Result<bool, Error> {
    Ok(is_past_scrum_notification_time(datetime) && !does_scrum_exist(db, datetime).await?)
}

const SCRUM_NOTIFY_STRING: &str = "AUTOMATED SCRUM TEST: This is a test of the new scrum system. 
This does not indicate a scrum.";

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
}

pub async fn parse_scrum_reactions(
    db: &SqlitePool,
    ctx: &Context,
    message: &Message,
) -> Result<ParsedScrumReacts, Error> {
    let mut user_availability: HashMap<user::User, ScrumReact> = HashMap::new();
    let mut num_available: u8 = 0;
    let mut num_unavailable: u8 = 0;

    let all_users = user::get_all_users(db).await?;
    for u in all_users {
        user_availability.insert(u, ScrumReact::Unknown);
    }

    // It's technically inefficient to refetch all the users that react, but we're going to have like four total,
    // so whatever.
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
        num_available += 1;
        user_availability.insert(avail_user, ScrumReact::Available);
    }

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
        num_unavailable += 1;
        user_availability.insert(unavail_user, ScrumReact::Unavailable);
    }

    Ok(ParsedScrumReacts {
        availability: user_availability,
        num_available: num_available,
        num_unavailable: num_unavailable,
    })
}

pub fn scrum_possible(reactions: &ParsedScrumReacts) -> bool {
    reactions.num_available >= 3 || reactions.num_unavailable >= 2
}

fn is_past_scrum_close_time(datetime: DateTime<Local>) -> bool {
    // Let's close past 10 PM
    datetime.hour() >= 22
}

async fn does_open_scrum_exist(db: &SqlitePool, datetime: DateTime<Local>) -> Result<bool, Error> {
    let scrum = get_scrum_for_date(db, datetime).await?;

    match scrum {
        Some(scrum) => Ok(scrum.is_open),
        None => Ok(false),
    }
}

pub async fn should_close_today_scrum(
    db: &SqlitePool,
    datetime: DateTime<Local>,
) -> Result<bool, Error> {
    Ok(is_past_scrum_close_time(datetime) && does_open_scrum_exist(db, datetime).await?)
}

pub async fn alert_scrum_possible(
    db: &SqlitePool,
    ctx: &Context,
    scrum: &Scrum,
    reactions: &ParsedScrumReacts,
    channel_id: ChannelId,
) -> Result<(), Error> {
    sqlx::query!("UPDATE scrums SET is_open = false WHERE id = ?", scrum.id)
        .execute(db)
        .await?;

    let mut close_message: String = format!(
        "SCRUM POSSIBLE: {}/{} available for scrum.\n\nNot available:\n",
        reactions.num_available,
        reactions.availability.len()
    );

    for (u, avail) in &reactions.availability {
        match avail {
            ScrumReact::Available => {}
            _ => close_message += &format!("{}\n", u.display_name),
        }
    }

    channel_id
        .send_message(&ctx.http, |message| message.content(close_message))
        .await?;

    Ok(())
}
