-- This migration is not reversible as we don't store the original tier information
-- If you need to revert, you'll need to manually update users based on their Stripe subscription
SELECT 'Migration consolidate_to_tier_2 is not reversible' AS warning;
