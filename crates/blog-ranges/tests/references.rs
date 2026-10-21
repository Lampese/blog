mod fixtures;

use blog_ranges::{RangeDocument, RangeReference, ReferenceError};
use std::{fs, process::Command};

fn select(doc: &mut RangeDocument, needle: &str) -> RangeReference {
    let source = doc.source();
    let byte_start = source.find(needle).unwrap();
    let start = source[..byte_start].chars().count();
    doc.register(start..start + needle.chars().count()).unwrap()
}

#[test]
fn a_copied_reference_stays_constant_while_its_range_tracks_edits() {
    let mut doc = RangeDocument::new("article-a", fixtures::text("bounded_word")).unwrap();
    let reference = select(&mut doc, fixtures::text("word"));
    let copied = reference.to_typst_metadata();
    doc.update_source(fixtures::text("boundary_insertions"))
        .unwrap();
    assert_eq!(
        doc.resolve(&reference).unwrap().text,
        fixtures::text("word")
    );
    doc.update_source(fixtures::text("interior_insertion"))
        .unwrap();
    assert_eq!(
        doc.resolve(&reference).unwrap().text,
        fixtures::text("interior_selection")
    );
    assert_eq!(reference.to_typst_metadata(), copied);
}

#[test]
fn overlapping_references_are_independent_and_keep_their_own_boundaries() {
    let mut doc = RangeDocument::new("a", "AabcdefB").unwrap();
    let first = select(&mut doc, "abcd");
    let second = select(&mut doc, "cdef");
    assert_ne!(first.range, second.range);
    doc.update_source("AabXcdefB").unwrap();
    assert_eq!(doc.resolve(&first).unwrap().text, "abXcd");
    assert_eq!(doc.resolve(&second).unwrap().text, "cdef");
}

#[test]
fn deleting_one_overlapping_range_does_not_destroy_the_other() {
    let mut doc = RangeDocument::new("a", "AabcdefB").unwrap();
    let first = select(&mut doc, "abcd");
    let second = select(&mut doc, "cdef");
    doc.update_source("AefB").unwrap();
    assert!(matches!(
        doc.resolve(&first),
        Err(ReferenceError::DeletedRange(_))
    ));
    assert_eq!(doc.resolve(&second).unwrap().text, "ef");
}

#[test]
fn the_selected_second_occurrence_keeps_its_identity() {
    let mut doc = RangeDocument::new("a", fixtures::text("repeated_sentence")).unwrap();
    let reference = doc.register(8..12).unwrap();
    doc.update_source(fixtures::text("prefixed_repeated_sentence"))
        .unwrap();
    let selection = doc.resolve(&reference).unwrap();
    assert_eq!(selection.unicode_range, 11..15);
    assert_eq!(selection.text, fixtures::text("repeated_selection"));
}

#[test]
fn a_deleted_range_stays_invalid_after_new_text_and_snapshot_restore() {
    let mut doc = RangeDocument::new("a", fixtures::text("bounded_word")).unwrap();
    let reference = select(&mut doc, fixtures::text("word"));
    doc.update_source("AB").unwrap();
    assert!(matches!(
        doc.resolve(&reference),
        Err(ReferenceError::DeletedRange(_))
    ));
    doc.update_source(fixtures::text("replacement")).unwrap();
    assert!(matches!(
        doc.resolve(&reference),
        Err(ReferenceError::DeletedRange(_))
    ));
    let restored = RangeDocument::from_snapshot(&doc.state_snapshot().unwrap()).unwrap();
    assert!(matches!(
        restored.resolve(&reference),
        Err(ReferenceError::DeletedRange(_))
    ));
}

#[test]
fn deleting_all_source_does_not_reattach_old_references_to_a_new_document_body() {
    let mut doc = RangeDocument::new("a", fixtures::text("original_document")).unwrap();
    let reference = select(&mut doc, fixtures::text("original_document"));
    doc.update_source("").unwrap();
    assert!(matches!(
        doc.resolve(&reference),
        Err(ReferenceError::DeletedRange(_))
    ));
    doc.update_source(fixtures::text("replaced_document"))
        .unwrap();
    assert!(matches!(
        doc.resolve(&reference),
        Err(ReferenceError::DeletedRange(_))
    ));
    let new_reference = select(&mut doc, fixtures::text("replacement_selection"));
    assert_eq!(
        doc.resolve(&new_reference).unwrap().text,
        fixtures::text("replacement_selection")
    );
}

#[test]
fn document_end_is_a_regular_nonexpanding_boundary() {
    let mut doc = RangeDocument::new("a", fixtures::text("word_at_end")).unwrap();
    let reference = select(&mut doc, fixtures::text("word"));
    doc.update_source(fixtures::text("appended_tail")).unwrap();
    assert_eq!(
        doc.resolve(&reference).unwrap().text,
        fixtures::text("word")
    );
}

#[test]
fn source_utf16_offsets_preserve_emoji_and_combining_marks() {
    let mut doc = RangeDocument::new("a", fixtures::text("emoji_combining_document")).unwrap();
    let reference = doc.register_utf16(1..8).unwrap();
    doc.update_source(fixtures::text("prefixed_emoji_document"))
        .unwrap();
    assert_eq!(
        doc.resolve(&reference).unwrap().text,
        fixtures::text("emoji_selection")
    );
    assert!(doc.register_utf16(7..8).is_err());
}

#[test]
fn concurrent_source_changes_converge_without_rewriting_reference_metadata() {
    let mut left = RangeDocument::new("a", fixtures::text("registered_sentence")).unwrap();
    let reference = select(&mut left, fixtures::text("registered_selection"));
    let mut right = left.fork();
    left.update_source(fixtures::text("prefixed_registered_sentence"))
        .unwrap();
    right
        .update_source(fixtures::text("edited_registered_sentence"))
        .unwrap();
    left.merge(&right).unwrap();
    right.merge(&left).unwrap();
    assert_eq!(left.source(), right.source());
    assert_eq!(
        left.resolve(&reference).unwrap(),
        right.resolve(&reference).unwrap()
    );
    assert_eq!(
        left.resolve(&reference).unwrap().text,
        fixtures::text("edited_registered_selection")
    );
}

#[test]
fn concurrently_registered_overlapping_ranges_do_not_overwrite_each_other() {
    let mut left = RangeDocument::new("a", "AabcdefB").unwrap();
    let mut right = left.fork();
    let first = select(&mut left, "abcd");
    let second = select(&mut right, "cdef");
    assert_ne!(first.range, second.range);
    left.merge(&right).unwrap();
    right.merge(&left).unwrap();
    for doc in [&left, &right] {
        assert_eq!(doc.resolve(&first).unwrap().text, "abcd");
        assert_eq!(doc.resolve(&second).unwrap().text, "cdef");
    }
}

#[test]
fn latest_state_snapshot_retains_marks_and_the_same_reference_keeps_working() {
    let mut doc = RangeDocument::new("a", "AabcdefB").unwrap();
    let first = select(&mut doc, "abcd");
    let second = select(&mut doc, "cdef");
    doc.update_source("AabXcdefB").unwrap();
    let bytes = doc.state_snapshot().unwrap();
    drop(doc);
    let mut restored = RangeDocument::from_snapshot(&bytes).unwrap();
    assert_eq!(restored.resolve(&first).unwrap().text, "abXcd");
    assert_eq!(restored.resolve(&second).unwrap().text, "cdef");
    restored.update_source("AabXcdYYefB").unwrap();
    assert_eq!(restored.resolve(&first).unwrap().text, "abXcd");
    assert_eq!(restored.resolve(&second).unwrap().text, "cdYYef");
    let again = RangeDocument::from_snapshot(&restored.state_snapshot().unwrap()).unwrap();
    assert_eq!(again.resolve(&second).unwrap().text, "cdYYef");
}

#[test]
fn unknown_ranges_wrong_articles_and_future_schemas_are_distinct_errors() {
    let mut doc = RangeDocument::new("a", fixtures::text("bounded_word")).unwrap();
    let reference = select(&mut doc, fixtures::text("word"));
    let mut unknown = reference.clone();
    unknown.range = "missing".into();
    assert!(matches!(
        doc.resolve(&unknown),
        Err(ReferenceError::UnknownRange(_))
    ));
    let mut wrong_article = reference.clone();
    wrong_article.article = "b".into();
    assert!(matches!(
        doc.resolve(&wrong_article),
        Err(ReferenceError::WrongArticle)
    ));
    let mut future = reference;
    future.schema = 999;
    assert!(matches!(
        doc.resolve(&future),
        Err(ReferenceError::UnsupportedSchema(999))
    ));
}

#[test]
fn independent_documents_with_the_same_title_cannot_be_merged_as_shared_history() {
    let mut left = RangeDocument::new("a", fixtures::text("left_branch")).unwrap();
    let right = RangeDocument::new("a", fixtures::text("right_branch")).unwrap();
    assert!(matches!(
        left.merge(&right),
        Err(ReferenceError::UnrelatedHistory)
    ));
    assert_eq!(left.source(), fixtures::text("left_branch"));
}

#[test]
fn copied_metadata_round_trips_through_typst_and_resolves_after_restore() {
    let mut doc = RangeDocument::new(
        fixtures::text("quoted_article_id"),
        fixtures::text("registered_sentence"),
    )
    .unwrap();
    let reference = select(&mut doc, fixtures::text("registered_selection"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reference.typ");
    fs::write(&path, reference.to_typst_metadata()).unwrap();
    let output = Command::new("typst")
        .args(["eval", "query(<blog-reference>).first().value", "--in"])
        .arg(&path)
        .output()
        .expect("this experiment needs typst 0.15+ on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let copied: RangeReference = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(copied, reference);
    let restored = RangeDocument::from_snapshot(&doc.state_snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.resolve(&copied).unwrap().text,
        fixtures::text("registered_selection")
    );
}
