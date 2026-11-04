//! Round-trip test for the trace wrapper.

use rtd_bin::{Reader, Writer};
use rtd_tg::{HelpGetConfig, HelpGetNearestDc, Method};
use rtd_tgtrace::InvokeWithTrace;

#[test]
fn trace_wrapper_round_trips() {
    let value = InvokeWithTrace {
        trace_parent: Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned()),
        trace_state: None,
        baggage: Some("key=value".to_owned()),
        query: Method::HelpGetConfig(Box::new(HelpGetConfig)),
    };

    let mut writer = Writer::new();
    value.encode(&mut writer);
    let bytes = writer.into_vec();

    let mut reader = Reader::new(&bytes);
    let decoded = InvokeWithTrace::decode(&mut reader).expect("decode trace wrapper");
    assert_eq!(decoded, value);
}

#[test]
fn nested_query_round_trips_without_trace_context() {
    let value = InvokeWithTrace {
        trace_parent: None,
        trace_state: None,
        baggage: None,
        query: Method::HelpGetNearestDc(Box::new(HelpGetNearestDc)),
    };

    let mut writer = Writer::new();
    value.encode(&mut writer);
    let bytes = writer.into_vec();
    let mut reader = Reader::new(&bytes);
    assert_eq!(InvokeWithTrace::decode(&mut reader).unwrap(), value);
}
