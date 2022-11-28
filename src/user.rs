use sqlx::SqlitePool;

use serenity::model::id::UserId;

use crate::error::Error;
pub struct User {
    pub id: i64,
    pub discord_id: String,
    pub display_name: String,
}

pub async fn get_user(db: &SqlitePool, user_id: &UserId) -> Result<User, Error> {
    let user_id_str = user_id.to_string();

    let query = sqlx::query_as!(
        User,
        "SELECT 
        users.id, users.display_name, 
        users_discord_ids.discord_id FROM users
        LEFT JOIN users_discord_ids ON users_discord_ids.user_id = users.id 
        WHERE users_discord_ids.discord_id = ?",
        user_id_str
    )
    .fetch_one(db)
    .await;

    query.map_err(|err| match err {
        sqlx::Error::RowNotFound => Error::UserNotFound,
        _ => err.into(),
    })
}
