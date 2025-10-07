-- This file should undo anything in `up.sql`
alter table user_settings drop column magic_login_token;
alter table user_settings drop column magic_login_token_expiration_timestamp;
