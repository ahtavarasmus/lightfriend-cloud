-- Migrate users with ask_sender to notify all (NULL)
UPDATE user_settings
SET action_on_critical_message = NULL
WHERE action_on_critical_message = 'ask_sender';

-- Migrate users with ask_sender_exclude_family to notify_family
UPDATE user_settings
SET action_on_critical_message = 'notify_family'
WHERE action_on_critical_message = 'ask_sender_exclude_family';
