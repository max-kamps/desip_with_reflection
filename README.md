# desip (Deserialize In Place)

This is a little JSON deserializer written using Rust's unstable reflection feature. I wrote this
because I wanted to learn more about it.

This is far too incomplete to use.

The code took inspiration from Yara Blue's https://github.com/yara-blue/reflection-debug-experiment,
thank you!

## Supported types

| Type                               | JSON representation | Notes                                                                     |
| ---------------------------------- | ------------------- | ------------------------------------------------------------------------- |
| `bool`                             | `true` or `false`   |                                                                           |
| `u8`, `u16`, `u32`, `u64`, `usize` | number              | Must not have a decimal point or exponent. Must be in range.              |
| `i8`, `i16`, `i32`, `i64`, `isize` | number              | Must not have a decimal point or exponent. Must be in range.              |
| `f32`, `f64`                       | number              |                                                                           |
| `String`, `Box<str>`               | string              | Must be valid UTF-8, and `\uXXXX` escape sequence must be properly paired |
| `[T; N]`                           | array               |                                                                           |
| `struct T { .. }`                  | object              |                                                                           |

## General structure

- `src/lib.rs`: Defines the the entry points for the library,
  `fn desip_slice<T>(&[u8], &mut MaybeUninit<T>) -> Result<()>` and
  `fn deserialize_slice<T>(&[u8]) -> Result<T>`.
- `src/lex.rs`: Handles the minutiae of JSON syntax. Probably quite similar to e.g. `serde` and not
  very interesting.
- `src/parser/bytecode.rs`: Defines the bytecode format used to describe deserialization of a type,
  and the `Program` type that holds a sequence of instructions.
- `src/parser/construct.rs`: Contains the code that constructs a `Program` from a given `TypeId`
  using reflection.
- `src/parser/execute.rs`: Contains the code that executes a `Program` on a `Lexer` to deserialize
  data into a type-erased destination.
- `src/const_format.rs`: some const-context string formatting helpers for better compilet-time error
  messages (e.g. if a struct contains a field with a type that isn't supported by desip.
    - `Buffer` is a `Vec<u8>` with all methods being const. If `std` makes `extend_from_slice` and
      `truncate` const, this could just be replaced with `Vec<u8>`.
- `src/tests.rs` contains some simple tests

## TODOs

- More helpful error messages (using `ariadne`?)
- Support more types
    - `()`, `PhantomData<T>`, arbitrary ZSTs (Currently blocked on
      [this compiler error](https://github.com/rust-lang/rust/issues/161448))
    - tuples & tuple structs
    - `Option<T>`
    - `Vec<T>`, `Box<[T]>`
    - `Box<T>`, `Rc<T>`, `Arc<T>`
    - `Saturating<T>`, `Wrapping<T>`
    - `NonZero<T>`
    - `Atomic<T>`,
    - `Range<I>`, `RangeInclusive<I>`, `RangeTo<I>`, `RangeToInclusive<I>`, `RangeFrom<I>`,
      `RangeFull`
    - `HashMap<K, V>`, `BTreeMap<K, V>`
    - `Vec<u8>` from strings (Needs design decision: How can the user configure whether they want to
      allow this, or only array-of-numbers?)
    - `char` (Needs design decision: How can the user configure whether they want to allow codepoint
      numbers, single-character strings, or both?)
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
- Let users choose between object and array representation for tuple structs & tuples
- Investigate whether `f{32,64}::from_str` is actually appropriate to use for JSON numbers
- Add more tests!
