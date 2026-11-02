//! Hand-written runtime tests for the generated `rtd-tlprofile` sidecar.
//!
//! These exercise the multi-layer machinery end to end: exact-profile
//! admission, wrapper selection, limits, dispatch, freezing and fail-closed
//! behavior.

use rtd_bin::Writer;
use rtd_tg::{
    AuthBindTempAuthKey, Bool, BotCommand, BotInfo, BotVerification, ButtonType,
    ChannelsJoinChannel, ChatAdminRights, Decoder, InitConnection, InputChannel, InputInvoice,
    InputPeer, InvokeAfterMsg, InvokeAfterMsgs, InvokeWithLayer, KeyboardButton, MessageEntity,
    MessagesDeleteMessages, Method, Object, SendMessageAction, TextWithEntities, Update,
};
use rtd_tlprofile::{
    Admission, AdmitError, Dispatcher, FrozenObject, Limits, Profile, ProjectionError, RouteMode,
    WrapperKind, admit, admit_default, admit_unprofiled, admit_with_limits, decode_object,
    lookup_route, resolve_profile, semantic, semantic_for_wire_id, wire_id,
};

const BIND_ID: u32 = 0xcdd4_2a05;
const DELETE_ID: u32 = 0xe58e_95d2;
const INIT_CONNECTION_ID: u32 = 0xc1cd_5ea9;

fn encode_method(method: &Method) -> Vec<u8> {
    let mut writer = rtd_bin::Writer::new();
    method.encode(&mut writer);
    writer.into_vec()
}

fn bind() -> Method {
    Method::AuthBindTempAuthKey(Box::new(AuthBindTempAuthKey {
        perm_auth_key_id: 1,
        nonce: 2,
        expires_at: 3,
        encrypted_message: vec![1, 2, 3],
    }))
}

fn invoke(layer: i32, query: Method) -> Method {
    Method::InvokeWithLayer(Box::new(InvokeWithLayer {
        layer,
        query: Box::new(query),
    }))
}

fn init_connection(query: Method) -> Method {
    Method::InitConnection(Box::new(InitConnection {
        flags: 0,
        api_id: 2040,
        device_model: "RTD".to_owned(),
        system_version: "Linux".to_owned(),
        app_version: "1.0".to_owned(),
        system_lang_code: "en".to_owned(),
        lang_pack: String::new(),
        lang_code: "en".to_owned(),
        proxy: None,
        params: None,
        query: Box::new(query),
    }))
}

fn invoke_after_msg(msg_id: i64, query: Method) -> Method {
    Method::InvokeAfterMsg(Box::new(InvokeAfterMsg {
        msg_id,
        query: Box::new(query),
    }))
}

fn invoke_after_msgs(msg_ids: Vec<i64>, query: Method) -> Method {
    Method::InvokeAfterMsgs(Box::new(InvokeAfterMsgs {
        msg_ids,
        query: Box::new(query),
    }))
}

/// A rewrite route has an identical local body but a nested type that changed in
/// the older layer. Encoding for the profile must re-identify the nested value,
/// and decoding must rebuild the canonical value.
#[test]
fn rewrite_type_round_trips_on_the_profile() {
    let value = Object::Update(Box::new(Update::UpdateUserTyping {
        flags: 0,
        user_id: 7,
        top_msg_id: None,
        action: Box::new(SendMessageAction::SendMessageTypingAction),
    }));
    let frozen = FrozenObject::freeze(value.clone()).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode for the profile");
    let bytes = writer.into_vec();

    // The first word is the profile's wire id for this constructor.
    let expected = rtd_tlprofile::routes()
        .iter()
        .find(|route| {
            route.profile == 225
                && rtd_tlprofile::canonical_wire_id(route.semantic) == Some(0x2a17_bf5c)
        })
        .map(|route| route.wire_id)
        .expect("layer-225 route for updateUserTyping");
    let id = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    assert_eq!(id, expected);

    let decoded = decode_object(Profile::new(225), &bytes, Limits::default()).expect("decode");
    assert_eq!(decoded, value);
}

/// A policy route drops fields the older layer lacks. Layer 225
/// `chatAdminRights` has no `manage_linked_peers`/`manage_welcome_messages`
/// presence bits, so encoding must recompute the flags word without them and
/// decoding must rebuild the canonical value with them clear, keeping the
/// shared bits intact.
#[test]
fn policy_drop_projects_presence_bits_on_the_profile() {
    let value = Object::ChatAdminRights(Box::new(ChatAdminRights {
        flags: 1,
        change_info: true,
        post_messages: false,
        edit_messages: false,
        delete_messages: false,
        ban_users: false,
        invite_users: false,
        pin_messages: false,
        add_admins: false,
        anonymous: false,
        manage_call: false,
        other: false,
        manage_topics: false,
        post_stories: false,
        edit_stories: false,
        delete_stories: false,
        manage_direct_messages: false,
        manage_ranks: false,
        manage_linked_peers: true,
        manage_welcome_messages: true,
    }));
    let frozen = FrozenObject::freeze(value).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode for layer 225");
    let bytes = writer.into_vec();

    // The layer-225 wire id is followed by the recomputed flags word, which must
    // not carry the dropped presence bits.
    let flags = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(flags & (1 << 19), 0, "manage_linked_peers must be dropped");
    assert_eq!(
        flags & (1 << 20),
        0,
        "manage_welcome_messages must be dropped"
    );
    assert_eq!(flags & 1, 1, "the shared change_info bit is preserved");

    let decoded = decode_object(Profile::new(225), &bytes, Limits::default()).expect("decode");
    let Object::ChatAdminRights(decoded) = decoded else {
        panic!("expected chatAdminRights");
    };
    assert!(decoded.change_info);
    assert!(!decoded.manage_linked_peers);
    assert!(!decoded.manage_welcome_messages);
    assert_eq!(decoded.flags, 1);
}

/// A policy route may reject a canonical value that carries a field the older
/// layer lacks, rather than silently dropping the data. Layer 225
/// `inputInvoiceStarGiftResale` has neither the `show_name` bit nor the
/// `message` field, so a value that sets either is refused, while a value that
/// leaves both clear round trips.
#[test]
fn policy_reject_if_present_fails_closed_on_the_profile() {
    let clean = Object::InputInvoice(Box::new(InputInvoice::InputInvoiceStarGiftResale {
        flags: 1,
        ton: true,
        show_name: false,
        slug: "gift".to_owned(),
        to_id: Box::new(InputPeer::InputPeerEmpty),
        message: None,
    }));
    let frozen = FrozenObject::freeze(clean.clone()).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode the representable value");
    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    assert_eq!(decoded, clean);

    for (flags, show_name, message) in [
        (1u32 | 4, true, None),
        (
            1u32 | 2,
            false,
            Some(Box::new(TextWithEntities {
                text: "hi".to_owned(),
                entities: Vec::new(),
            })),
        ),
    ] {
        let unsupported = FrozenObject::freeze(Object::InputInvoice(Box::new(
            InputInvoice::InputInvoiceStarGiftResale {
                flags,
                ton: true,
                show_name,
                slug: "gift".to_owned(),
                to_id: Box::new(InputPeer::InputPeerEmpty),
                message,
            },
        )))
        .expect("freeze");
        let error = unsupported
            .encode(Profile::new(225), &mut Writer::new())
            .expect_err("the unsupported field is refused");
        assert_eq!(
            error,
            ProjectionError::Unrepresentable {
                layer: 225,
                wire_id: 0xe9b0_c658,
            }
        );
    }
}

#[test]
fn retag_method_admits_on_the_profile() {
    // `channels.joinChannel` has a different wire id in layer 225 but an
    // identical body, so a layer-225 request is the profile id followed by the
    // canonical body. Admission must map it back to the canonical method.
    let method = Method::ChannelsJoinChannel(Box::new(ChannelsJoinChannel {
        channel: Box::new(InputChannel::InputChannel {
            channel_id: 7,
            access_hash: 11,
        }),
    }));
    let mut writer = Writer::new();
    writer.put_u32(0x24b5_24c5);
    method.encode_body(&mut writer);
    let body = writer.into_vec();

    let admission = admit(Profile::new(225), &body).expect("retag admits");
    assert_eq!(admission.effective_profile, Some(Profile::new(225)));
    assert_eq!(admission.wire_id, 0x24b5_24c5);
    assert_eq!(admission.request, method);
    assert_eq!(
        admission.semantic,
        semantic_for_wire_id(Profile::new(232), 0x7f6a_1e22).unwrap()
    );
}

/// A field-shaped adapter hook converts one field between its canonical and
/// older-layer types. Layer 225 `botVerification.description` is a plain
/// `string`, so a plain canonical `TextWithEntities` round trips.
#[test]
fn policy_field_hook_round_trips_text() {
    let value = Object::BotVerification(Box::new(BotVerification {
        bot_id: 1,
        icon: 2,
        description: Box::new(TextWithEntities {
            text: "verified".to_owned(),
            entities: Vec::new(),
        }),
    }));
    let frozen = FrozenObject::freeze(value.clone()).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode for layer 225");
    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    assert_eq!(decoded, value);
}

/// The historical `string` cannot carry entities, so the hook fails closed and
/// the encoder surfaces it as a conversion failure.
#[test]
fn policy_field_hook_fails_closed_on_entities() {
    let value = Object::BotVerification(Box::new(BotVerification {
        bot_id: 1,
        icon: 2,
        description: Box::new(TextWithEntities {
            text: "verified".to_owned(),
            entities: vec![MessageEntity::MessageEntityUnknown {
                offset: 0,
                length: 1,
            }],
        }),
    }));
    let frozen = FrozenObject::freeze(value).expect("freeze");
    let error = frozen
        .encode(Profile::new(225), &mut Writer::new())
        .expect_err("entities are refused");
    assert!(matches!(error, ProjectionError::Hook(_)), "{error:?}");
}

/// A `default` obligation substitutes the recorded value for a mandatory target
/// field whose source side omits it. Layer 225 `auth.sentCodePaymentRequired` has
/// no `premium_days`, so decoding rebuilds the canonical value with the default.
#[test]
fn policy_default_fills_a_missing_canonical_field() {
    use rtd_tg::AuthSentCode;

    let route = rtd_tlprofile::routes()
        .iter()
        .find(|route| {
            route.profile == 225 && route.semantic == semantic::TYPE_AUTH_SENT_CODE_PAYMENT_REQUIRED
        })
        .copied()
        .expect("layer 225 has a sentCodePaymentRequired route");
    assert_eq!(route.mode, RouteMode::Policy);

    let mut writer = Writer::new();
    writer.put_u32(route.wire_id);
    writer.put_string("store");
    writer.put_string("hash");
    writer.put_string("");
    writer.put_string("");
    writer.put_string("USD");
    writer.put_i64(7);

    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    let Object::AuthSentCode(value) = decoded else {
        panic!("expected authSentCode");
    };
    let AuthSentCode::SentCodePaymentRequired {
        premium_days,
        amount,
        ..
    } = *value
    else {
        panic!("expected sentCodePaymentRequired");
    };
    assert_eq!(premium_days, 0);
    assert_eq!(amount, 7);
}

/// A wire id that exists only in an older layer has no canonical value to decode
/// into, so decoding for that profile must fail closed rather than guess.
#[test]
fn profile_only_route_is_refused() {
    let route = rtd_tlprofile::routes()
        .iter()
        .find(|route| route.profile == 225 && route.mode == RouteMode::ProfileOnly)
        .copied()
        .expect("layer 225 has a profile-only route");
    let error = decode_object(
        Profile::new(225),
        &route.wire_id.to_le_bytes(),
        Limits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, AdmitError::UnsupportedRoute(found) if found == route),
        "{error:?}"
    );
}

/// A wire id with no route in the profile at all is refused.
#[test]
fn unknown_wire_id_is_refused() {
    let error = decode_object(
        Profile::new(225),
        &0xdead_beef_u32.to_le_bytes(),
        Limits::default(),
    )
    .unwrap_err();
    assert!(matches!(error, AdmitError::Decode(_)), "{error:?}");
}

/// A policy *method* is admitted: the profile wire id is decoded through the
/// generated policy codec and the canonical method is resolved, so admission no
/// longer refuses rewrite/policy terminals.
#[test]
fn policy_method_admits_on_the_profile() {
    let route = rtd_tlprofile::routes()
        .iter()
        .find(|route| {
            route.profile == 225 && route.semantic == semantic::METHOD_BOTS_SET_CUSTOM_VERIFICATION
        })
        .copied()
        .expect("layer 225 has a bots.setCustomVerification route");
    assert_eq!(route.mode, RouteMode::Policy);

    let mut writer = Writer::new();
    writer.put_u32(route.wire_id);
    writer.put_u32(0); // no bot, enabled = false, no custom_description
    writer.put_u32(0x7da0_7ec9); // InputPeerSelf
    let admission = admit(Profile::new(225), &writer.into_vec()).expect("policy method admits");
    assert_eq!(admission.wire_id, route.wire_id);
    assert_eq!(
        admission.semantic,
        semantic::METHOD_BOTS_SET_CUSTOM_VERIFICATION
    );
    let Method::BotsSetCustomVerification(value) = admission.request else {
        panic!("expected bots.setCustomVerification");
    };
    assert!(!value.enabled);
    assert!(value.custom_description.is_none());
}

#[test]
fn unsupported_layer_is_refused() {
    assert!(resolve_profile(232).is_some());
    assert!(resolve_profile(224).is_none());
    let body = encode_method(&bind());
    let error = admit(Profile::new(224), &body).unwrap_err();
    assert_eq!(error, AdmitError::UnsupportedLayer(224));
}

#[test]
fn supported_layer_is_served_exactly() {
    let body = encode_method(&bind());
    let admission = admit(Profile::new(232), &body).expect("admitted");
    assert_eq!(admission.effective_profile, Some(Profile::new(232)));
    assert_eq!(admission.wire_id, BIND_ID);
    assert_eq!(
        admission.semantic,
        semantic_for_wire_id(Profile::new(232), BIND_ID).unwrap()
    );
    assert!(matches!(admission.request, Method::AuthBindTempAuthKey(_)));
}

#[test]
fn conflicting_selectors_are_rejected() {
    let body = encode_method(&invoke(232, invoke(231, bind())));
    let error = admit(Profile::new(232), &body).unwrap_err();
    assert_eq!(error, AdmitError::Conflict(232, 231));
}

#[test]
fn default_inheritance_lets_a_selector_replace_the_fallback() {
    // The caller fallback is an unsupported layer, but the first selector wins.
    let body = encode_method(&invoke(232, bind()));
    let admission = admit_default(Profile::new(231), &body).expect("admitted");
    assert_eq!(admission.effective_profile, Some(Profile::new(232)));
    assert_eq!(admission.explicit_profile, Some(Profile::new(232)));
}

#[test]
fn frozen_profile_rejects_a_conflicting_selector() {
    let body = encode_method(&invoke(231, bind()));
    let error = admit(Profile::new(232), &body).unwrap_err();
    assert_eq!(error, AdmitError::Conflict(232, 231));
}

#[test]
fn limits_are_enforced() {
    let body = encode_method(&bind());
    let limits = Limits {
        max_wire_bytes: 2,
        ..Limits::default()
    };
    let error = admit_with_limits(Profile::new(232), &body, limits).unwrap_err();
    assert_eq!(
        error,
        AdmitError::Limit {
            name: "max_wire_bytes",
            value: body.len()
        }
    );

    // A tiny vector-element limit is rejected while decoding.
    let body = encode_method(&Method::MessagesDeleteMessages(Box::new(
        MessagesDeleteMessages {
            flags: 0,
            revoke: false,
            id: (0..64).collect(),
        },
    )));
    let limits = Limits {
        max_vector_elements: 4,
        ..Limits::default()
    };
    let error = admit_with_limits(Profile::new(232), &body, limits).unwrap_err();
    assert!(matches!(error, AdmitError::Decode(_)), "{error:?}");
}

#[test]
fn dispatch_and_result_encoding() {
    let body = encode_method(&bind());
    let admission: Admission = admit(Profile::new(232), &body).unwrap();
    let mut dispatcher = Dispatcher::new();
    let semantic = semantic_for_wire_id(Profile::new(232), BIND_ID).unwrap();
    dispatcher.register(semantic, |_admission: &Admission| {
        Ok(Object::Bool(Box::new(Bool::BoolTrue)))
    });
    let frozen = dispatcher.dispatch(&admission).expect("dispatched");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(232), &mut writer)
        .expect("encoded");
    let bytes = writer.into_vec();
    let mut decoder = Decoder::new(&bytes, rtd_tg::DecodeLimits::default());
    let decoded = Object::decode(&mut decoder).expect("decode");
    assert_eq!(decoded, Object::Bool(Box::new(Bool::BoolTrue)));
}

#[test]
fn unrepresentable_value_fails_closed() {
    let frozen = FrozenObject::freeze(Object::Bool(Box::new(Bool::BoolTrue))).unwrap();
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(232), &mut writer)
        .expect("canonical encoding");
    let error = frozen
        .encode(Profile::new(224), &mut Writer::new())
        .unwrap_err();
    assert_eq!(
        error,
        ProjectionError::Unrepresentable {
            layer: 224,
            wire_id: frozen.type_id()
        }
    );
}

#[test]
fn freezing_fans_out_to_multiple_profiles() {
    let frozen = FrozenObject::freeze(Object::Bool(Box::new(Bool::BoolFalse))).unwrap();
    let first = resolve_profile(232).unwrap();
    let second = resolve_profile(232).unwrap();
    let mut a = Writer::new();
    let mut b = Writer::new();
    frozen.encode(first, &mut a).unwrap();
    frozen.encode(second, &mut b).unwrap();
    assert_eq!(a.as_slice(), b.as_slice());
    assert_eq!(frozen.canonical(), &Object::Bool(Box::new(Bool::BoolFalse)));
}

/// One frozen canonical value fans out to every enabled layer that has a route
/// for it: each layer's encoding decodes back to the same value. `botVerification`
/// is canonical in 230..=232 and a field-hook policy route in 225..=229.
#[test]
fn freezing_fans_out_to_every_enabled_layer() {
    let value = Object::BotVerification(Box::new(BotVerification {
        bot_id: 1,
        icon: 2,
        description: Box::new(TextWithEntities {
            text: "verified".to_owned(),
            entities: Vec::new(),
        }),
    }));
    let frozen = FrozenObject::freeze(value.clone()).expect("freeze");
    for &layer in rtd_tlprofile::PROFILES {
        let profile = resolve_profile(layer).expect("enabled layer resolves");
        let mut writer = Writer::new();
        frozen
            .encode(profile, &mut writer)
            .unwrap_or_else(|error| panic!("encode for layer {layer}: {error:?}"));
        let decoded = decode_object(profile, &writer.into_vec(), Limits::default())
            .unwrap_or_else(|error| panic!("decode for layer {layer}: {error:?}"));
        assert_eq!(decoded, value, "layer {layer} did not round trip");
    }
}

/// A `project` obligation drops a canonical value the older layer cannot
/// represent. Layer 225 `botCommand` has no `ephemeral` bit, so an ephemeral
/// command is projected away (the gate returns `Dropped`) while a plain command
/// round trips.
#[test]
fn policy_projection_gates_an_unrepresentable_value() {
    let plain = Object::BotCommand(Box::new(BotCommand {
        flags: 0,
        ephemeral: false,
        command: "start".to_owned(),
        description: "Start".to_owned(),
    }));
    let frozen = FrozenObject::freeze(plain.clone()).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode a plain command");
    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    assert_eq!(decoded, plain);

    let ephemeral = FrozenObject::freeze(Object::BotCommand(Box::new(BotCommand {
        flags: 1,
        ephemeral: true,
        command: "start".to_owned(),
        description: "Start".to_owned(),
    })))
    .expect("freeze");
    let error = ephemeral
        .encode(Profile::new(225), &mut Writer::new())
        .expect_err("an ephemeral command is projected away");
    assert_eq!(error, ProjectionError::Dropped);
}

/// A projected vector element is dropped, not the whole container: a `botInfo`
/// whose `commands` includes an ephemeral command encodes for layer 225 without
/// that command.
#[test]
fn policy_projection_drops_a_vector_element() {
    let plain = BotCommand {
        flags: 0,
        ephemeral: false,
        command: "a".to_owned(),
        description: "A".to_owned(),
    };
    let ephemeral = BotCommand {
        flags: 1,
        ephemeral: true,
        command: "b".to_owned(),
        description: "B".to_owned(),
    };
    let value = Object::BotInfo(Box::new(BotInfo {
        flags: 0,
        has_preview_medias: false,
        user_id: None,
        description: None,
        description_photo: None,
        description_document: None,
        commands: Some(vec![plain, ephemeral]),
        menu_button: None,
        privacy_policy_url: None,
        app_settings: None,
        verifier_settings: None,
    }));
    let frozen = FrozenObject::freeze(value).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode for layer 225");
    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    let Object::BotInfo(info) = decoded else {
        panic!("expected botInfo");
    };
    let commands = info.commands.expect("commands are present");
    assert_eq!(commands.len(), 1, "the ephemeral command is dropped");
    assert_eq!(commands[0].command, "a");
    assert!(!commands[0].ephemeral);
}

/// `keyboardButton` must project a non-default button type away and supply the
/// default type when decoding the historical typeless button.
#[test]
fn policy_keyboard_button_supplies_and_projects_the_type() {
    let value = Object::KeyboardButton(Box::new(KeyboardButton {
        flags: 0,
        style: None,
        text: "OK".to_owned(),
        type_: Box::new(ButtonType::ButtonTypeDefault),
    }));
    let frozen = FrozenObject::freeze(value.clone()).expect("freeze");
    let mut writer = Writer::new();
    frozen
        .encode(Profile::new(225), &mut writer)
        .expect("encode a default-typed button");
    let decoded =
        decode_object(Profile::new(225), &writer.into_vec(), Limits::default()).expect("decode");
    assert_eq!(decoded, value);

    let non_default = FrozenObject::freeze(Object::KeyboardButton(Box::new(KeyboardButton {
        flags: 0,
        style: None,
        text: "OK".to_owned(),
        type_: Box::new(ButtonType::ButtonTypeRequestPhone),
    })))
    .expect("freeze");
    let error = non_default
        .encode(Profile::new(225), &mut Writer::new())
        .expect_err("a non-default button type is projected away");
    assert_eq!(error, ProjectionError::Dropped);
}

/// A second present-in-both family fans out across the enabled set: layer 225..=228
/// `chatAdminRights` is a policy route that drops two presence bits, and 229..=232
/// is canonical. A value with those bits clear round trips through every layer.
#[test]
fn policy_drop_family_fans_out_to_every_enabled_layer() {
    let value = Object::ChatAdminRights(Box::new(ChatAdminRights {
        flags: 1,
        change_info: true,
        post_messages: false,
        edit_messages: false,
        delete_messages: false,
        ban_users: false,
        invite_users: false,
        pin_messages: false,
        add_admins: false,
        anonymous: false,
        manage_call: false,
        other: false,
        manage_topics: false,
        post_stories: false,
        edit_stories: false,
        delete_stories: false,
        manage_direct_messages: false,
        manage_ranks: false,
        manage_linked_peers: false,
        manage_welcome_messages: false,
    }));
    let frozen = FrozenObject::freeze(value.clone()).expect("freeze");
    for &layer in rtd_tlprofile::PROFILES {
        let profile = resolve_profile(layer).expect("enabled layer resolves");
        let mut writer = Writer::new();
        frozen
            .encode(profile, &mut writer)
            .unwrap_or_else(|error| panic!("encode for layer {layer}: {error:?}"));
        let decoded = decode_object(profile, &writer.into_vec(), Limits::default())
            .unwrap_or_else(|error| panic!("decode for layer {layer}: {error:?}"));
        assert_eq!(decoded, value, "layer {layer} did not round trip");
    }
}

#[test]
fn invariant_terminal_needs_no_profile() {
    let body = encode_method(&bind());
    let admission = admit_unprofiled(&body).expect("admitted without a profile");
    assert_eq!(admission.effective_profile, None);
    assert_eq!(admission.wire_id, BIND_ID);
}

#[test]
fn flags_round_trip_through_admission() {
    // The flags word is rebuilt from its controlled bits, so `flags` must agree
    // with `revoke` for the round trip to be an identity.
    let original = Method::MessagesDeleteMessages(Box::new(MessagesDeleteMessages {
        flags: 1,
        revoke: true,
        id: vec![7, 8, 9],
    }));
    let body = encode_method(&original);
    let admission = admit(Profile::new(232), &body).expect("admitted");
    assert_eq!(admission.request, original);
}

#[test]
fn decode_object_is_profile_bound() {
    let mut writer = Writer::new();
    Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
    let bytes = writer.into_vec();
    let value = decode_object(Profile::new(232), &bytes, Limits::default()).expect("decoded");
    assert_eq!(value, Object::Bool(Box::new(Bool::BoolTrue)));
    assert!(decode_object(Profile::new(224), &bytes, Limits::default()).is_err());
}

#[test]
fn wrapper_selects_profile_without_a_caller_profile() {
    let body = encode_method(&invoke(232, bind()));
    let admission = admit_unprofiled(&body).expect("admitted");
    assert_eq!(admission.effective_profile, Some(Profile::new(232)));
}

#[test]
fn semantic_identity_round_trips() {
    let profile = Profile::new(232);
    let semantic = semantic_for_wire_id(profile, BIND_ID).unwrap();
    assert_eq!(wire_id(profile, semantic), Some(BIND_ID));
    assert!(lookup_route(profile, DELETE_ID).is_some());
}

#[test]
fn init_connection_capture() {
    let body = encode_method(&invoke(232, init_connection(bind())));
    let admission = admit(Profile::new(232), &body).expect("admitted");

    let view = admission.view();
    assert_eq!(view.effective_profile(), Some(Profile::new(232)));
    assert_eq!(view.explicit_profile(), Some(Profile::new(232)));
    assert_eq!(view.wrapper_count(), 2);

    let layer = view.wrapper(0).expect("layer wrapper");
    assert_eq!(layer.kind(), WrapperKind::Layer);
    assert_eq!(layer.semantic(), semantic::METHOD_INVOKE_WITH_LAYER);
    assert_eq!(layer.profile(), Some(Profile::new(232)));

    let init = view.wrapper(1).expect("init wrapper");
    assert_eq!(init.kind(), WrapperKind::InitConnection);
    assert_eq!(init.wire_id(), INIT_CONNECTION_ID);
    let client = init.client().expect("client info captured");
    assert_eq!(client.api_id, 2040);
    assert_eq!(client.device_model, "RTD");
    assert_eq!(client.system_version, "Linux");
    assert_eq!(client.app_version, "1.0");
    assert_eq!(client.system_lang_code, "en");
    assert!(client.lang_pack.is_empty());
    assert_eq!(client.lang_code, "en");

    // The terminal remains the inner method, not the wrapper.
    assert_eq!(admission.wire_id, BIND_ID);
    assert_eq!(
        admission.semantic,
        semantic_for_wire_id(Profile::new(232), BIND_ID).unwrap()
    );
}

#[test]
fn invoke_after_ordering_key() {
    let body = encode_method(&invoke_after_msgs(
        vec![11, 22],
        invoke_after_msg(33, bind()),
    ));
    let admission = admit(Profile::new(232), &body).expect("admitted");

    assert_eq!(admission.wrappers().len(), 2);
    assert_eq!(admission.wrappers()[0].kind(), WrapperKind::InvokeAfterMsgs);
    assert_eq!(admission.wrappers()[0].ordering().as_slice(), &[11, 22]);
    assert_eq!(admission.wrappers()[1].kind(), WrapperKind::InvokeAfterMsg);
    assert_eq!(admission.wrappers()[1].ordering().as_slice(), &[33]);

    // The call identity accumulates the ordering ids, outermost first.
    let identity = admission.call_identity();
    assert_eq!(identity.ordering().as_slice(), &[11, 22, 33]);
    assert_eq!(identity.semantic(), admission.semantic);
    assert_eq!(identity.wire_id(), BIND_ID);
    assert!(!identity.ordering().is_empty());
}

#[test]
fn dispatch_result_carries_call_identity() {
    let body = encode_method(&invoke_after_msg(7, bind()));
    let admission = admit(Profile::new(232), &body).expect("admitted");
    let method = admission.semantic;

    let mut dispatcher = Dispatcher::new();
    dispatcher.register(method, |_admission: &Admission| {
        Ok(Object::Bool(Box::new(Bool::BoolTrue)))
    });
    let result = dispatcher.dispatch(&admission).expect("dispatched");

    assert_eq!(result.identity().semantic(), method);
    assert_eq!(result.identity().wire_id(), BIND_ID);
    assert_eq!(result.identity().ordering().as_slice(), &[7]);
    assert_eq!(
        result.value().canonical(),
        &Object::Bool(Box::new(Bool::BoolTrue))
    );

    let mut writer = Writer::new();
    result
        .encode(Profile::new(232), &mut writer)
        .expect("encoded");
    assert!(!writer.into_vec().is_empty());
}

#[test]
fn fail_closed_behaviors_are_unchanged() {
    // Unsupported-layer refusal.
    let body = encode_method(&bind());
    assert_eq!(
        admit(Profile::new(224), &body).unwrap_err(),
        AdmitError::UnsupportedLayer(224)
    );

    // Conflicting-selector rejection.
    let body = encode_method(&invoke(232, invoke(231, bind())));
    assert_eq!(
        admit(Profile::new(232), &body).unwrap_err(),
        AdmitError::Conflict(232, 231)
    );

    // Unrepresentable-value failure.
    let frozen = FrozenObject::freeze(Object::Bool(Box::new(Bool::BoolTrue))).unwrap();
    assert_eq!(
        frozen
            .encode(Profile::new(224), &mut Writer::new())
            .unwrap_err(),
        ProjectionError::Unrepresentable {
            layer: 224,
            wire_id: frozen.type_id()
        }
    );
}
