-- Your SQL goes here
alter table user_settings add column magic_login_token text;
alter table user_settings add column magic_login_token_expiration_timestamp integer;
