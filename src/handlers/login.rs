use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use bcrypt::verify;
use validator::Validate;

use crate::auth::{create_session, SESSION_COOKIE};
use crate::error::{AppError, Result};
use crate::models::LoginForm;
use crate::state::AppState;
use crate::templates::{LoginErrors, LoginPage};

pub async fn get_login() -> LoginPage {
    LoginPage::new(
        String::new(),
        LoginErrors::default(),
        String::new(),
    )
}

pub async fn post_login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Response> {
    if let Err(e) = form.validate() {
        let mut errors = LoginErrors::default();
        for (field, err) in e.field_errors() {
            let msg = err
                .first()
                .and_then(|m| m.message.as_ref())
                .map(|m| m.to_string())
                .unwrap_or_else(|| "Invalid".to_string());
            match field.as_ref() {
                "username" => errors.username = msg,
                "password" => errors.password = msg,
                _ => {}
            }
        }
        return Ok(LoginPage::new(form.username, errors, String::new()).into_response());
    }

    let user = sqlx::query_as::<_, (i32, String)>(
        "SELECT id, password_hash FROM users WHERE username = $1",
    )
    .bind(&form.username)
    .fetch_optional(&state.pool)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            return Ok(LoginPage::new(
                form.username,
                LoginErrors::default(),
                "Invalid username or password.".to_string(),
            )
            .into_response());
        }
    };

    let valid = verify(&form.password, &user.1).map_err(|_| AppError::Internal(anyhow::anyhow!("bcrypt error")))?;
    if !valid {
        return Ok(LoginPage::new(
            form.username,
            LoginErrors::default(),
            "Invalid username or password.".to_string(),
        )
        .into_response());
    }

    let token = create_session(&state.pool, user.0).await?;
    let cookie = Cookie::build((SESSION_COOKIE, token.clone()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(7))
        .build();

    let mut response = Redirect::to("/").into_response();
    response
        .headers_mut()
        .insert("Set-Cookie", cookie.to_string().parse().unwrap());
    Ok(response)
}

pub async fn logout(State(_state): State<AppState>) -> Result<Response> {
    // Session is deleted when cookie is cleared; optional: delete by token if we had it
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        "Set-Cookie",
        format!("{}=; Path=/; HttpOnly; Max-Age=0", SESSION_COOKIE)
            .parse()
            .unwrap(),
    );
    Ok(response)
}
