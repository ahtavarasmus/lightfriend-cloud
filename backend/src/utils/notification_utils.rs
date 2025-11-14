use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use crate::models::user_models::User;
use tokio::time::{sleep, Duration};
use tracing;

/// Sends a notification message to the user with automatic retry logic.
///
/// This function ensures that critical user notifications are delivered even if
/// the first attempt fails. It will retry up to 3 times with exponential backoff.
///
/// # Arguments
/// * `state` - The application state
/// * `message` - The message to send to the user
/// * `media_sid` - Optional media SID for MMS messages
/// * `user` - The user to send the message to
///
/// # Returns
/// * `Ok(String)` - The message SID on success
/// * `Err(Box<dyn Error>)` - Error if all retry attempts fail
pub async fn send_user_notification_with_retry(
    state: &Arc<AppState>,
    message: &str,
    media_sid: Option<&String>,
    user: &User,
) -> Result<String, Box<dyn Error>> {
    const MAX_RETRIES: u32 = 3;
    const BASE_DELAY_MS: u64 = 500;

    let mut last_error: Option<Box<dyn Error>> = None;

    for attempt in 0..MAX_RETRIES {
        match crate::api::twilio_utils::send_conversation_message(
            state,
            message,
            media_sid,
            user,
        ).await {
            Ok(sid) => {
                if attempt > 0 {
                    tracing::info!(
                        "Successfully sent notification to user {} after {} retries",
                        user.id,
                        attempt
                    );
                }
                return Ok(sid);
            }
            Err(e) => {
                last_error = Some(e);
                tracing::warn!(
                    "Failed to send notification to user {} (attempt {}/{})",
                    user.id,
                    attempt + 1,
                    MAX_RETRIES
                );

                // Don't sleep after the last attempt
                if attempt < MAX_RETRIES - 1 {
                    // Exponential backoff: 500ms, 1000ms, 2000ms
                    let delay = Duration::from_millis(BASE_DELAY_MS * 2_u64.pow(attempt));
                    sleep(delay).await;
                }
            }
        }
    }

    // All retries failed - log to Sentry and return error
    let final_error = last_error.unwrap_or_else(|| "Unknown error".into());
    tracing::error!(
        "CRITICAL: Failed to send notification to user {} after {} attempts",
        user.id,
        MAX_RETRIES
    );

    // If Sentry is configured, capture this critical error
    #[cfg(feature = "sentry")]
    {
        sentry::capture_message(
            &format!(
                "Failed to send notification to user {} after {} retries",
                user.id, MAX_RETRIES
            ),
            sentry::Level::Error,
        );
    }

    Err(final_error)
}

/// Sends an error notification to the user with retry logic.
/// This is a convenience wrapper around send_user_notification_with_retry
/// that ensures error messages are always delivered to the user.
///
/// If the notification fails after all retries, it logs the error but does not panic.
pub async fn notify_user_of_error(
    state: &Arc<AppState>,
    error_message: &str,
    user: &User,
) {
    if let Err(e) = send_user_notification_with_retry(
        state,
        error_message,
        None,
        user,
    ).await {
        tracing::error!(
            "CRITICAL: Unable to notify user {} of error: {:?}",
            user.id,
            e
        );
    }
}

/// Sends an email to the admin (rasmus@ahtava.com) with usage statistics
/// for Tinfoil API key renewals. This helps monitor token consumption patterns.
///
/// # Arguments
/// * `state` - The application state
/// * `user_id` - The user ID requesting renewal
/// * `days_until_renewal` - Days remaining until next billing cycle
/// * `tokens_consumed` - Number of tokens consumed since last renewal
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err(Box<dyn Error>)` - Error sending email
pub async fn send_tinfoil_renewal_notification(
    state: &Arc<AppState>,
    user_id: i32,
    days_until_renewal: i32,
    tokens_consumed: i64,
) -> Result<(), Box<dyn Error>> {
    use axum::extract::{Json, State as AxumState};

    // Get user details
    let user = state.user_core.find_by_id(user_id)
        .map_err(|e| format!("Failed to find user: {}", e))?
        .ok_or("User not found")?;

    // Calculate tokens per day
    let days_elapsed = if days_until_renewal >= 30 {
        1  // Prevent division by zero on first renewal
    } else {
        30 - days_until_renewal
    };
    let tokens_per_day = if days_elapsed > 0 {
        tokens_consumed / days_elapsed as i64
    } else {
        tokens_consumed
    };

    // Prepare email body
    let body = format!(
        "Tinfoil API Key Renewal Request\n\
        =====================================\n\n\
        User ID: {}\n\
        User Email: {}\n\
        Days Until Next Billing: {}\n\
        Days Since Last Renewal: {}\n\
        Total Tokens Consumed: {}\n\
        Average Tokens/Day: {}\n\n\
        A new Tinfoil API key has been automatically generated for this user.\n\
        \n\
        Please review these usage statistics to determine if the monthly token limit should be adjusted.\n\
        ",
        user_id,
        user.email,
        days_until_renewal,
        days_elapsed,
        tokens_consumed,
        tokens_per_day
    );

    // Create email request
    let email_request = crate::handlers::imap_handlers::SendEmailRequest {
        to: "rasmus@ahtava.com".to_string(),
        subject: format!("Tinfoil Key Renewal - User {}", user_id),
        body: body.replace("\n", "\r\n"),  // CRLF for email
    };

    // Create a fake auth user for sending (admin context)
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: 1,
        is_admin: true,
    };

    // Send email
    match crate::handlers::imap_handlers::send_email(
        AxumState(state.clone()),
        auth_user,
        Json(email_request),
    ).await {
        Ok(_) => {
            tracing::info!("Successfully sent Tinfoil renewal notification for user {}", user_id);
            Ok(())
        }
        Err((status, err)) => {
            let error_msg = format!("Failed to send Tinfoil renewal notification: {:?} - {:?}", status, err);
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        }
    }
}
