use std::fmt::Display;

use crate::{
    error::{Error, InnerError},
    user::User,
};

use sqlx::SqlitePool;

use thousands::Separable;

use super::tx;

// Ugocoins are represented as a fixed-point number of ugocents.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Ugocoin(i64);

impl Ugocoin {
    pub fn from_ugocents(cents: i64) -> Ugocoin {
        Ugocoin(cents)
    }

    pub fn from_ugocoin(coins: i64) -> Ugocoin {
        Ugocoin(coins * 100)
    }

    pub fn as_ugocents(&self) -> i64 {
        self.0
    }
}

impl Display for Ugocoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cents = self.0 % 100;
        let coins = self.0 / 100;

        if coins == 0 && cents == 0 {
            f.pad("U$0.00")
        } else if coins > 0 {
            f.pad(&format!("U${}.{:02}", coins.separate_with_commas(), cents))
        } else {
            f.pad(&format!("{} U¢", cents))
        }
    }
}

pub struct UgocoinAccount {
    pub id: i64,
    pub user_id: Option<i64>,
    pub balance: Ugocoin,
}

pub async fn get_user_account(db: &SqlitePool, user: &User) -> Result<UgocoinAccount, Error> {
    let result = sqlx::query!(
        "SELECT id, user_id, balance from ugocoin_accounts WHERE user_id = ?",
        user.id
    )
    .fetch_one(db)
    .await?;

    Ok(UgocoinAccount {
        id: result.id,
        user_id: result.user_id,
        balance: Ugocoin::from_ugocents(result.balance),
    })
}

pub async fn get_central_bank_account(db: &SqlitePool) -> Result<UgocoinAccount, Error> {
    // The central bank account is the account with no user ID associated
    let result =
        sqlx::query!("SELECT id, user_id, balance from ugocoin_accounts WHERE user_id IS NULL",)
            .fetch_one(db)
            .await?;

    Ok(UgocoinAccount {
        id: result.id,
        user_id: result.user_id,
        balance: Ugocoin::from_ugocents(result.balance),
    })
}

pub async fn transfer(
    db: &SqlitePool,
    from: &UgocoinAccount,
    to: &UgocoinAccount,
    amount: Ugocoin,
    memo: &String,
) -> Result<(), Error> {
    if amount < Ugocoin::from_ugocents(0) {
        return Err(InnerError::NegativeTransfer.into());
    }

    if from.balance < amount {
        return Err(InnerError::InsufficientFunds.into());
    }

    let amount_ugocents = amount.as_ugocents();

    // Start a transaction since we need to debit/credit accounts and add a transaction log
    let mut db_tx = db.begin().await?;

    // Debit the from account
    sqlx::query!(
        "UPDATE ugocoin_accounts SET balance = balance - ? WHERE id = ?",
        amount_ugocents,
        from.id
    )
    .execute(&mut db_tx)
    .await?;

    // Credit the to account
    sqlx::query!(
        "UPDATE ugocoin_accounts SET balance = balance + ? WHERE id = ?",
        amount_ugocents,
        to.id
    )
    .execute(&mut db_tx)
    .await?;

    // And finally create the transaction log
    tx::create_log(&mut db_tx, from, to, amount, memo).await?;

    db_tx.commit().await?;

    Ok(())
}

// Credits an account from the central bank account.
pub async fn credit_account(
    db: &SqlitePool,
    to: &UgocoinAccount,
    amount: Ugocoin,
    memo: &String,
) -> Result<(), Error> {
    let central_account = get_central_bank_account(db).await?;
    transfer(db, &central_account, to, amount, memo).await?;

    Ok(())
}

// Debits an account, and sends the money back to the central bank account
pub async fn debit_account(
    db: &SqlitePool,
    from: &UgocoinAccount,
    amount: Ugocoin,
    memo: &String,
) -> Result<(), Error> {
    let central_account = get_central_bank_account(db).await?;
    transfer(db, from, &central_account, amount, memo).await?;

    Ok(())
}
