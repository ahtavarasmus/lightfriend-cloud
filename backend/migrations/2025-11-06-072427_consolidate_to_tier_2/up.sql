-- Consolidate all tier 1 and tier 1.5 subscriptions to tier 2
-- Update subscription tier for all existing tier 1 and tier 1.5 users to tier 2
UPDATE users
SET sub_tier = 'tier 2'
WHERE sub_tier IN ('tier 1', 'tier 1.5');

-- Note: Credit adjustments will be handled by the application logic
-- The webhook handler will properly allocate credits based on region when the next billing cycle occurs
