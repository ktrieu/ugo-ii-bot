INSERT INTO ugocoin_accounts (user_id, balance) SELECT users.id, 0 from users;
-- Add an account with a NULL user_id, this is the UGO Central Bank Account
INSERT INTO ugocoin_accounts (user_id, balance) VALUES (NULL, 0);