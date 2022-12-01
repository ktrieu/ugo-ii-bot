use std::hash::Hash;

use sqlx::SqlitePool;

use serenity::model::id::UserId;

use crate::error::Error;

#[derive(Debug)]
pub struct User {
    pub id: i64,
    pub display_name: String,
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for User {}

impl Hash for User {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

pub async fn get_user(db: &SqlitePool, user_id: &UserId) -> Result<User, Error> {
    let user_id_str = user_id.to_string();

    let query = sqlx::query_as!(
        User,
        "SELECT 
        users.id, users.display_name FROM users
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

pub async fn get_all_users(db: &SqlitePool) -> Result<Vec<User>, Error> {
    Ok(
        sqlx::query_as!(User, "SELECT users.id, users.display_name FROM users")
            .fetch_all(db)
            .await?,
    )
}
