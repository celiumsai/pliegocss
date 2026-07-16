#![no_main]

use libfuzzer_sys::fuzz_target;
use pliego_css_compiler::{
    decode_style_ir, emit_css, encode_style_ir, lower_style, try_encode_style_identity,
};
use pliego_css_parser::{format_style_list, parse_style_list};

fuzz_target!(|bytes: &[u8]| {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };
    let first = parse_style_list(source);
    let second = parse_style_list(source);
    assert_eq!(first, second);

    let Ok(parsed) = first else {
        return;
    };
    let formatted = format_style_list(&parsed);
    let reparsed = parse_style_list(&formatted).expect("formatter must preserve accepted syntax");
    assert_eq!(format_style_list(&reparsed), formatted);

    let (first_style, second_style) = match (lower_style(&parsed), lower_style(&reparsed)) {
        (Ok(first_style), Ok(second_style)) => (first_style, second_style),
        (Err(first_error), Err(second_error)) => {
            assert_eq!(first_error.code, second_error.code);
            return;
        }
        (first_result, second_result) => panic!(
            "formatter changed semantic lowering acceptance: first={first_result:?}, second={second_result:?}"
        ),
    };
    assert_eq!(first_style.id, second_style.id);
    assert_eq!(
        try_encode_style_identity(&first_style),
        try_encode_style_identity(&second_style)
    );
    assert_eq!(emit_css(&first_style), emit_css(&second_style));

    let encoded = encode_style_ir(&first_style).expect("lowered style must encode");
    let decoded = decode_style_ir(&encoded).expect("encoded style must decode");
    assert_eq!(encode_style_ir(&decoded), Ok(encoded));
    assert_eq!(emit_css(&decoded), emit_css(&first_style));
});
