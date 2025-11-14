-- Add messaging_service_sid for US subaccounts
ALTER TABLE subaccounts ADD COLUMN messaging_service_sid TEXT DEFAULT NULL;
