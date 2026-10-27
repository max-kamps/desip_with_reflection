use desip_with_reflection::JsonErrorKind::*;

macro_rules! assert_parses {
    ($input:expr, $ty:ident $constructor_args:tt) => {
        let data: $ty = desip_with_reflection::parse_slice($input).unwrap();
        assert_eq!(data, $ty $constructor_args);
    };
}

macro_rules! assert_errors {
    ($input:expr, $ty:ident, $error_kind:expr) => {
        let result = desip_with_reflection::parse_slice::<$ty>($input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, $error_kind);
    };
}

#[derive(Debug, PartialEq)]
struct Inner {
    x: u32,
    y: u16,
    z: u8,
}

#[test]
fn simple_struct() {
    assert_parses!(
        br#"{
            "x": 43,
            "y": 65534,
            "z": 254
        }"#,
        Inner {
            x: 43,
            y: 65534,
            z: 254,
        }
    );
}

#[test]
fn simple_fail_duplicate_field() {
    assert_errors!(
        br#"{
            "x": 43,
            "y": 65534,
            "z": 254,
            "x": 44
        }"#,
        Inner,
        DuplicateField
    );
}

#[test]
fn simple_fail_missing_field() {
    assert_errors!(
        br#"{
            "x": 43,
            "y": 65534
        }"#,
        Inner,
        MissingField
    );
}

#[derive(Debug, PartialEq)]
struct Nested {
    a: u32,
    b: u16,
    c: u8,
    inner: Inner,
}

#[test]
fn nested_structs() {
    assert_parses!(
        br#"{
            "a": 42,
            "b": 65535,
            "c": 255,
            "inner": {
                "x": 43,
                "y": 65534,
                "z": 254
            }
        }"#,
        Nested {
            a: 42,
            b: 65535,
            c: 255,
            inner: Inner {
                x: 43,
                y: 65534,
                z: 254,
            },
        }
    );
}
