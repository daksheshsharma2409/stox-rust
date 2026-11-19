use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    config::AppConfig,
    error::AppError,
    middleware::auth::Claims,
    models::{SanitizedUser, User},
};

const DUMMY_PASSWORD_HASH: &str = "$2a$10$vI8aWBnW3fID.ZQ4/zo1G.q1lRps.9cGLcZEiGPEWJqNd6cePr.eq";

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn validate_password_strength(password: &str) -> Result<(), AppError> {
    let mut errors = Vec::new();

    if password.len() < 6 {
        errors.push("Password must be at least 6 characters long.".to_string());
    }

    let mut has_lowercase = false;
    let mut has_uppercase = false;
    let mut has_number = false;
    let mut has_symbol = false;

    for ch in password.chars() {
        if ch.is_ascii_lowercase() {
            has_lowercase = true;
        } else if ch.is_ascii_uppercase() {
            has_uppercase = true;
        } else if ch.is_ascii_digit() {
            has_number = true;
        } else if !ch.is_whitespace() {
            has_symbol = true;
        }
    }

    if !has_lowercase {
        errors.push("Password must contain at least one lowercase letter.".to_string());
    }
    if !has_uppercase {
        errors.push("Password must contain at least one uppercase letter.".to_string());
    }
    if !has_number {
        errors.push("Password must contain at least one number.".to_string());
    }
    if !has_symbol {
        errors.push("Password must contain at least one symbol.".to_string());
    }
    if password.len() > 72 {
        errors.push("Password must not exceed 72 bytes.".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::bad_request_with_details(
            errors.join(" "),
            "WEAK_PASSWORD",
            serde_json::to_value(errors).unwrap(),
        ));
    }

    Ok(())
}

pub fn hash_password(password: &str, cost: u32) -> Result<String, AppError> {
    bcrypt::hash(password, cost)
        .map_err(|_| AppError::internal("Failed to hash password."))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn parse_duration(val: &str) -> Duration {
    let len = val.len();
    if len < 2 {
        return Duration::days(7);
    }
    let (num_part, unit) = val.split_at(len - 1);
    let amount: i64 = num_part.parse().unwrap_or(7);
    match unit {
        "s" => Duration::seconds(amount),
        "m" => Duration::minutes(amount),
        "h" => Duration::hours(amount),
        "d" => Duration::days(amount),
        _ => Duration::days(amount),
    }
}

pub fn create_access_token(user: &User, config: &AppConfig) -> Result<String, AppError> {
    let duration = parse_duration(&config.jwt_access_expiry);
    let exp = (Utc::now() + duration).timestamp() as usize;

    let claims = Claims {
        user_id: user.id.clone(),
        role: user.role.as_str().to_string(),
        email: user.email.clone(),
        iss: Some("stox".to_string()),
        sub: Some(user.id.clone()),
        exp: Some(exp),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_access_secret.as_bytes()),
    )
    .map_err(|_| AppError::internal("Failed to create access token."))
}

pub fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    BASE64URL.encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponseData {
    pub user: SanitizedUser,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
}

pub async fn issue_token_pair_tx(
    tx: &mut Transaction<'_, Postgres>,
    user: &User,
    config: &AppConfig,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<(String, String, DateTime<Utc>), AppError> {
    let access_token = create_access_token(user, config)?;
    let raw_refresh_token = generate_refresh_token();
    let hashed_refresh_token = hash_refresh_token(&raw_refresh_token);

    let refresh_duration = parse_duration(&config.jwt_refresh_expiry);
    let expires_at = Utc::now() + refresh_duration;
    let token_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO "RefreshToken" ("id", "token", "userId", "expiresAt", "userAgent", "ipAddress", "createdAt")
        VALUES ($1, $2, $3, $4, $5, $6, now())
        "#
    )
    .bind(token_id)
    .bind(hashed_refresh_token)
    .bind(&user.id)
    .bind(expires_at.naive_utc())
    .bind(user_agent)
    .bind(ip_address)
    .execute(&mut **tx)
    .await?;

    Ok((access_token, raw_refresh_token, expires_at))
}

pub async fn register_user(
    pool: &PgPool,
    config: &AppConfig,
    name: &str,
    email: &str,
    password: &str,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<AuthResponseData, AppError> {
    let normalized = normalize_email(email);
    validate_password_strength(password)?;

    let existing: Option<(String,)> = sqlx::query_as(
        r#"SELECT "id" FROM "User" WHERE "email" = $1 LIMIT 1"#
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::conflict(
            "An account with this email already exists.",
            "EMAIL_ALREADY_EXISTS",
        ));
    }

    let password_hash = hash_password(password, config.bcrypt_salt_rounds)?;
    let mut tx = pool.begin().await?;

    let user_id = Uuid::new_v4().to_string();
    let user: User = sqlx::query_as(
        r#"
        INSERT INTO "User" ("id", "name", "email", "passwordHash", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "createdAt", "updatedAt")
        VALUES ($1, $2, $3, $4, 'TRADER'::"Role", false, false, true, true, false, now(), now())
        RETURNING "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        "#
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(&normalized)
    .bind(password_hash)
    .fetch_one(&mut *tx)
    .await?;

    // Create default Main Portfolio
    let portfolio_id = Uuid::new_v4().to_string();
    let starting_balance = rust_decimal_macros::dec!(10000);
    sqlx::query(
        r#"
        INSERT INTO "Portfolio" ("id", "userId", "name", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt")
        VALUES ($1, $2, 'Main Portfolio', $3, $3, 0, true, true, now(), now())
        "#
    )
    .bind(portfolio_id)
    .bind(&user.id)
    .bind(starting_balance)
    .execute(&mut *tx)
    .await?;

    let (access_token, refresh_token, expires_at) = issue_token_pair_tx(
        &mut tx,
        &user,
        config,
        user_agent,
        ip_address,
    )
    .await?;

    tx.commit().await?;

    Ok(AuthResponseData {
        user: user.into(),
        access_token,
        refresh_token,
        expires_at,
    })
}

pub async fn login_user(
    pool: &PgPool,
    config: &AppConfig,
    email: &str,
    password: &str,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<AuthResponseData, AppError> {
    let normalized = normalize_email(email);

    let user: Option<User> = sqlx::query_as(
        r#"
        SELECT "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        FROM "User"
        WHERE "email" = $1
        LIMIT 1
        "#
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await?;

    let target_hash = user
        .as_ref()
        .and_then(|u| u.password_hash.as_deref())
        .unwrap_or(DUMMY_PASSWORD_HASH);

    let matches = verify_password(password, target_hash);

    let user = match user {
        Some(u) if u.password_hash.is_some() && matches => u,
        _ => {
            return Err(AppError::unauthorized(
                "Invalid email or password.",
                "INVALID_CREDENTIALS",
            ))
        }
    };

    if user.is_suspended {
        return Err(AppError::forbidden(
            "This account has been suspended.",
            "ACCOUNT_SUSPENDED",
        ));
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        UPDATE "User"
        SET "lastLoginAt" = now(), "updatedAt" = now()
        WHERE "id" = $1
        "#
    )
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;

    let (access_token, refresh_token, expires_at) = issue_token_pair_tx(
        &mut tx,
        &user,
        config,
        user_agent,
        ip_address,
    )
    .await?;

    tx.commit().await?;

    Ok(AuthResponseData {
        user: user.into(),
        access_token,
        refresh_token,
        expires_at,
    })
}

pub async fn refresh_access_token(
    pool: &PgPool,
    config: &AppConfig,
    raw_refresh_token: &str,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<AuthResponseData, AppError> {
    if raw_refresh_token.trim().is_empty() {
        return Err(AppError::unauthorized(
            "Refresh token is required.",
            "REFRESH_TOKEN_REQUIRED",
        ));
    }

    let hashed = hash_refresh_token(raw_refresh_token);
    let mut tx = pool.begin().await?;

    let record: Option<(String, String, chrono::NaiveDateTime, Option<chrono::NaiveDateTime>)> = sqlx::query_as(
        r#"
        SELECT "id", "userId", "expiresAt", "revokedAt"
        FROM "RefreshToken"
        WHERE "token" = $1
        FOR UPDATE
        "#
    )
    .bind(&hashed)
    .fetch_optional(&mut *tx)
    .await?;

    let (id, user_id, _expires_at_naive, _revoked_at) = match record {
        Some((id, user_id, exp, revoked)) if revoked.is_none() && exp > Utc::now().naive_utc() => {
            (id, user_id, exp, revoked)
        }
        _ => {
            return Err(AppError::unauthorized(
                "Invalid or expired refresh token.",
                "INVALID_REFRESH_TOKEN",
            ))
        }
    };

    // Revoke old token
    sqlx::query(
        r#"
        UPDATE "RefreshToken"
        SET "revokedAt" = now()
        WHERE "id" = $1
        "#
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let user: User = sqlx::query_as(
        r#"
        SELECT "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        FROM "User"
        WHERE "id" = $1
        LIMIT 1
        "#
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::unauthorized("User not found.", "USER_NOT_FOUND"))?;

    if user.is_suspended {
        return Err(AppError::forbidden(
            "This account has been suspended.",
            "ACCOUNT_SUSPENDED",
        ));
    }

    let (access_token, new_refresh_token, expires_at) = issue_token_pair_tx(
        &mut tx,
        &user,
        config,
        user_agent,
        ip_address,
    )
    .await?;

    tx.commit().await?;

    Ok(AuthResponseData {
        user: user.into(),
        access_token,
        refresh_token: new_refresh_token,
        expires_at,
    })
}

pub async fn revoke_refresh_token(pool: &PgPool, raw_refresh_token: &str) -> bool {
    let hashed = hash_refresh_token(raw_refresh_token);
    let result = sqlx::query(
        r#"
        UPDATE "RefreshToken"
        SET "revokedAt" = now()
        WHERE "token" = $1 AND "revokedAt" IS NULL
        "#
    )
    .bind(hashed)
    .execute(pool)
    .await;

    match result {
        Ok(res) => res.rows_affected() > 0,
        Err(_) => false,
    }
}

pub async fn get_current_user(pool: &PgPool, user_id: &str) -> Result<SanitizedUser, AppError> {
    let user: User = sqlx::query_as(
        r#"
        SELECT "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        FROM "User"
        WHERE "id" = $1
        LIMIT 1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("User not found.", "USER_NOT_FOUND"))?;

    if user.is_suspended {
        return Err(AppError::forbidden(
            "Account is suspended.",
            "ACCOUNT_SUSPENDED",
        ));
    }

    Ok(user.into())
}

pub async fn update_profile(
    pool: &PgPool,
    user_id: &str,
    name: Option<String>,
    avatar_url: Option<String>,
) -> Result<SanitizedUser, AppError> {
    let user = get_current_user(pool, user_id).await?;

    let updated: User = sqlx::query_as(
        r#"
        UPDATE "User"
        SET "name" = COALESCE($2, "name"),
            "avatarUrl" = COALESCE($3, "avatarUrl"),
            "updatedAt" = now()
        WHERE "id" = $1
        RETURNING "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        "#
    )
    .bind(&user.id)
    .bind(name.map(|n| n.trim().to_string()))
    .bind(avatar_url)
    .fetch_one(pool)
    .await?;

    Ok(updated.into())
}

#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    sub: Option<String>,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
    email_verified: Option<String>,
}

pub async fn login_with_google(
    pool: &PgPool,
    config: &AppConfig,
    client: &reqwest::Client,
    credential: &str,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<AuthResponseData, AppError> {
    if credential.trim().is_empty() {
        return Err(AppError::bad_request(
            "Google credential is required.",
            "GOOGLE_CREDENTIAL_REQUIRED",
        ));
    }

    let url = format!(
        "https://oauth2.googleapis.com/tokeninfo?id_token={}",
        credential
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|_| AppError::unauthorized("Invalid Google credential.", "INVALID_GOOGLE_TOKEN"))?;

    if !resp.status().is_success() {
        return Err(AppError::unauthorized(
            "Invalid Google credential.",
            "INVALID_GOOGLE_TOKEN",
        ));
    }

    let token_info: GoogleTokenInfo = resp
        .json()
        .await
        .map_err(|_| AppError::unauthorized("Invalid Google account payload.", "INVALID_GOOGLE_TOKEN"))?;

    let google_id = token_info.sub.ok_or_else(|| {
        AppError::unauthorized(
            "Google account information is incomplete.",
            "INVALID_GOOGLE_ACCOUNT",
        )
    })?;

    let email = token_info.email.ok_or_else(|| {
        AppError::unauthorized(
            "Google account information is incomplete.",
            "INVALID_GOOGLE_ACCOUNT",
        )
    })?;

    let normalized = normalize_email(&email);

    // Existing Google account
    let existing_google_user: Option<User> = sqlx::query_as(
        r#"
        SELECT "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        FROM "User"
        WHERE "googleId" = $1
        LIMIT 1
        "#
    )
    .bind(&google_id)
    .fetch_optional(pool)
    .await?;

    if let Some(user) = existing_google_user {
        if user.is_suspended {
            return Err(AppError::forbidden(
                "Account is suspended.",
                "ACCOUNT_SUSPENDED",
            ));
        }

        let mut tx = pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE "User"
            SET "lastLoginAt" = now(), "updatedAt" = now()
            WHERE "id" = $1
            "#
        )
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;

        let (access_token, refresh_token, expires_at) = issue_token_pair_tx(
            &mut tx,
            &user,
            config,
            user_agent,
            ip_address,
        )
        .await?;

        tx.commit().await?;

        return Ok(AuthResponseData {
            user: user.into(),
            access_token,
            refresh_token,
            expires_at,
        });
    }

    // Existing email account with password
    let existing_email_user: Option<(String,)> = sqlx::query_as(
        r#"SELECT "id" FROM "User" WHERE "email" = $1 LIMIT 1"#
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await?;

    if existing_email_user.is_some() {
        return Err(AppError::conflict(
            "An account already exists with this email. Sign in with your password before linking Google.",
            "GOOGLE_ACCOUNT_LINK_REQUIRED",
        ));
    }

    // New Google account
    let mut tx = pool.begin().await?;
    let new_user_id = Uuid::new_v4().to_string();
    let is_verified = token_info.email_verified.map(|v| v == "true").unwrap_or(false);

    let user: User = sqlx::query_as(
        r#"
        INSERT INTO "User" ("id", "name", "email", "googleId", "avatarUrl", "isVerified", "isSuspended", "role", "emailAlerts", "emailDigest", "pushEnabled", "createdAt", "updatedAt", "lastLoginAt")
        VALUES ($1, $2, $3, $4, $5, $6, false, 'TRADER'::"Role", true, true, false, now(), now(), now())
        RETURNING "id", "email", "passwordHash", "googleId", "name", "avatarUrl", "role", "isVerified", "isSuspended", "emailAlerts", "emailDigest", "pushEnabled", "pushToken", "createdAt", "updatedAt", "lastLoginAt"
        "#
    )
    .bind(new_user_id)
    .bind(token_info.name.unwrap_or_else(|| "Stox User".to_string()))
    .bind(normalized)
    .bind(google_id)
    .bind(token_info.picture)
    .bind(is_verified)
    .fetch_one(&mut *tx)
    .await?;

    // Create Main Portfolio
    let portfolio_id = Uuid::new_v4().to_string();
    let starting_balance = rust_decimal_macros::dec!(10000);
    sqlx::query(
        r#"
        INSERT INTO "Portfolio" ("id", "userId", "name", "startingBalance", "cashBalance", "reservedCash", "isDefault", "isActive", "createdAt", "updatedAt")
        VALUES ($1, $2, 'Main Portfolio', $3, $3, 0, true, true, now(), now())
        "#
    )
    .bind(portfolio_id)
    .bind(&user.id)
    .bind(starting_balance)
    .execute(&mut *tx)
    .await?;

    let (access_token, refresh_token, expires_at) = issue_token_pair_tx(
        &mut tx,
        &user,
        config,
        user_agent,
        ip_address,
    )
    .await?;

    tx.commit().await?;

    Ok(AuthResponseData {
        user: user.into(),
        access_token,
        refresh_token,
        expires_at,
    })
}
