//! Hand-written runtime tests for the generated `rtd-tlprofile` sidecar.
//!
//! These exercise the multi-layer machinery end to end: exact-profile
//! admission, wrapper selection, limits, dispatch, freezing and fail-closed
//! behavior.

use rtd_bin::Writer;
use rtd_tg::{
    AuthBindTempAuthKey, Bool, Decoder, InvokeWithLayer, MessagesDeleteMessages, Method, Object,
};
use rtd_tlprofile::{
    Admission, AdmitError, Dispatcher, FrozenObject, Limits, Profile, ProjectionError, admit,
    admit_default, admit_unprofiled, admit_with_limits, decode_object, lookup_route,
    resolve_profile, semantic_for_wire_id, wire_id,
};

const BIND_ID: u32 = 0xcdd4_2a05;
const DELETE_ID: u32 = 0xe58e_95d2;

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

#[test]
fn unsupported_layer_is_refused() {
    assert!(resolve_profile(232).is_some());
    assert!(resolve_profile(231).is_none());
    let body = encode_method(&bind());
    let error = admit(Profile::new(231), &body).unwrap_err();
    assert_eq!(error, AdmitError::UnsupportedLayer(231));
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
        .encode(Profile::new(231), &mut Writer::new())
        .unwrap_err();
    assert_eq!(
        error,
        ProjectionError::Unrepresentable {
            layer: 231,
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
    assert!(decode_object(Profile::new(231), &bytes, Limits::default()).is_err());
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
