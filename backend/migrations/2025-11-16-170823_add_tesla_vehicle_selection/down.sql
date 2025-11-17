-- Revert vehicle selection fields from tesla table
ALTER TABLE tesla DROP COLUMN selected_vehicle_id;
ALTER TABLE tesla DROP COLUMN selected_vehicle_name;
ALTER TABLE tesla DROP COLUMN selected_vehicle_vin;
