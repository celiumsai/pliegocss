#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use pliego_css_compiler::{
    decode_style_ir, emit_css, encode_style_ir, lower_style, try_encode_style_identity,
};
use pliego_css_parser::parse_style_list;
use sha2::{Digest, Sha256};

const PAYLOAD_SHA256_START: usize = 8 + 2 + 2 + 2 + 16 + 16;
const PAYLOAD_OFFSET: usize = PAYLOAD_SHA256_START + 32;

fn valid_artifact() -> &'static Vec<u8> {
    static ARTIFACT: OnceLock<Vec<u8>> = OnceLock::new();
    ARTIFACT.get_or_init(|| {
        let parsed = parse_style_list(
            "flex items-center gap-4 p-4 md:grid dark:hover:bg-accent/20 w-[calc(100%-1rem)]",
        )
        .expect("binary fuzz seed must parse");
        let style = lower_style(&parsed).expect("binary fuzz seed must lower");
        encode_style_ir(&style).expect("binary fuzz seed must encode")
    })
}

fn candidate(bytes: &[u8]) -> Vec<u8> {
    if bytes.starts_with(b"valid-template") {
        return valid_artifact().clone();
    }
    let Some((&mode, mutations)) = bytes.split_first() else {
        return valid_artifact().clone();
    };
    if mode % 3 == 0 {
        return mutations.to_vec();
    }
    let mut artifact = valid_artifact().clone();
    for (index, byte) in mutations.iter().copied().enumerate() {
        let position = (index.wrapping_mul(17) + usize::from(byte)) % artifact.len();
        artifact[position] ^= byte.rotate_left((index % 8) as u32);
    }
    if mode % 3 == 2 && artifact.len() >= PAYLOAD_OFFSET {
        let digest: [u8; 32] = Sha256::digest(&artifact[PAYLOAD_OFFSET..]).into();
        artifact[PAYLOAD_SHA256_START..PAYLOAD_OFFSET].copy_from_slice(&digest);
    }
    artifact
}

fuzz_target!(|bytes: &[u8]| {
    let artifact = candidate(bytes);
    let Ok(style) = decode_style_ir(&artifact) else {
        return;
    };
    let reencoded = encode_style_ir(&style).expect("accepted artifact must re-encode");
    assert_eq!(reencoded, artifact);
    let decoded = decode_style_ir(&reencoded).expect("re-encoded artifact must decode");
    assert_eq!(
        try_encode_style_identity(&style),
        try_encode_style_identity(&decoded)
    );
    assert_eq!(emit_css(&style), emit_css(&decoded));
});
