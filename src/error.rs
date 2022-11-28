#[derive(Debug)]
pub enum Error {
    DatabaseError(sqlx::Error),
    DiscordError(serenity::Error),
    UserNotFound,
    ScrumNotFound,
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
