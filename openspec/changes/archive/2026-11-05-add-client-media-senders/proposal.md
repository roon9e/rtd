# Proposal

## Why

`td`'s `telegram/message` has staged senders for audio, video, GIF, dice, venue, game and
web-page previews. `rtd-client::message` only sends photos, documents, geo, contacts and polls,
so callers cannot send the common staged media kinds without hand-building `InputMedia`.

## What Changes

- Add media builders (`audio_media`, `video_media`, `gif_media`, `dice_media`, `venue_media`,
  `game_media`, `webpage_media`) and their `send_*` wrappers.
- Add `AudioSpec`, `VideoSpec` and `VenueSpec` parameter types.

## Capabilities

### New Capabilities

- `client-media-senders`: staged audio/video/GIF/dice/venue/game/web-page senders.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: sticker senders (they need a sticker-set reference).
