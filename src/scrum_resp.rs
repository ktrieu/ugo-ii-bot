use chrono::DateTime;
use chrono::Local;
use serenity::client::Context;
use serenity::model::channel::{Reaction, ReactionType};

use sqlx::SqlitePool;

use crate::error::Error;
use crate::scrum;
use crate::user;

pub async fn scrum_response_exists(
    db: &SqlitePool,
    scrum: &scrum::Scrum,
    user: &user::User,
) -> Result<bool, Error> {
    Ok(sqlx::query!(
        "SELECT id FROM scrum_responses WHERE scrum_id = ? AND user_id = ?",
        scrum.id,
        user.id
    )
    .fetch_optional(db)
    .await?
    .is_some())
}

pub async fn create_scrum_response(
    db: &SqlitePool,
    scrum: &scrum::Scrum,
    user: &user::User,
    available: bool,
    response_time: i64,
) -> Result<(), Error> {
    sqlx::query!(
        "INSERT INTO scrum_responses 
        (scrum_id, user_id, available, response_time) 
        VALUES (?, ?, ?, ?)",
        scrum.id,
        user.id,
        available,
        response_time
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn update_scrum_response(
    db: &SqlitePool,
    scrum: &scrum::Scrum,
    user: &user::User,
    available: bool,
    response_time: i64,
) -> Result<(), Error> {
    sqlx::query!(
        "UPDATE scrum_responses SET available = ?, response_time = ?
         WHERE scrum_id = ? AND user_id = ?",
        available,
        response_time,
        scrum.id,
        user.id
    )
    .execute(db)
    .await?;

    Ok(())
}

fn get_availablity_from_react(react: &Reaction) -> Option<bool> {
    if let ReactionType::Unicode(emoji) = &react.emoji {
        match emoji.as_str() {
            "👍" => Some(true),
            "👎" => Some(false),
            _ => None,
        }
    } else {
        None
    }
}

pub struct ScrumReact {
    available: bool,
    user: user::User,
    scrum: scrum::Scrum,
}

pub async fn parse_react(
    db: &SqlitePool,
    ctx: &Context,
    react: &Reaction,
) -> Result<Option<ScrumReact>, Error> {
    let available = match get_availablity_from_react(react) {
        Some(available) => available,
        // This emote isn't a thumbs down or thumbs up, so it isn't really a response.
        None => return Ok(None),
    };

    let message = react.message(&ctx.http).await?;

    let scrum = match scrum::get_scrum_from_message(db, message.id).await? {
        Some(scrum) => scrum,
        // This isn't a react for a scrum message.
        None => return Ok(None),
    };

    if !scrum.is_open {
        // We can't respond to non-open scrums.
        return Ok(None);
    }

    let user_id = match react.user_id {
        Some(id) => id,
        // Reacts not created by users aren't responses.
        None => return Ok(None),
    };

    let user = user::get_user(db, &user_id).await?;

    Ok(Some(ScrumReact {
        available: available,
        scrum: scrum,
        user: user,
    }))
}

pub async fn respond_scrum(
    db: &SqlitePool,
    react: &ScrumReact,
    response_time: DateTime<Local>,
) -> Result<(), Error> {
    if scrum_response_exists(db, &react.scrum, &react.user).await? {
        update_scrum_response(
            db,
            &react.scrum,
            &react.user,
            react.available,
            response_time.timestamp(),
        )
        .await?;
    } else {
        create_scrum_response(
            db,
            &react.scrum,
            &react.user,
            react.available,
            response_time.timestamp(),
        )
        .await?;
    }

    Ok(())
}

async fn delete_resp_if_exists(
    db: &SqlitePool,
    scrum: &scrum::Scrum,
    user: &user::User,
    available: bool,
) -> Result<(), Error> {
    // IMPORTANT: We query on available here because we take the *last* react as the response. If someone reacts with both
    // emojis, and then deletes their first react, not checking available would falsely delete the response.
    sqlx::query!(
        "DELETE FROM scrum_responses WHERE scrum_id = ? AND user_id = ? AND available = ?;",
        scrum.id,
        user.id,
        available
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn unrespond_scrum(db: &SqlitePool, react: &ScrumReact) -> Result<(), Error> {
    delete_resp_if_exists(db, &react.scrum, &react.user, react.available).await?;

    Ok(())
}
