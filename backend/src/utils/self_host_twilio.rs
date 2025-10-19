use crate::AppState;
use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use reqwest::header::{CONTENT_TYPE};
use serde_json::{json, Value};

use crate::models::user_models::NewSubaccount;
use axum::{
    Json,
    extract::{State},

    http::StatusCode,
};
use reqwest::Client;

async fn create_subaccount_with_twilio(
    main_account_sid: &str,
    main_auth_token: &str,
    friendly_name: &str,
    client: &Client,
) -> Result<(String, String), (StatusCode, Json<serde_json::Value>)> {
    let form_params = [("FriendlyName", friendly_name)];

    let response = client
        .post("https://api.twilio.com/2010-04-01/Accounts.json")
        .basic_auth(main_account_sid, Some(main_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&form_params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create Twilio subaccount: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create subaccount"})),
            )
        })?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!("Twilio API error: {}", error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Twilio API error", "details": error_body})),
        ));
    }

    let json: Value = response.json().await.map_err(|e| {
        tracing::error!("Failed to parse Twilio response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to parse response"})),
        )
    })?;

    let subaccount_sid = json["sid"]
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid response: missing sid"})),
        ))?
        .to_string();
    let subaccount_auth_token = json["auth_token"]
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid response: missing auth_token"})),
        ))?
        .to_string();

    Ok((subaccount_sid, subaccount_auth_token))
}

async fn create_usage_trigger(
    subaccount_sid: &str,
    subaccount_auth_token: &str,
    limit: f32,
    client: &Client,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let callback_url = std::env::var("TWILIO_USAGE_TRIGGER_CALLBACK_URL")
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Missing TWILIO_USAGE_TRIGGER_CALLBACK_URL"})),
            )
        })?;

    let friendly_name = format!("Monthly spend limit ${}", limit);
    let trigger_value = limit.to_string();
    let form_params = [
        ("CallbackUrl", callback_url.as_str()),
        ("TriggerValue", trigger_value.as_str()),
        ("UsageCategory", "totalprice"),
        ("Recurring", "monthly"),
        ("TriggerBy", "price"),
        ("CallbackMethod", "POST"),
        ("FriendlyName", friendly_name.as_str()),
    ];

    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Usage/Triggers.json",
        subaccount_sid
    );

    let response = client
        .post(&url)
        .basic_auth(subaccount_sid, Some(subaccount_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&form_params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create usage trigger: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create usage trigger"})),
            )
        })?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!("Twilio trigger error: {}", error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Twilio trigger error", "details": error_body})),
        ));
    }

    Ok(())
}

async fn buy_phone_number(
    main_account_sid: &str,
    main_auth_token: &str,
    iso_country: &str,
    client: &Client,
) -> Result<(String, String), (StatusCode, Json<serde_json::Value>)> {
    let available_type = if iso_country == "US" { "TollFree" } else { "Mobile" };
    let available_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/{}.json?Limit=1",
        main_account_sid, iso_country, available_type
    );
    let available_resp = client
        .get(&available_url)
        .basic_auth(main_account_sid, Some(main_auth_token))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to list available numbers: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to find available number"})),
            )
        })?;

    if !available_resp.status().is_success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to list available numbers"})),
        ));
    }

    let available_json: Value = available_resp.json().await.map_err(|e| {
        tracing::error!("Failed to parse available numbers: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to parse available numbers"})),
        )
    })?;

    let available_numbers = available_json["available_phone_numbers"]
        .as_array()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "No available phone numbers"})),
        ))?;

    if available_numbers.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "No available phone numbers in this country"})),
        ));
    }

    let phone_number = available_numbers[0]["phone_number"]
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid available number format"})),
        ))?
        .to_string();

    // Buy the number in main account
    let mut buy_params = vec![("PhoneNumber", phone_number.as_str())];

    // For UK, add regulatory bundle
    let mut bundle_sid: Option<String> = None;
    if iso_country == "UK" {
        bundle_sid = Some(std::env::var("UK_REGULATORY_BUNDLE_SID").expect("UK_REGULATORY_BUNDLE_SID not set"));
    } else if iso_country == "AU" {
        bundle_sid = Some(std::env::var("AU_REGULATORY_BUNDLE_SID").expect("AU_REGULATORY_BUNDLE_SID not set"));
    }
    if let Some(ref sid) = bundle_sid {
        let sid_str = sid.as_str();
        buy_params.push(("BundleSid", sid_str));
    }

    let buy_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers.json",
        main_account_sid
    );

    let buy_response = client
        .post(&buy_url)
        .basic_auth(main_account_sid, Some(main_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&buy_params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to buy phone number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to purchase number"})),
            )
        })?;

    if !buy_response.status().is_success() {
        let error_body = buy_response.text().await.unwrap_or_default();
        tracing::error!("Twilio buy error: {}", error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to buy number", "details": error_body})),
        ));
    }

    let buy_json: Value = buy_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse buy response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to parse buy response"})),
        )
    })?;

    let pn_sid = buy_json["sid"]
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid buy response: missing sid"})),
        ))?
        .to_string();

    Ok((phone_number, pn_sid))
}

async fn transfer_phone_number(
    main_account_sid: &str,
    main_auth_token: &str,
    subaccount_sid: &str,
    pn_sid: &str,
    client: &Client,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let transfer_params = [("AccountSid", subaccount_sid)];

    let transfer_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers/{}.json",
        main_account_sid, pn_sid
    );

    let transfer_response = client
        .post(&transfer_url)
        .basic_auth(main_account_sid, Some(main_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&transfer_params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to transfer phone number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to transfer number"})),
            )
        })?;

    if !transfer_response.status().is_success() {
        let error_body = transfer_response.text().await.unwrap_or_default();
        tracing::error!("Twilio transfer error: {}", error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to transfer number", "details": error_body})),
        ));
    }

    Ok(())
}

// Updated main handler (now orchestrates the split functions)
pub async fn create_twilio_subaccount(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {

    // Fetch user to get country and credits_left (user_id as i32)
    let user = state.user_core
        .find_by_id(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to fetch user"}))))?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))))?;

    let country = user.phone_number_country.ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "User country not set"})),
    ))?;

    if user.credits_left <= 0.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Insufficient credits to create subaccount"})),
        ));
    }

    let client = Client::new();

    // Check for free subaccount matching country
    let free_sub_opt = state.user_core
        .find_free_subaccount_by_country(&country)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to query free subaccounts"}))))?;

    if let Some(free_sub) = free_sub_opt {
        // Assign existing subaccount (number already set in free sub)
        if let Err(e) = state.user_core.assign_subaccount_to_user(
            free_sub.id,
            &auth_user.user_id.to_string(), 
            &free_sub.number.unwrap_or_default(),
            &country,
            0.0,
        ) {
            tracing::error!("Failed to assign subaccount {} to user {}: {}", free_sub.id, auth_user.user_id, e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to assign subaccount"})),
            ));
        }

        // Set usage trigger for the assigned free subaccount
        if let Err(e) = create_usage_trigger(
            &free_sub.subaccount_sid,
            &free_sub.auth_token,
            user.credits_left,
            &client,
        ).await {
            tracing::error!("Failed to create usage trigger for free subaccount {}: {:?}", free_sub.subaccount_sid, e);
            // Don't fail the whole operation; log and continue
        }

        tracing::info!("Assigned free subaccount {} to user: {}", free_sub.subaccount_sid, auth_user.user_id);
        return Ok(StatusCode::OK);
    }

    // No free subaccount, create new one
    let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_ACCOUNT_SID"}))))?;
    let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_AUTH_TOKEN"}))))?;

    let friendly_name = auth_user.user_id.to_string().clone();

    // Step 1: Create subaccount
    let (subaccount_sid, subaccount_auth_token) = create_subaccount_with_twilio(
        &main_account_sid,
        &main_auth_token,
        &friendly_name,
        &client,
    )
    .await?;

    // Step 1.5: Set usage trigger
    if let Err(e) = create_usage_trigger(
        &subaccount_sid,
        &subaccount_auth_token,
        user.credits_left,
        &client,
    ).await {
        tracing::error!("Failed to create usage trigger for new subaccount {}: {:?}", subaccount_sid, e);
        // Don't fail the whole operation; log and continue (or handle as needed)
    }

    let created_at = Some(chrono::Utc::now().timestamp() as i32);

    // Step 2: Buy phone number
    let (phone_number, pn_sid) = buy_phone_number(
        &main_account_sid,
        &main_auth_token,
        &country.as_str(),
        &client,
    )
    .await?;

    // Step 3: Transfer phone number
    transfer_phone_number(
        &main_account_sid,
        &main_auth_token,
        &subaccount_sid,
        &pn_sid,
        &client,
    )
    .await?;

    let tinfoil_key = crate::handlers::self_host_handlers::create_temp_tinfoil_api_key(auth_user.user_id.to_string(), 1).await.unwrap();

    // Step 4: Insert into DB
    let new_subaccount = NewSubaccount {
        user_id: auth_user.user_id.to_string(),
        subaccount_sid,
        auth_token: subaccount_auth_token,
        country: Some(country),
        number: Some(phone_number.clone()),
        cost_this_month: Some(0.0),
        created_at,
        status: Some("active".to_string()),
        tinfoil_key: Some(tinfoil_key),
    };
    if let Err(e) = state.user_core.insert_subaccount(&new_subaccount) {
        tracing::error!("Failed to insert subaccount into DB: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to save subaccount"})),
        ));
    }

    tracing::info!(
        "Successfully created subaccount, bought/transferred number {} for user: {}",
        phone_number,
        auth_user.user_id
    );
    Ok(StatusCode::CREATED)
}

pub async fn suspend_subaccount(
    subaccount_sid: &str,
    main_account_sid: &str,
    main_auth_token: &str,
    client: &Client,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let form_params = [("Status", "suspended")];

    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}.json",
        subaccount_sid
    );

    let response = client
        .post(&url)
        .basic_auth(main_account_sid, Some(main_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&form_params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to suspend subaccount {}: {}", subaccount_sid, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to suspend subaccount"})),
            )
        })?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!("Twilio suspend error for {}: {}", subaccount_sid, error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Twilio suspend error", "details": error_body})),
        ));
    }

    tracing::info!("Suspended subaccount: {}", subaccount_sid);
    Ok(())
}

pub async fn revoke_subaccount(
    State(state): State<Arc<AppState>>,
    subaccount_id: i32,  // Assuming route param or body with id
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    // Fetch subaccount from DB
    let sub_opt = state.user_core
        .find_subaccount_by_id(subaccount_id)  // Assume this method exists
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to fetch subaccount"}))))?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"error": "Subaccount not found"}))))?;

    let subaccount_sid = &sub_opt.subaccount_sid;
    let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_ACCOUNT_SID"}))))?;
    let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_AUTH_TOKEN"}))))?;

    let client = Client::new();

    // Generate new auth token (regenerate)
    let new_auth_token = regenerate_subaccount_auth_token(
        subaccount_sid,
        &main_account_sid,
        &main_auth_token,
        &client,
    ).await
    .map_err(|e| {
        tracing::error!("Failed to regenerate auth token: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to regenerate auth token"})),
        )
    })?;

    // Update DB: set user_id to "-1", auth_token to new one, status maybe to "available"
    if let Err(e) = state.user_core.update_subaccount_user_and_token(
        subaccount_id,
        "-1".to_string(),
        new_auth_token,
        "available",  // Assuming status update
    ) {  // Assume this method: updates user_id, auth_token, status
        tracing::error!("Failed to update DB for revoke: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to update subaccount in DB"})),
        ));
    }

    tracing::info!("Revoked subaccount {} (user_id set to -1, new token generated)", subaccount_id);
    Ok(StatusCode::OK)
}


async fn regenerate_subaccount_auth_token(
    subaccount_sid: &str,
    main_account_sid: &str,
    main_auth_token: &str,
    client: &Client,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/RegenerateAuthToken.json",
        subaccount_sid
    );

    let response = client
        .post(&url)
        .basic_auth(main_account_sid, Some(main_auth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to regenerate auth token for {}: {}", subaccount_sid, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to regenerate auth token"})),
            )
        })?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!("Twilio regenerate error for {}: {}", subaccount_sid, error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Twilio regenerate error", "details": error_body})),
        ));
    }

    let json: Value = response.json().await.map_err(|e| {
        tracing::error!("Failed to parse regenerate response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to parse regenerate response"})),
        )
    })?;

    let new_token = json["auth_token"]
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid response: missing auth_token"})),
        ))?
        .to_string();

    Ok(new_token)
}
