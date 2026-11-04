//! Round-trip tests for the generated secret-chat model.

use rtd_bin::Writer;
use rtd_tg_e2e::{DecodeLimits, Decoder, DecryptedMessage, DecryptedMessageMedia};

#[test]
fn secret_chat_message_round_trips() {
    let message = DecryptedMessage::DecryptedMessage {
        flags: 1 << 9,
        silent: false,
        random_id: 42,
        ttl: 0,
        message: "hello secret world".to_owned(),
        media: Some(Box::new(
            DecryptedMessageMedia::DecryptedMessageMediaWebPage {
                url: "https://example.com".to_owned(),
            },
        )),
        entities: None,
        via_bot_name: None,
        reply_to_random_id: None,
        grouped_id: None,
    };

    let mut writer = Writer::new();
    message.encode(&mut writer);
    let bytes = writer.into_vec();

    let mut decoder = Decoder::new(&bytes, DecodeLimits::default());
    let decoded = DecryptedMessage::decode(&mut decoder).expect("decode secret message");
    assert_eq!(decoded, message);
}

#[test]
fn unknown_constructor_is_rejected() {
    let bytes = [0xff_u8, 0xff, 0xff, 0xff];
    let mut decoder = Decoder::new(&bytes, DecodeLimits::default());
    assert!(DecryptedMessage::decode(&mut decoder).is_err());
}
