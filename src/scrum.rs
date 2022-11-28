use chrono::{DateTime, Local, Timelike};

use serenity::client::Context;
use serenity::model::channel::ReactionType;
use serenity::model::id::ChannelId;
use serenity::model::id::MessageId;

use sqlx::SqlitePool;

use crate::error::Error;

pub struct Scrum {
    pub id: i64,
    pub is_open: bool,
    pub scrum_date: String,
    pub message_id: String,
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
    // We only notify past 3 AM
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
        .react(&ctx.http, ReactionType::Unicode("👍".to_string()))
        .await?;
    message
        .react(&ctx.http, ReactionType::Unicode("👎".to_string()))
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

pub async fn close_todays_scrum(db: &SqlitePool, datetime: DateTime<Local>) -> Result<(), Error> {
    let scrum_date = date_to_scrum_db_format(datetime);

    sqlx::query!(
        "UPDATE scrums SET is_open = false WHERE scrum_date = ?",
        scrum_date
    )
    .execute(db)
    .await?;

    Ok(())
}
