use crate::AppState;
use std::sync::Arc;

/// Checks if a user has sufficient credits to perform an action.
/// Returns Ok(()) if the user has enough credits, or Err with an appropriate error message if not.
/// Also handles automatic recharging if enabled.
pub async fn check_user_credits(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    event_type: &str,
    amount: Option<i32>,
) -> Result<(), String> {

    // Check if the event type is free based on discount_tier
    let messages_are_included;
    if user.phone_number.starts_with("+1") ||
            user.phone_number.starts_with("+358") ||
            user.phone_number.starts_with("+31") ||
            user.phone_number.starts_with("+44") ||
            user.phone_number.starts_with("+61") {
        messages_are_included = false;
    } else {
        // true since they pay to twilio. won't cause bug since sending messages outside the range will require their own creds
        messages_are_included = true; 
    }

    if messages_are_included {
        return Ok(());
    }

    // Define costs based on phone number
    let (message_cost, voice_second_cost, noti_msg_cost, noti_call_cost) = if user.phone_number.starts_with("+1") {
        (0.075, 0.0033, 0.075, 0.15) // US, CA
    } else if user.phone_number.starts_with("+358") {
        (0.30, 0.005, 0.15, 0.70) // Finland
    } else if user.phone_number.starts_with("+31") {
        (0.30, 0.005, 0.15, 0.45) // NL
    } else if user.phone_number.starts_with("+44") {
        (0.30, 0.005, 0.15, 0.20) // UK
    } else if user.phone_number.starts_with("+61") {
        (0.30, 0.005, 0.15, 0.20) // Australia
    } else {
        (0.0, 0.0, 0.0, 0.0) 
    };

    // Calculate cost based on event type
    let required_credits = match event_type {
        "message" => message_cost,
        "voice" => amount.unwrap_or(0) as f32 * voice_second_cost,
        "noti_msg" => noti_msg_cost,
        "noti_call" => noti_call_cost,
        "digest" => amount.unwrap_or(0) as f32 * message_cost,
        _ => return Err("Invalid event type".to_string()),
    };

    let required_credits_left= match event_type {
        "message" => 1.00,
        "voice" => 0.00,
        "noti_call" => 1.00 / 2.00,
        "noti_msg" => 1.00 / 2.00,
        "digest" => 1.00 * amount.unwrap_or(0) as f32,
        _ => return Err("Invalid event type".to_string()),
    };

    
    if (user.credits_left < 0.00 || user.credits_left < required_credits_left) && (user.credits < 0.0 || user.credits < required_credits) {
        // Check if enough time has passed since the last notification (24 hours)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        
        let should_notify = match user.last_credits_notification {
            None => true,
            Some(last_time) => (current_time - last_time) >= 24 * 3600 // 24 hours in seconds
        };

        if should_notify && event_type != "digest" {
            // Send notification about depleted credits and monthly quota
                
            // Update the last notification timestamp
            if let Err(e) = state.user_core.update_last_credits_notification(user.id, current_time) {
                eprintln!("Failed to update last_credits_notification: {}", e);
            }

            let user_clone = user.clone();
            let state_clone = state.clone();
            
            tokio::spawn(async move {
                let _ = crate::api::twilio_utils::send_conversation_message(
                    &state_clone,
                    "Your credits and monthly quota have been depleted. Please recharge your credits to continue using the service.",
                    None,
                    &user_clone,
                ).await;
            });
        }
        return Err("Insufficient credits. You have used all your monthly quota and don't have enough extra credits.".to_string());
    }

    // Check credits threshold and handle automatic charging
    match state.user_repository.is_credits_under_threshold(user.id) {
        Ok(is_under) => {
            if is_under && user.charge_when_under {
                println!("User {} credits is under threshold, attempting automatic charge", user.id);
                use axum::extract::{State, Path};
                let state_clone = Arc::clone(state);
                let user_id = user.id; // Clone the user ID
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user_id),
                    ).await;
                });
                println!("Initiated automatic recharge for user");
            }
        },
        Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
    }

    Ok(())
}

/// Deducts credits from a user's account, using monthly credits (credits_left) first before using regular credits.
/// Returns Ok(()) if credits were successfully deducted, or Err with an appropriate error message if not.
pub fn deduct_user_credits(
    state: &Arc<AppState>,
    user_id: i32,
    event_type: &str,
    amount: Option<i32>,
) -> Result<(), String> {
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => return Err("User not found".to_string()),
        Err(e) => {
            eprintln!("Database error while finding user {}: {}", user_id, e);
            return Err("Database error occurred".to_string());
        }
    };

    // For tier 3 self-hosted users, check if they have US/CA unlimited
    let is_tier3 = user.sub_tier.as_deref() == Some("tier 3");
    let user_settings = if is_tier3 {
        match state.user_core.get_user_settings(user_id) {
            Ok(settings) => Some(settings),
            Err(e) => {
                eprintln!("Failed to get user settings for tier 3 user {}: {}", user_id, e);
                None
            }
        }
    } else {
        None
    };

    let messages_are_included;
    if user.phone_number.starts_with("+1") ||
            user.phone_number.starts_with("+358") ||
            user.phone_number.starts_with("+31") ||
            user.phone_number.starts_with("+44") ||
            user.phone_number.starts_with("+61") {
        messages_are_included = false;
    } else {
        // true since they pay to twilio. won't cause bug since sending messages outside the range will require their own creds
        messages_are_included = true;
    }

    if messages_are_included {
        return Ok(());
    }

    // Define costs based on phone number or tier 3 dynamic pricing
    let (message_cost, voice_second_cost, noti_msg_cost, noti_call_cost) = if is_tier3 {
        // For tier 3, use outbound_message_pricing from user_settings
        if let Some(ref settings) = user_settings {
            if let Some(pricing) = settings.outbound_message_pricing {
                // Tier 3 dynamic pricing based on country
                (pricing, 0.005, pricing, pricing * 2.0)
            } else {
                // Fallback to US pricing if not set
                (0.075, 0.0033, 0.075, 0.15)
            }
        } else {
            (0.075, 0.0033, 0.075, 0.15)
        }
    } else if user.phone_number.starts_with("+1") {
        (0.075, 0.0033, 0.075, 0.15) // US
    } else if user.phone_number.starts_with("+358") {
        (0.30, 0.005, 0.15, 0.70) // Finland
    } else if user.phone_number.starts_with("+31") {
        (0.30, 0.005, 0.15, 0.45) // NL
    } else if user.phone_number.starts_with("+44") {
        (0.30, 0.005, 0.15, 0.20) // UK
    } else if user.phone_number.starts_with("+61") {
        (0.30, 0.005, 0.15, 0.20) // Australia
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    // Calculate cost based on event type
    let cost = match event_type {
        "message" => message_cost,
        "voice" => amount.unwrap_or(0) as f32 * voice_second_cost,
        "noti_msg" => noti_msg_cost,
        "noti_call" => noti_call_cost,
        "digest" => message_cost,
        _ => return Err("Invalid event type".to_string()),
    };

    let cost_credits_left= match event_type {
        "message" => 1.00,
        "voice" => 0.00,
        "noti_msg" => 1.00 / 2.00,
        "noti_call" => 1.00 / 2.00,
        "digest" => 1.00,
        _ => return Err("Invalid event type".to_string()),
    };

    // Deduct credits based on available credits_left
    if user.credits_left >= cost_credits_left {
        // Deduct from credits_left only
        if let Err(e) = state.user_repository.update_user_credits_left(user_id, (user.credits_left - cost_credits_left).max(0.0)) {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else {
        // Deduct from regular credits only
        let new_credits = (user.credits - cost).max(0.0);
        if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    }

    // For tier 3 US/CA users: Increment monthly message count and monitor for 1000 limit
    if is_tier3 && event_type == "message" {
        if let Some(ref settings) = user_settings {
            // Check if this is a US/CA subaccount (unlimited messaging with monitoring)
            // US/CA users have outbound_message_pricing of None or very low (we monitor all tier 3 messages)
            if let Err(e) = state.user_core.increment_monthly_message_count(user_id) {
                eprintln!("Failed to increment monthly message count for user {}: {}", user_id, e);
            }

            // Get updated settings to check current count
            if let Ok(updated_settings) = state.user_core.get_user_settings(user_id) {
                let count = updated_settings.monthly_message_count;

                // Send email alert when hitting 1000 messages (only send once when crossing threshold)
                if count >= 1000 && settings.monthly_message_count < 1000 {
                    tracing::warn!("Tier 3 user {} has reached {} messages this month", user_id, count);

                    // Send email notification to admin
                    let state_clone = state.clone();
                    let user_email = user.email.clone();
                    tokio::spawn(async move {
                        if let Err(e) = send_tier3_usage_alert(&state_clone, user_id, &user_email, count).await {
                            tracing::error!("Failed to send tier 3 usage alert: {}", e);
                        }
                    });
                }
            }
        }
    }

    Ok(())
}

/// Sends an email alert when a tier 3 user exceeds 1000 messages per month
async fn send_tier3_usage_alert(
    state: &Arc<AppState>,
    user_id: i32,
    user_email: &str,
    message_count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use axum::extract::{Json, State as AxumState};

    let body = format!(
        "Tier 3 Usage Alert - 1000 Messages Reached\n\
        ==========================================\n\n\
        User ID: {}\n\
        User Email: {}\n\
        Monthly Message Count: {}\n\n\
        This tier 3 self-hosted user has reached 1000 outbound messages this month.\n\
        This is a monitoring alert to track usage patterns.\n\
        ",
        user_id,
        user_email,
        message_count
    );

    let email_request = crate::handlers::imap_handlers::SendEmailRequest {
        to: "rasmus@ahtava.com".to_string(),
        subject: format!("Tier 3 Usage Alert - User {} - 1000 Messages", user_id),
        body: body.replace("\n", "\r\n"),
    };

    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: 1,
        is_admin: true,
    };

    match crate::handlers::imap_handlers::send_email(
        AxumState(state.clone()),
        auth_user,
        Json(email_request),
    ).await {
        Ok(_) => {
            tracing::info!("Successfully sent tier 3 usage alert for user {}", user_id);
            Ok(())
        }
        Err((status, err)) => {
            let error_msg = format!("Failed to send tier 3 usage alert: {:?} - {:?}", status, err);
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        }
    }
}
