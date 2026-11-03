//! Data-export (takeout) sessions.
//!
//! [`run`] initializes an `account.initTakeoutSession`, runs the caller's
//! closure with a helper that wraps each call in `invokeWithTakeout`, and
//! finishes the session even when the closure fails.

use rtd_tg::{
    AccountFinishTakeoutSession, AccountInitTakeoutSession, InvokeWithTakeout, Method, Object,
};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// Which categories a takeout session exports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TakeoutConfig {
    /// Export contacts.
    pub contacts: bool,
    /// Export messages with users.
    pub message_users: bool,
    /// Export messages in legacy chats.
    pub message_chats: bool,
    /// Export messages in megagroups.
    pub message_megagroups: bool,
    /// Export messages in channels.
    pub message_channels: bool,
    /// Export files.
    pub files: bool,
    /// Maximum file size to export, if bounded.
    pub file_max_size: Option<i64>,
}

impl TakeoutConfig {
    /// Returns a configuration exporting every category.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            contacts: true,
            message_users: true,
            message_chats: true,
            message_megagroups: true,
            message_channels: true,
            files: true,
            file_max_size: None,
        }
    }

    /// Returns the `initTakeoutSession` flag bits for this configuration.
    #[must_use]
    pub const fn flags(&self) -> u32 {
        let mut flags = 0u32;
        if self.contacts {
            flags |= 1 << 0;
        }
        if self.message_users {
            flags |= 1 << 1;
        }
        if self.message_chats {
            flags |= 1 << 2;
        }
        if self.message_megagroups {
            flags |= 1 << 3;
        }
        if self.message_channels {
            flags |= 1 << 4;
        }
        if self.files {
            flags |= 1 << 5;
        }
        flags
    }
}

/// Wraps `query` in `invokeWithTakeout` for `takeout_id`.
#[must_use]
pub fn wrap(query: Method, takeout_id: i64) -> Method {
    Method::InvokeWithTakeout(Box::new(InvokeWithTakeout {
        takeout_id,
        query: Box::new(query),
    }))
}

/// Runs `f` inside a takeout session, finishing the session before returning.
///
/// # Errors
///
/// Returns the closure's error when it fails, otherwise the error from starting
/// or finishing the session.
pub fn run<T: Transport, R>(
    client: &mut Client<T>,
    config: TakeoutConfig,
    f: impl FnOnce(&mut Client<T>, i64) -> Result<R>,
) -> Result<R> {
    let takeout_id = init(client, &config)?;
    let outcome = f(client, takeout_id);
    let finished = finish(client, takeout_id);
    match outcome {
        Ok(value) => {
            finished?;
            Ok(value)
        }
        Err(error) => {
            let _ = finished;
            Err(error)
        }
    }
}

/// Starts a takeout session and returns its id.
fn init<T: Transport>(client: &mut Client<T>, config: &TakeoutConfig) -> Result<i64> {
    let query = Method::AccountInitTakeoutSession(Box::new(AccountInitTakeoutSession {
        flags: config.flags(),
        contacts: config.contacts,
        message_users: config.message_users,
        message_chats: config.message_chats,
        message_megagroups: config.message_megagroups,
        message_channels: config.message_channels,
        files: config.files,
        file_max_size: config.file_max_size,
    }));
    match client.invoke(query)? {
        Object::AccountTakeout(takeout) => Ok(takeout.id),
        other => Err(Error::Unexpected(format!(
            "account.initTakeoutSession returned unexpected constructor {:#010x}",
            other.type_id()
        ))),
    }
}

/// Finishes a takeout session.
fn finish<T: Transport>(client: &mut Client<T>, _takeout_id: i64) -> Result<()> {
    let query = Method::AccountFinishTakeoutSession(Box::new(AccountFinishTakeoutSession {
        flags: 0,
        success: true,
    }));
    client.invoke(query)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TakeoutConfig;

    #[test]
    fn all_sets_every_flag() {
        assert_eq!(TakeoutConfig::all().flags(), 0b11_1111);
        assert_eq!(TakeoutConfig::default().flags(), 0);
    }
}
