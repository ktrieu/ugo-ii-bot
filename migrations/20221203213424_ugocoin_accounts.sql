CREATE TABLE ugocoin_accounts (
    id INTEGER PRIMARY KEY NOT NULL,
    balance INTEGER NOT NULL,
    user_id INTEGER UNIQUE,
    FOREIGN KEY(user_id) REFERENCES users(id)
);