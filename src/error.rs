use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DatabaseError(sqlx::Error),
    DiscordError(serenity::Error),
    UserNotFound,
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Error::DatabaseError(err)
    }
}

impl From<serenity::Error> for Error {
    fn from(err: serenity::Error) -> Self {
        Error::DiscordError(err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DatabaseError(db_err) => f.write_fmt(format_args!("Database error: {}", db_err)),
            Error::DiscordError(discord_err) => {
                f.write_fmt(format_args!("Discord error: {}", discord_err))
            }
            Error::UserNotFound => f.write_str("User not found."),
        }
    }
}

impl std::error::Error for Error {}
