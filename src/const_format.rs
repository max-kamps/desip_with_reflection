pub const trait ConstFormat {
    fn const_fmt(&self) -> &str;
}

const impl ConstFormat for &str {
    fn const_fmt(&self) -> &str {
        self
    }
}

pub const fn const_fmt(fmt: &str, mut args: &[&str]) -> Vec<u8> {
    let fmt = fmt.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c == b'{' {
            if i + 1 >= fmt.len() || fmt[i + 1] != b'}' {
                panic!("Unmatched '{{' in format string");
            }
            if args.is_empty() {
                panic!("Not enough arguments for format string");
            }
            let bytes = args[0].as_bytes();
            #[allow(clippy::needless_range_loop)] // slice isn't const iter yet
            for bi in 0..args[0].len() {
                result.push(bytes[bi]);
            }
            args = &args[1..];
            i += 2;
        } else {
            result.push(c);
            i += 1;
        }
    }
    result
}

pub macro const_format($fmt:literal $(, $arg:expr)* $(,)?) {{
    let mut args: Vec<&str> = Vec::new();
    $(
        args.push($arg.const_fmt());
    )*
    let bytes = const_fmt($fmt, &args).const_make_global();
    unsafe { str::from_utf8_unchecked(bytes) }
}}

pub macro const_panic($fmt:literal $(, $arg:expr)* $(,)?) {{
    panic!("{}", const_format!($fmt $(, $arg)*));
}}
