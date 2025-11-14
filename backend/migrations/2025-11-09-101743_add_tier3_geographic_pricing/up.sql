-- Add fields for tier 3 geographic pricing to user_settings
ALTER TABLE user_settings ADD COLUMN monthly_message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_settings ADD COLUMN outbound_message_pricing REAL;

-- Add subaccount type tracking to subaccounts table
ALTER TABLE subaccounts ADD COLUMN subaccount_type TEXT NOT NULL DEFAULT 'full_service';
ALTER TABLE subaccounts ADD COLUMN country_code TEXT;

-- Create country availability cache table
CREATE TABLE country_availability (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    country_code TEXT NOT NULL UNIQUE,
    has_local_numbers BOOLEAN NOT NULL DEFAULT 0,
    outbound_sms_price REAL,
    inbound_sms_price REAL,
    outbound_voice_price_per_min REAL,
    inbound_voice_price_per_min REAL,
    last_checked INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
