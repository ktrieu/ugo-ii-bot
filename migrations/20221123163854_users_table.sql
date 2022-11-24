CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    display_name VARCHAR(255) NOT NULL
);

CREATE TABLE users_discord_ids (
    id INTEGER PRIMARY KEY NOT NULL,
    discord_id VARCHAR(255) NOT NULL,
    user_id INTEGER,
    FOREIGN KEY(user_id) REFERENCES users(id)
);