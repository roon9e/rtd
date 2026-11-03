//! Channel member iteration and status.
//!
//! [`ChannelMembers`] wraps the participants iterator and maps each
//! [`ChannelParticipant`] to a [`Member`] with a coarse [`MemberStatus`].

use rtd_tg::{ChannelParticipant, InputChannel, Method, Object, Peer};

use crate::client::Client;
use crate::error::Result;
use crate::pagination::{ParticipantsIter, Query};
use crate::transport::Transport;

/// A member's status in a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatus {
    /// The channel's creator.
    Creator,
    /// An administrator.
    Administrator,
    /// A regular member (including the requesting user).
    Member,
    /// A member who left or was removed.
    Left,
    /// A restricted or banned member.
    Banned,
}

/// A channel member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member's user id, when it is a user.
    pub user_id: Option<i64>,
    /// The member's status.
    pub status: MemberStatus,
    /// The optional rank/title.
    pub rank: Option<String>,
}

impl Member {
    /// Returns whether the member is the creator.
    #[must_use]
    pub fn is_creator(&self) -> bool {
        matches!(self.status, MemberStatus::Creator)
    }

    /// Returns whether the member is an administrator or the creator.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(
            self.status,
            MemberStatus::Creator | MemberStatus::Administrator
        )
    }
}

/// Maps a raw participant to a [`Member`].
#[must_use]
pub fn member_from_participant(participant: &ChannelParticipant) -> Member {
    match participant {
        ChannelParticipant::ChannelParticipant { user_id, rank, .. }
        | ChannelParticipant::ChannelParticipantSelf { user_id, rank, .. } => Member {
            user_id: Some(*user_id),
            status: MemberStatus::Member,
            rank: rank.clone(),
        },
        ChannelParticipant::ChannelParticipantAdmin { user_id, rank, .. } => Member {
            user_id: Some(*user_id),
            status: MemberStatus::Administrator,
            rank: rank.clone(),
        },
        ChannelParticipant::ChannelParticipantCreator { user_id, rank, .. } => Member {
            user_id: Some(*user_id),
            status: MemberStatus::Creator,
            rank: rank.clone(),
        },
        ChannelParticipant::ChannelParticipantBanned { left, peer, .. } => Member {
            user_id: peer_user_id(peer),
            status: if *left {
                MemberStatus::Left
            } else {
                MemberStatus::Banned
            },
            rank: None,
        },
        ChannelParticipant::ChannelParticipantLeft { peer } => Member {
            user_id: peer_user_id(peer),
            status: MemberStatus::Left,
            rank: None,
        },
    }
}

/// Extracts a user id from a peer, if it references a user.
fn peer_user_id(peer: &Peer) -> Option<i64> {
    match peer {
        Peer::PeerUser { user_id } => Some(*user_id),
        _ => None,
    }
}

/// An iterator over a channel's members.
#[derive(Debug, Clone)]
pub struct ChannelMembers {
    inner: ParticipantsIter,
}

impl ChannelMembers {
    /// Creates an iterator over all of `channel`'s members.
    #[must_use]
    pub fn new(channel: InputChannel) -> Self {
        Self {
            inner: ParticipantsIter::new(channel),
        }
    }

    /// Bounds the total number of members returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.inner = self.inner.with_limit(limit);
        self
    }
}

impl Query for ChannelMembers {
    type Item = Member;

    fn request(&mut self) -> Option<Method> {
        self.inner.request()
    }

    fn accept(&mut self, object: Object) -> Result<Option<Vec<Member>>> {
        Ok(self.inner.accept(object)?.map(|page| {
            page.iter()
                .map(member_from_participant)
                .collect::<Vec<Member>>()
        }))
    }
}

/// Drains a channel's members into one vector.
///
/// # Errors
///
/// Returns the iterator's error.
pub fn fetch_members<T: Transport>(
    client: &mut Client<T>,
    channel: InputChannel,
    limit: i32,
) -> Result<Vec<Member>> {
    crate::pagination::fetch_all(
        &mut |method| client.invoke(method),
        ChannelMembers::new(channel).with_limit(limit),
    )
}

#[cfg(test)]
mod tests {
    use rtd_tg::{ChannelParticipant, Peer};

    use super::{MemberStatus, member_from_participant};

    #[test]
    fn maps_participant_statuses() {
        let member = ChannelParticipant::ChannelParticipant {
            flags: 0,
            user_id: 1,
            date: 0,
            subscription_until_date: None,
            rank: Some("member".to_owned()),
        };
        let member = member_from_participant(&member);
        assert_eq!(member.status, MemberStatus::Member);
        assert!(!member.is_admin());
        assert_eq!(member.rank.as_deref(), Some("member"));

        let left = ChannelParticipant::ChannelParticipantLeft {
            peer: Box::new(Peer::PeerUser { user_id: 9 }),
        };
        let member = member_from_participant(&left);
        assert_eq!(member.status, MemberStatus::Left);
        assert_eq!(member.user_id, Some(9));
        assert!(!member.is_admin());
    }
}
