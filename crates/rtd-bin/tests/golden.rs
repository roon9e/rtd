//! Golden byte-vector tests for TL primitives.
//!
//! These vectors pin the exact wire encoding; a change to any of them is a
//! protocol change and must be justified.

use rtd_bin::{
    Error, Int128, Int256, Reader, TYPE_BOOL_FALSE, TYPE_BOOL_TRUE, TYPE_VECTOR, Writer,
};

fn encode(f: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::new();
    f(&mut w);
    w.into_vec()
}

#[test]
fn integers_are_little_endian() {
    assert_eq!(
        encode(|w| w.put_u32(0x0102_0304)),
        vec![0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(encode(|w| w.put_i32(-1)), vec![0xff, 0xff, 0xff, 0xff]);
    assert_eq!(encode(|w| w.put_u64(1)), vec![1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn booleans_use_constructor_ids() {
    assert_eq!(encode(|w| w.put_bool(true)), vec![0xb5, 0x75, 0x72, 0x99]);
    assert_eq!(encode(|w| w.put_bool(false)), vec![0x37, 0x97, 0x79, 0xbc]);
    assert_eq!(TYPE_BOOL_TRUE.to_le_bytes(), [0xb5, 0x75, 0x72, 0x99]);
    assert_eq!(TYPE_BOOL_FALSE.to_le_bytes(), [0x37, 0x97, 0x79, 0xbc]);
}

#[test]
fn strings_and_padding() {
    assert_eq!(encode(|w| w.put_string("")), vec![0, 0, 0, 0]);
    assert_eq!(encode(|w| w.put_string("abc")), vec![3, b'a', b'b', b'c']);
    assert_eq!(
        encode(|w| w.put_string("abcd")),
        vec![4, b'a', b'b', b'c', b'd', 0, 0, 0]
    );
}

#[test]
fn long_string_form() {
    let payload = vec![0x41u8; 254];
    let out = encode(|w| w.put_bytes(&payload));
    // 0xFE marker + 3-byte little-endian length (254) + payload + 2 pad bytes.
    assert_eq!(&out[..4], &[0xfe, 0xfe, 0x00, 0x00]);
    assert_eq!(out.len(), 4 + 254 + 2);
}

#[test]
fn vector_header() {
    let mut expected = TYPE_VECTOR.to_le_bytes().to_vec();
    expected.extend_from_slice(&2i32.to_le_bytes());
    assert_eq!(encode(|w| w.put_vector_header(2)), expected);
}

#[test]
fn int128_and_int256_are_raw_bytes() {
    let a = Int128::from_array([7u8; 16]);
    assert_eq!(encode(|w| w.put_int128(a)), vec![7u8; 16]);
    let b = Int256::from_array([9u8; 32]);
    assert_eq!(encode(|w| w.put_int256(b)), vec![9u8; 32]);
}

#[test]
fn round_trip_scalars_and_strings() {
    let bytes = encode(|w| {
        w.put_u32(0xdead_beef);
        w.put_i64(-42);
        w.put_f64(1.5);
        w.put_bool(false);
        w.put_string("hello world");
        w.put_bytes(&[1, 2, 3, 4, 5]);
        w.put_vector_header(3);
        w.put_u32(1);
        w.put_u32(2);
        w.put_u32(3);
    });

    let mut r = Reader::new(&bytes);
    assert_eq!(r.take_u32().unwrap(), 0xdead_beef);
    assert_eq!(r.take_i64().unwrap(), -42);
    assert_eq!(r.take_f64().unwrap(), 1.5);
    assert!(!r.take_bool().unwrap());
    assert_eq!(r.take_string().unwrap(), "hello world");
    assert_eq!(r.take_bytes().unwrap(), &[1, 2, 3, 4, 5]);
    assert_eq!(r.take_vector_header().unwrap(), 3);
    assert_eq!(r.take_u32().unwrap(), 1);
    assert_eq!(r.take_u32().unwrap(), 2);
    assert_eq!(r.take_u32().unwrap(), 3);
    assert!(r.is_empty());
}

#[test]
fn decode_rejects_bad_inputs() {
    // Truncated scalar.
    let mut r = Reader::new(&[1, 2, 3]);
    assert!(matches!(r.take_u32(), Err(Error::UnexpectedEof { .. })));

    // Wrong bool constructor.
    let zero = 0u32.to_le_bytes();
    let mut r = Reader::new(&zero);
    assert!(matches!(r.take_bool(), Err(Error::UnexpectedId { .. })));

    // Invalid UTF-8 in a string.
    let bytes = encode(|w| w.put_bytes(&[0xff, 0xfe]));
    let mut r = Reader::new(&bytes);
    assert_eq!(r.take_string().unwrap_err(), Error::InvalidUtf8);

    // Negative vector count.
    let mut bytes = TYPE_VECTOR.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(-5i32).to_le_bytes());
    let mut r = Reader::new(&bytes);
    assert_eq!(
        r.take_vector_header().unwrap_err(),
        Error::NegativeVectorCount(-5)
    );

    // Wrong constructor id.
    let mut r = Reader::new(&[0, 0, 0, 0]);
    assert!(matches!(
        r.consume_id(TYPE_VECTOR),
        Err(Error::UnexpectedId { .. })
    ));
}

#[test]
fn peek_does_not_advance() {
    let bytes = TYPE_VECTOR.to_le_bytes();
    let r = Reader::new(&bytes);
    assert_eq!(r.peek_id().unwrap(), TYPE_VECTOR);
    assert_eq!(r.position(), 0);
}

#[test]
fn patch_and_borrow_escape_hatches() {
    // patch_u32 rewrites a length placeholder in place after the payload is known.
    let mut w = Writer::new();
    w.put_u32(0);
    let payload = [9u8, 8, 7, 6];
    w.put_raw(&payload);
    w.patch_u32(0, payload.len() as u32);
    assert_eq!(w.as_slice(), &[4, 0, 0, 0, 9, 8, 7, 6]);

    // Decoding borrows the input; an owned copy is available on demand.
    let src = [9u8, 8, 7, 6];
    let mut r = Reader::new(&src);
    let borrowed = r.take_raw(4).unwrap();
    assert_eq!(borrowed, &src[..]);
    let owned: Vec<u8> = borrowed.to_vec();
    assert_eq!(owned, src.to_vec());
}
