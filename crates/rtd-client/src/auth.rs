//! Password providers for two-factor authorization.

use zeroize::Zeroize;

use rtd_tg::{AuthAuthorization, AuthSentCode, CodeSettings};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// Supplies the account password when the server asks for it.
///
/// The client calls this only after `account.getPassword` reports an SRP
/// challenge, so a caller that has no password configured can return `None`.
pub trait PasswordProvider: Send {
    /// Returns the account password, or `None` if none is configured.
    fn password(&self) -> Option<Vec<u8>>;
}

/// A password held in memory and zeroized when dropped.
///
/// Mirrors `td`'s `srpguard`: the plaintext password never lives in a plain
/// `Vec` whose backing bytes could be left behind after the value is dropped.
pub struct HeldPassword(Vec<u8>);

impl HeldPassword {
    /// Wraps a password.
    #[must_use]
    pub fn new(password: impl Into<Vec<u8>>) -> Self {
        Self(password.into())
    }

    /// Returns the password bytes.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl PasswordProvider for HeldPassword {
    fn password(&self) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

impl Drop for HeldPassword {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl core::fmt::Debug for HeldPassword {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("HeldPassword(<redacted>)")
    }
}

/// The authorization status of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The session is authorized.
    Authorized,
    /// The session needs to log in.
    Unauthorized,
}

/// Drives a phone login: status, sending/resending the code, signing in (with
/// two-factor), and signing up.
#[derive(Debug, Clone)]
pub struct Flow {
    api_id: i32,
    api_hash: String,
    phone_number: String,
    first_name: String,
    last_name: String,
    settings: CodeSettings,
}

impl Flow {
    /// Creates a login flow for `phone_number`.
    #[must_use]
    pub fn new(
        api_id: i32,
        api_hash: impl Into<String>,
        phone_number: impl Into<String>,
        settings: CodeSettings,
    ) -> Self {
        Self {
            api_id,
            api_hash: api_hash.into(),
            phone_number: phone_number.into(),
            first_name: String::new(),
            last_name: String::new(),
            settings,
        }
    }

    /// Sets the first and last name used when signing up.
    #[must_use]
    pub fn with_name(
        mut self,
        first_name: impl Into<String>,
        last_name: impl Into<String>,
    ) -> Self {
        self.first_name = first_name.into();
        self.last_name = last_name.into();
        self
    }

    /// Returns the session's authorization status.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto or decode error. An RPC error other than the
    /// `401` unauthorized response is propagated.
    pub fn status<T: Transport>(&self, client: &mut Client<T>) -> Result<Status> {
        match client.self_user() {
            Ok(_) => Ok(Status::Authorized),
            Err(Error::Rpc(error)) if error.code() == 401 => Ok(Status::Unauthorized),
            Err(other) => Err(other),
        }
    }

    /// Sends a login code to the flow's phone number (`auth.sendCode`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error.
    pub fn send_code<T: Transport>(&self, client: &mut Client<T>) -> Result<AuthSentCode> {
        client.send_code(
            &self.phone_number,
            self.api_id,
            &self.api_hash,
            self.settings.clone(),
        )
    }

    /// Signs in with `code`, completing two-factor authorization through
    /// `passwords` when the server requires it.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when `sent` carries no code hash.
    pub fn sign_in<T: Transport>(
        &self,
        client: &mut Client<T>,
        sent: &AuthSentCode,
        code: &str,
        passwords: &dyn PasswordProvider,
    ) -> Result<AuthAuthorization> {
        let hash = code_hash(sent)?;
        match client.sign_in(&self.phone_number, hash, Some(code)) {
            Ok(authorization) => Ok(authorization),
            Err(Error::Rpc(error)) if error.is_type("SESSION_PASSWORD_NEEDED") => {
                client.check_password_with(passwords)
            }
            Err(other) => Err(other),
        }
    }

    /// Signs up a new account (`auth.signUp`).
    ///
    /// When the server requires the terms of service to be accepted and
    /// `accept_tos` is set, the flow accepts them and retries.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the sign-up still requires the terms of service and `accept_tos` is
    /// not set.
    pub fn sign_up<T: Transport>(
        &self,
        client: &mut Client<T>,
        sent: &AuthSentCode,
        accept_tos: bool,
    ) -> Result<AuthAuthorization> {
        let hash = code_hash(sent)?.to_owned();
        let first = client.sign_up(&self.phone_number, &hash, &self.first_name, &self.last_name)?;
        if matches!(&first, AuthAuthorization::Authorization { .. }) {
            return Ok(first);
        }
        let AuthAuthorization::AuthorizationSignUpRequired {
            terms_of_service, ..
        } = first
        else {
            unreachable!("only two auth.authorization constructors exist")
        };
        let Some(tos) = terms_of_service else {
            return Err(Error::Unexpected(
                "auth.signUp requires sign-up but returned no terms of service".to_owned(),
            ));
        };
        if !accept_tos {
            return Err(Error::Unexpected(
                "auth.signUp requires accepting the terms of service".to_owned(),
            ));
        }
        client.accept_terms_of_service(*tos.id)?;
        let retry = client.sign_up(&self.phone_number, &hash, &self.first_name, &self.last_name)?;
        match retry {
            AuthAuthorization::Authorization { .. } => Ok(retry),
            AuthAuthorization::AuthorizationSignUpRequired { .. } => Err(Error::Unexpected(
                "auth.signUp still requires sign-up after accepting the terms".to_owned(),
            )),
        }
    }
}

/// Returns code settings with no special options.
#[must_use]
pub fn default_code_settings() -> CodeSettings {
    CodeSettings {
        flags: 0,
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    }
}

/// Returns the phone code hash carried by a sent-code response.
fn code_hash(sent: &AuthSentCode) -> Result<&str> {
    match sent {
        AuthSentCode::SentCode {
            phone_code_hash, ..
        }
        | AuthSentCode::SentCodePaymentRequired {
            phone_code_hash, ..
        } => Ok(phone_code_hash),
        AuthSentCode::SentCodeSuccess { .. } => Err(Error::Unexpected(
            "auth.sendCode already returned an authorization".to_owned(),
        )),
    }
}

/// A [`PasswordProvider`] that always returns the same password.
#[derive(Debug, Clone)]
pub struct ConstantPassword(Vec<u8>);

impl ConstantPassword {
    /// Creates a provider for `password`.
    #[must_use]
    pub fn new(password: impl Into<Vec<u8>>) -> Self {
        Self(password.into())
    }
}

impl PasswordProvider for ConstantPassword {
    fn password(&self) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

/// A bot authorization token.
///
/// The token is held privately and its [`Debug`](core::fmt::Debug)
/// implementation is redacted, so it can never leak into logs.
#[derive(Clone, PartialEq, Eq)]
pub struct BotToken(String);

impl BotToken {
    /// Wraps a token literal.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Reads a token from the environment, checking `BOT_TOKEN` then
    /// `TELEGRAM_BOT_TOKEN`.
    ///
    /// Returns `None` when neither variable is set or a variable is empty.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        for key in ["BOT_TOKEN", "TELEGRAM_BOT_TOKEN"] {
            if let Ok(value) = std::env::var(key)
                && !value.is_empty()
            {
                return Some(Self(value));
            }
        }
        None
    }

    /// Returns the token string.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for BotToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("BotToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::{BotToken, HeldPassword, PasswordProvider};

    #[test]
    fn token_debug_is_redacted() {
        let token = BotToken::new("123456:secret");
        assert_eq!(token.expose(), "123456:secret");
        assert_eq!(format!("{token:?}"), "BotToken(<redacted>)");
    }

    #[test]
    fn held_password_is_redacted_and_available() {
        let held = HeldPassword::new("hunter2");
        assert_eq!(held.expose(), b"hunter2");
        assert_eq!(held.password().as_deref(), Some(b"hunter2".as_slice()));
        assert_eq!(format!("{held:?}"), "HeldPassword(<redacted>)");
    }
}
