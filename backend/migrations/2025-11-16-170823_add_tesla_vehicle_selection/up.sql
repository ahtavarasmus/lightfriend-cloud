-- Add vehicle selection fields to tesla table
ALTER TABLE tesla ADD COLUMN selected_vehicle_vin TEXT;
ALTER TABLE tesla ADD COLUMN selected_vehicle_name TEXT;
ALTER TABLE tesla ADD COLUMN selected_vehicle_id TEXT;
