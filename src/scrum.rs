use chrono::{DateTime, Local, Timelike};

use serenity::client::Context;
use serenity::model::id::ChannelId;
use serenity::model::id::MessageId;

use sqlx::SqlitePool;

use crate::error::Error;

pub struct Scrum {
    id: i64,
    is_open: bool,
    scrum_date: String,
    message_id: String,
}

pub fn date_to_scrum_db_format(date: DateTime<Local>) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub async fn create_scrum_row(
    db: &SqlitePool,
    date: DateTime<Local>,
    message_id: MessageId,
) -> Result<(), Error> {
    let date_str = date_to_scrum_db_format(date);
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

pub async fn does_open_scrum_exist(db: &SqlitePool, date: DateTime<Local>) -> Result<bool, Error> {
    let date_str = date_to_scrum_db_format(date);

    let scrum = sqlx::query!(
        "SELECT is_open FROM scrums WHERE scrum_date = ? and is_open = true",
        date_str
    )
    .fetch_optional(db)
    .await?;

    Ok(scrum.is_some())
}

fn is_past_scrum_notification_time(datetime: DateTime<Local>) -> bool {
    // We only notify past 3 AM
    datetime.hour() > 3
}

pub async fn should_create_scrum(
    db: &SqlitePool,
    datetime: DateTime<Local>,
) -> Result<bool, Error> {
    Ok(is_past_scrum_notification_time(datetime) && !does_open_scrum_exist(db, datetime).await?)
}

const SCRUM_NOTIFY_STRING: &str = "AUTOMATED SCRUM TEST: This is a test of the new scrum system. 
This does not indicate a scrum.";

pub async fn notify_scrum(
    db: &SqlitePool,
    date: DateTime<Local>,
    ctx: &Context,
    channel_id: ChannelId,
) -> Result<(), Error> {
    let message = channel_id
        .send_message(&ctx.http, |message| message.content(SCRUM_NOTIFY_STRING))
        .await?;

    let result = create_scrum_row(db, date, message.id).await;

    // Roll back the message on databse failure, so we can re-try next time this job runs
    if let Err(err) = result {
        // If the delete fails, just throw up our hands and give up
        message.delete(&ctx.http).await?;
        return Err(err);
    }

    Ok(())
}
