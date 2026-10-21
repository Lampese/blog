mod fixtures;

use blog_ranges::{Byte, Scalar, TextRange, Utf16};

#[test]
fn every_text_boundary_round_trips_between_byte_scalar_and_utf16_units() {
    let text = fixtures::text("emoji_combining_document");
    let boundaries: Vec<_> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    for &start in &boundaries {
        for &end in boundaries.iter().filter(|&&end| end >= start) {
            let bytes = TextRange::<Byte>::new(start..end);
            let scalars = bytes.convert::<Scalar>(text).unwrap();
            let utf16 = bytes.convert::<Utf16>(text).unwrap();
            assert_eq!(bytes.slice(text).unwrap(), scalars.slice(text).unwrap());
            assert_eq!(bytes.slice(text).unwrap(), utf16.slice(text).unwrap());
            assert_eq!(scalars.convert::<Byte>(text).unwrap(), bytes);
            assert_eq!(utf16.convert::<Scalar>(text).unwrap(), scalars);
        }
    }
    for offset in 0..text.len() {
        if !text.is_char_boundary(offset) {
            assert!(
                TextRange::<Byte>::new(offset..text.len())
                    .convert::<Scalar>(text)
                    .is_err()
            );
        }
    }
    let utf16_boundaries: Vec<_> = boundaries
        .iter()
        .map(|&byte| text[..byte].encode_utf16().count())
        .collect();
    for offset in 0..text.encode_utf16().count() {
        if !utf16_boundaries.contains(&offset) {
            assert!(
                TextRange::<Utf16>::new(0..offset)
                    .convert::<Scalar>(text)
                    .is_err()
            );
            assert!(
                TextRange::<Utf16>::new(offset..text.encode_utf16().count())
                    .slice(text)
                    .is_err()
            );
        }
    }
    assert!(
        TextRange::<Scalar>::new(usize::MAX..usize::MAX)
            .slice(text)
            .is_err()
    );
    assert!(
        TextRange::<Scalar>::new(std::ops::Range { start: 2, end: 1 })
            .convert::<Utf16>(text)
            .is_err()
    );
}

#[test]
fn browser_ranges_keep_the_existing_wire_format_without_unit_fields() {
    let range = TextRange::<Utf16>::new(1..8);
    let json = serde_json::to_string(&range).unwrap();
    assert_eq!(json, r#"{"start":1,"end":8}"#);
    assert_eq!(
        serde_json::from_str::<TextRange<Utf16>>(&json).unwrap(),
        range
    );
}
