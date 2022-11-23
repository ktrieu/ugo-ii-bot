-- Add migration script here
CREATE TABLE users (
    discord_id INT NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    PRIMARY KEY (discord_id)
);