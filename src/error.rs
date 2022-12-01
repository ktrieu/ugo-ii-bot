use std::fmt::Display;

#[derive(Debug)]
pub enum InnerError {
    DatabaseError(sqlx::Error),
    DiscordError(serenity::Error),
    DateTimeParseError(chrono::ParseError),
    IdParseError(std::num::ParseIntError),
    UserNotFound,
}

#[derive(Debug)]
pub struct Error {
    pub error: InnerError,
    pub ctx: &'static str,
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Self {
            error: InnerError::DatabaseError(err),
            ctx: "",
        }
    }
}

impl From<serenity::Error> for Error {
    fn from(err: serenity::Error) -> Self {
        Self {
            error: InnerError::DiscordError(err),
            ctx: "",
        }
    }
}

impl From<chrono::ParseError> for Error {
    fn from(err: chrono::ParseError) -> Self {
        Self {
            error: InnerError::DateTimeParseError(err),
            ctx: "",
        }
    }
}

impl From<InnerError> for Error {
    fn from(inner: InnerError) -> Self {
        Self {
            error: inner,
            ctx: "",
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner_error_str = match &self.error {
            InnerError::DatabaseError(db_err) => format!("Database error: {}", db_err),
            InnerError::DiscordError(discord_err) => {
                format!("Discord error: {}", discord_err)
            }
            InnerError::DateTimeParseError(parse_err) => {
                format!("DateTime parse error: {}", parse_err)
            }
            InnerError::IdParseError(int_parse_err) => {
                format!("Error parsing ID from string: {}", int_parse_err)
            }
            InnerError::UserNotFound => "User not found.".to_string(),
        };

        f.write_fmt(format_args!("{} failed! ({})", self.ctx, inner_error_str))
    }
}

impl std::error::Error for Error {}

pub trait WithContext<T> {
    fn with_context(self, ctx: &'static str) -> Result<T, Error>;
}

impl<T, E> WithContext<T> for Result<T, E>
where
    Error: From<E>,
{
    fn with_context(self, ctx: &'static str) -> Result<T, Error> {
        self.map_err(|err| Error {
            error: Error::from(err).error,
            ctx: ctx,
        })
    }
}
