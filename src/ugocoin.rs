use std::fmt::Display;

use crate::{error::Error, user::User};

use sqlx::SqlitePool;

use thousands::Separable;

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
}

impl Display for Ugocoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cents = self.0 % 100;
        let coins = self.0 / 100;

        if coins == 0 && cents == 0 {
            f.write_str("U$0.00")
        } else if coins > 0 {
            f.write_fmt(format_args!(
                "U${}.{:02}",
                coins.separate_with_commas(),
                cents
            ))
        } else {
            f.write_fmt(format_args!("{} U¢", cents))
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
