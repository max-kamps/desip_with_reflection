#[derive(Debug, PartialEq)]
struct Data {
    a: u32,
    b: u16,
    c: u8,
    inner: Inner,
}

#[derive(Debug, PartialEq)]
struct Inner {
    x: u32,
    y: u16,
    z: u8,
}

#[test]
fn simple_struct() {
    let test_string = br#"{
        "x": 43,
        "y": 65534,
        "z": 254
    }"#;
    let test_data = Inner {
        x: 43,
        y: 65534,
        z: 254,
    };

    let data: Inner = desip_with_reflection::parse_slice(test_string).unwrap();

    println!("{:#?}", data);
    assert_eq!(data, test_data);
}

#[test]
fn nested_structs() {
    let test_string = br#"{
        "a": 42,
        "b": 65535,
        "c": 255,
        "inner": {
            "x": 43,
            "y": 65534,
            "z": 254
        }
    }"#;
    let test_data = Data {
        a: 42,
        b: 65535,
        c: 255,
        inner: Inner {
            x: 43,
            y: 65534,
            z: 254,
        },
    };

    let data: Data = desip_with_reflection::parse_slice(test_string).unwrap();

    println!("{:#?}", data);
    assert_eq!(data, test_data);
}
