-- Add region field to tesla table
ALTER TABLE tesla ADD COLUMN region TEXT NOT NULL DEFAULT 'https://fleet-api.prd.eu.vn.cloud.tesla.com';
