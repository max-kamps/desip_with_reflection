use desip_with_reflection::{JsonError, JsonErrorKind::*, parse_slice};

macro_rules! assert_parses {
    ($input:expr, $ty:ident $constructor_args:tt) => {
        let data: Result<$ty, JsonError> = parse_slice($input);
        match data {
            Ok(data) => assert_eq!(data, $ty $constructor_args),
            Err(err) => panic!("Parsing failed: {err}"),
        };
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

#[derive(Debug, PartialEq)]
struct AllPrimitives {
    unit: (),
    bool: bool,
    u8: u8,
    u16: u16,
    u32: u32,
    u64: u64,
    u128: u128,
    i8: i8,
    i16: i16,
    i32: i32,
    i64: i64,
    i128: i128,
    // char: char,
    f32: f32,
    f64: f64,
}

#[test]
fn all_primitives() {
    assert_parses!(
        br#"{
            "unit": null,
            "bool": true,
            "u8": 255,
            "u16": 65535,
            "u32": 4294967295,
            "u64": 18446744073709551615,
            "u128": 340282366920938463463374607431768211455,
            "i8": -128,
            "i16": -32768,
            "i32": -2147483648,
            "i64": -9223372036854775808,
            "i128": -170141183460469231731687303715884105728,
            "f32": 3.4028235e38,
            "f64": 1.7976931348623157e308
        }"#,
        AllPrimitives {
            unit: (),
            bool: true,
            u8: 255,
            u16: 65535,
            u32: 4294967295,
            u64: 18446744073709551615,
            u128: 340282366920938463463374607431768211455,
            i8: -128,
            i16: -32768,
            i32: -2147483648,
            i64: -9223372036854775808,
            i128: -170141183460469231731687303715884105728,
            f32: 3.4028235e38,
            f64: 1.7976931348623157e308,
        }
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
