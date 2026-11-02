//! Value-level projection hooks.
//!
//! Some canonical values cannot be represented in an older layer at all (not
//! merely with fewer fields). Encoding such a value must project it *away*
//! rather than encode a lossy approximation, so the hook returns `None` to drop
//! the value and the generated caller skips it.

use rtd_tg::BotCommand;

use crate::error::Result;

/// Canonical to profile for `botCommand`: an ephemeral command does not exist
/// before layer 228 and is projected away.
///
/// # Errors
///
/// Currently infallible; the signature is fallible for uniformity.
pub fn project_ephemeral_bot_command(value: &BotCommand) -> Result<Option<BotCommand>> {
    if value.ephemeral {
        Ok(None)
    } else {
        Ok(Some(value.clone()))
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::BotCommand;

    use super::project_ephemeral_bot_command;

    fn command(ephemeral: bool) -> BotCommand {
        BotCommand {
            flags: 0,
            ephemeral,
            command: "start".to_owned(),
            description: "Start".to_owned(),
        }
    }

    #[test]
    fn a_plain_command_survives() {
        let value = command(false);
        assert_eq!(project_ephemeral_bot_command(&value).unwrap(), Some(value));
    }

    #[test]
    fn an_ephemeral_command_is_projected_away() {
        assert!(
            project_ephemeral_bot_command(&command(true))
                .unwrap()
                .is_none()
        );
    }
}
