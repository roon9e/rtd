//! A WebRTC-backed media connection.
//!
//! Enabled by default through the `media-webrtc` feature (disable it with
//! `default-features = false`). This is the concrete backend behind
//! [`crate::calls::MediaConnection`]: it owns a `webrtc` peer connection with a
//! single Opus audio track and reports lifecycle events through
//! [`crate::calls::MediaEvents`]. Signaling and the DH exchange stay in
//! [`crate::calls`]; this module only carries media.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MediaEngine};
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

use crate::calls::{MediaConnection, MediaEvents, TrackKind};
use crate::error::{Error, Result};

/// The number of 16-bit PCM samples in one 20 ms Opus frame at 48 kHz.
const SAMPLES_PER_FRAME: usize = 960;

/// A WebRTC audio connection driving [`MediaEvents`].
pub struct WebRtcMedia {
    /// The underlying peer connection.
    peer: Arc<RTCPeerConnection>,
    /// The local Opus track.
    track: Arc<TrackLocalStaticSample>,
    /// A runtime handle for the synchronous [`MediaConnection`] surface.
    runtime: tokio::runtime::Handle,
}

impl WebRtcMedia {
    /// Creates a peer connection with one local Opus audio track.
    ///
    /// `events` receives `on_track`, `on_connected` and `on_disconnected` as the
    /// peer connection changes state.
    ///
    /// # Errors
    ///
    /// Returns an error when no tokio runtime is current, or the WebRTC stack
    /// cannot be initialised.
    pub fn new(events: Arc<dyn MediaEvents>) -> Result<Self> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
            Error::Unexpected("a tokio runtime is required to build WebRTC media".to_owned())
        })?;

        let mut media = MediaEngine::default();
        media.register_default_codecs().map_err(webrtc_error)?;
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media).map_err(webrtc_error)?;
        let api = APIBuilder::new()
            .with_media_engine(media)
            .with_interceptor_registry(registry)
            .build();

        let (peer, track) = runtime.block_on(async {
            let peer = api
                .new_peer_connection(RTCConfiguration::default())
                .await
                .map_err(webrtc_error)?;
            let track = Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_OPUS.to_owned(),
                    ..RTCRtpCodecCapability::default()
                },
                "audio".to_owned(),
                "rtd".to_owned(),
            ));
            peer.add_track(track.clone()).await.map_err(webrtc_error)?;
            Ok::<_, Error>((Arc::new(peer), track))
        })?;

        let connected = events.clone();
        peer.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            match state {
                RTCPeerConnectionState::Connected => connected.on_connected(),
                RTCPeerConnectionState::Disconnected
                | RTCPeerConnectionState::Failed
                | RTCPeerConnectionState::Closed => connected.on_disconnected(),
                _ => {}
            }
            Box::pin(async {})
        }));

        let tracked = events;
        peer.on_track(Box::new(move |_track, _receiver, _transceiver| {
            tracked.on_track(TrackKind::Audio);
            Box::pin(async {})
        }));

        Ok(Self {
            peer,
            track,
            runtime,
        })
    }

    /// Returns the underlying peer connection.
    #[must_use]
    pub fn peer_connection(&self) -> &Arc<RTCPeerConnection> {
        &self.peer
    }

    /// Returns the local audio track.
    #[must_use]
    pub fn audio_track(&self) -> &Arc<TrackLocalStaticSample> {
        &self.track
    }

    /// Closes the peer connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the peer connection cannot be closed.
    pub fn close(&self) -> Result<()> {
        self.runtime
            .block_on(self.peer.close())
            .map_err(webrtc_error)?;
        Ok(())
    }
}

impl MediaConnection for WebRtcMedia {
    fn write_audio(&mut self, samples: &[i16]) -> Result<()> {
        // Interleave-encode the PCM samples as little-endian bytes, then emit
        // one 20 ms Opus frame's worth per `write_sample`.
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let frame_bytes = SAMPLES_PER_FRAME * 2;
        for frame in bytes.chunks(frame_bytes.max(2)) {
            let sample = Sample {
                data: Bytes::copy_from_slice(frame),
                duration: Duration::from_millis(20),
                ..Sample::default()
            };
            self.runtime
                .block_on(self.track.write_sample(&sample))
                .map_err(webrtc_error)?;
        }
        Ok(())
    }

    fn audio_ssrc(&self) -> Option<u32> {
        // The SSRC is assigned by the peer connection during negotiation, so it
        // is not known before the track is bound.
        None
    }
}

/// Converts a WebRTC error into the client error type.
fn webrtc_error(error: impl std::fmt::Display) -> Error {
    Error::Unexpected(format!("webrtc: {error}"))
}
