-- Your SQL goes here
CREATE TABLE IF NOT EXISTS tesla (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_tesla_user_id ON tesla(user_id);