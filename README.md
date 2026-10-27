# desip (Deserialize In Place)

This is a little JSON deserializer written using Rust's unstable reflection feature. I wrote this
because I wanted to learn more about it.

This is far too incomplete to use.

The code took inspiration from Yara Blue's https://github.com/yara-blue/reflection-debug-experiment,
thank you!

## General structure

- `src/lex.rs`: JSON syntax handling
    - Quite similar to e.g. `serde`. Not very interesting.
    - defines the `Lexer` type
- `src/parse.rs`:
    - `parse_slice(slice: &[u8]) -> Result<T>` is the public entry point to the library
    - `Parser` is a data structure that describes how to deserialize a certain type
    - `Parser::new` is a comptime function that creates a `Parser` for a given `TypeId`
    - `Parser::parse_into(&self, lex: &mut Lexer, dest: *mut u8)` deserializes data from a lexer
      into some type-erased uninitalized destination.
- `src/tests.rs` contains some simple examples

## TODOs

- Support more types
    - `bool`
    - `char`
    - all integer types
    - all float types
    - `NonZero<T>`, `Atomic<T>`, `Saturating<T>`, `Wrapping<T>`
    - `String`, `Box<str>`
    - `Box<T>`, `Rc<T>`, `Arc<T>`
    - `Option<T>`
    - tuples
    - `Range<I>`, `RangeInclusive<I>`, `RangeTo<I>`, `RangeToInclusive<I>`, `RangeFrom<I>`,
      `RangeFull<I>`
    - `[T; N]`
    - `Box<[T]>`
    - `Vec<u8>` from strings
    - `Vec<T>`
    - `HashMap<K, V>`, `BTreeMap<K, V>`
    - `PhantomData<T>`
    - ...
- Support `enum`s
    - For now, we could just assume the enum tag has an offset of 0. This will be correct for all
      enums with non-Rust repr.
    - Not sure yet how to get enum discriminants, even for testing purposes
        - Maybe assume that discriminants match the order of the variants in the type definition?
          That should be fine for testing purposes.
- Add some feature to provide deserialization options like "rename" and "flatten"
    - Presumably this should be an extra argument supplied to Parser construction
    - Add support for adjacently-tagged and internally-tagged enums
    - Add support for untagged enums
