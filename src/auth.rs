use std::sync::Arc;

use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    models::{ApiMessage, AppState, UserCredentials},
};

pub async fn register(
    state: &Arc<AppState>,
    credentials: UserCredentials,
) -> AppResult<ApiMessage<String>> {
    validate_credentials(&credentials)?;

    if let Some(pool) = &state.databases.users {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT phone_number FROM alluser WHERE phone_number = ? LIMIT 1")
                .bind(&credentials.phone_number)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            return Ok(ApiMessage {
                state: 400,
                message: "您已经注册账号。".into(),
            });
        }
        sqlx::query("INSERT INTO alluser(phone_number, password) VALUES (?, ?)")
            .bind(&credentials.phone_number)
            .bind(&credentials.password)
            .execute(pool)
            .await?;
    } else {
        let mut users = state.memory_users.write().await;
        if users.contains_key(&credentials.phone_number) {
            return Ok(ApiMessage {
                state: 400,
                message: "您已经注册账号。".into(),
            });
        }
        users.insert(credentials.phone_number, credentials.password);
    }

    Ok(ApiMessage {
        state: 200,
        message: "成功注册账号。".into(),
    })
}

pub async fn login(
    state: &Arc<AppState>,
    credentials: UserCredentials,
) -> AppResult<ApiMessage<String>> {
    validate_credentials(&credentials)?;
    let password = if let Some(pool) = &state.databases.users {
        sqlx::query("SELECT password FROM alluser WHERE phone_number = ? LIMIT 1")
            .bind(&credentials.phone_number)
            .fetch_optional(pool)
            .await?
            .and_then(|row| row.try_get::<String, _>("password").ok())
    } else {
        state
            .memory_users
            .read()
            .await
            .get(&credentials.phone_number)
            .cloned()
    };

    let (state_code, message) = match password {
        None => (400, "请先注册账号。"),
        Some(saved) if saved != credentials.password => (400, "密码错误，请重试。"),
        Some(_) => (200, "密码正确。"),
    };
    Ok(ApiMessage {
        state: state_code,
        message: message.into(),
    })
}

pub async fn list_users(state: &Arc<AppState>, credentials: UserCredentials) -> AppResult<Value> {
    if credentials.phone_number != state.config.admin_phone
        || credentials.password != state.config.admin_password
    {
        return Ok(json!({"state": 400, "message": "您没有权限。"}));
    }

    let users: Vec<Vec<String>> = if let Some(pool) = &state.databases.users {
        sqlx::query("SELECT phone_number, password FROM alluser ORDER BY phone_number")
            .fetch_all(pool)
            .await?
            .into_iter()
            .filter_map(|row| {
                Some(vec![
                    row.try_get::<String, _>("phone_number").ok()?,
                    row.try_get::<String, _>("password").ok()?,
                ])
            })
            .collect()
    } else {
        let mut users: Vec<_> = state
            .memory_users
            .read()
            .await
            .iter()
            .map(|(phone, password)| vec![phone.clone(), password.clone()])
            .collect();
        users.sort();
        users
    };
    Ok(json!({"state": 200, "message": users}))
}

fn validate_credentials(credentials: &UserCredentials) -> AppResult<()> {
    if credentials.phone_number.len() != 11
        || !credentials.phone_number.chars().all(|c| c.is_ascii_digit())
    {
        return Err(AppError::Validation("手机号必须为 11 位数字".into()));
    }
    if credentials.password.is_empty() {
        return Err(AppError::Validation("密码不能为空".into()));
    }
    if credentials.password.len() > 72 {
        return Err(AppError::Validation("密码长度不能超过 72 个字符".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_phone() {
        let input = UserCredentials {
            phone_number: "123".into(),
            password: "secret".into(),
        };
        assert!(validate_credentials(&input).is_err());
    }

    #[test]
    fn accepts_legacy_password_shape() {
        let input = UserCredentials {
            phone_number: "13800138000".into(),
            password: "1103".into(),
        };
        assert!(validate_credentials(&input).is_ok());
    }
}
