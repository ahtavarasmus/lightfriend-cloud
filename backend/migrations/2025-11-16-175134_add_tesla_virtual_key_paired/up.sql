-- Add virtual_key_paired field to track whether the virtual key has been successfully paired with this vehicle
ALTER TABLE tesla ADD COLUMN virtual_key_paired INTEGER NOT NULL DEFAULT 0;
