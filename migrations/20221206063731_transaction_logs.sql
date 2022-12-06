CREATE TABLE ugocoin_tx_logs (
    id INTEGER PRIMARY KEY NOT NULL,
    tx_time INTEGER NOT NULL,
    from_account_id INTEGER NOT NULL,
    to_account_id INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    memo VARCHAR(255) NOT NULL,
    FOREIGN KEY (from_account_id) REFERENCES ugocoin_accounts(id),
    FOREIGN KEY (to_account_id) REFERENCES ugocoin_accounts(id)
);