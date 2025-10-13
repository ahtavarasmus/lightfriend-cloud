-- Your SQL goes here
CREATE TABLE subaccounts (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL DEFAULT '-1',
    subaccount_sid TEXT NOT NULL UNIQUE,
    auth_token TEXT NOT NULL,
    country TEXT,
    number TEXT,
    cost_this_month FLOAT DEFAULT 0.0,
    created_at INTEGER
);

CREATE INDEX idx_subaccounts_user_id ON subaccounts(user_id);
CREATE INDEX idx_subaccounts_country ON subaccounts(country);
