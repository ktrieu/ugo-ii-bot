use chrono::{DateTime, Local, Timelike};

use serenity::client::Context;
use serenity::model::id::ChannelId;
use serenity::model::id::MessageId;

use sqlx::SqlitePool;

#[derive(Debug)]
pub enum ScrumError {
    SerenityError(serenity::Error),
    SqlxError(sqlx::Error),
}

pub fn date_to_scrum_db_format(date: DateTime<Local>) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub async fn create_scrum_row(
    db: &SqlitePool,
    date: DateTime<Local>,
    message_id: MessageId,
) -> Result<(), ScrumError> {
    let date_str = date_to_scrum_db_format(date);
    let message_str = message_id.to_string();

    let result = sqlx::query!(
        "
        INSERT INTO scrums (scrum_date, is_open, message_id)
        VALUES (?, true, ?)
    ",
        date_str,
        message_str
    )
    .execute(db)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx_err) => Err(ScrumError::SqlxError(sqlx_err)),
    }
}

pub async fn does_open_scrum_exist(
    db: &SqlitePool,
    date: DateTime<Local>,
) -> Result<bool, ScrumError> {
    let date_str = date_to_scrum_db_format(date);

    let result = sqlx::query!("SELECT is_open FROM scrums WHERE scrum_date = ?", date_str)
        .fetch_optional(db)
        .await
        .map_err(|err| ScrumError::SqlxError(err))?;

    result.map_or(Ok(false), |row| Ok(row.is_open))
}

fn is_past_scrum_notification_time(datetime: DateTime<Local>) -> bool {
    // We only notify past 3 AM
    datetime.hour() > 3
}

pub async fn should_create_scrum(
    db: &SqlitePool,
    datetime: DateTime<Local>,
) -> Result<bool, ScrumError> {
    Ok(is_past_scrum_notification_time(datetime) && !does_open_scrum_exist(db, datetime).await?)
}

const SCRUM_NOTIFY_STRING: &str = "AUTOMATED SCRUM TEST: This is a test of the new scrum system. 
This does not indicate a scrum.";

pub async fn notify_scrum(
    db: &SqlitePool,
    date: DateTime<Local>,
    ctx: &Context,
    channel_id: ChannelId,
) -> Result<(), ScrumError> {
    let message = channel_id
        .send_message(&ctx.http, |message| message.content(SCRUM_NOTIFY_STRING))
        .await
        .map_err(|err| ScrumError::SerenityError(err))?;

    create_scrum_row(db, date, message.id).await?;

    Ok(())
}
