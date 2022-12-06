use chrono::{DateTime, Local};
use sqlx::{Executor, Sqlite};

use crate::error::Error;

use super::account::{Ugocoin, UgocoinAccount};

pub struct UgocoinTransaction {
    id: i64,
    tx_time: DateTime<Local>,
    from_account_id: i64,
    to_account_id: i64,
    memo: String,
}

pub async fn create_log<'a, E: Executor<'a, Database = Sqlite>>(
    db: E,
    from: &UgocoinAccount,
    to: &UgocoinAccount,
    amount: Ugocoin,
    memo: &String,
) -> Result<(), Error> {
    let now_unix = Local::now().timestamp();
    let ugocents = amount.as_ugocents();

    sqlx::query!(
        "INSERT into ugocoin_tx_logs (tx_time, from_account_id, to_account_id, amount, memo) VALUES (?, ?, ?, ?, ?)",
        now_unix, from.id, to.id, ugocents, memo
    ).execute(db).await?;

    Ok(())
}
