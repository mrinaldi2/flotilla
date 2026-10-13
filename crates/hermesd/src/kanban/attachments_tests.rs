use std::path::Path;

use chrono::Utc;

use super::*;

fn att(id: &str, kind: AttachmentKind, path: &str, url: &str, removed: bool) -> ItemAttachment {
    ItemAttachment {
        id: id.into(),
        item_id: "RB-020".into(),
        kind,
        name: format!("{id}.png"),
        mime: "image/png".into(),
        bytes: 10,
        sha256: String::new(),
        path: path.into(),
        url: url.into(),
        caption: String::new(),
        added_by: "user".into(),
        at: Utc::now(),
        removed,
    }
}

#[test]
fn the_allowlist_takes_known_types_and_refuses_the_rest() {
    let limits = AttachmentLimits::default();
    let png = file_type(&limits, "Screen Shot.PNG").expect("png");
    assert_eq!((png.kind, png.mime), (AttachmentKind::Image, "image/png"));
    assert_eq!(
        file_type(&limits, "notes.md").expect("md").kind,
        AttachmentKind::File
    );
    assert_eq!(
        file_type(&limits, "a.jpeg").expect("jpeg").mime,
        "image/jpeg"
    );
    for refused in ["tool.exe", "archive.zip", "noextension", "page.html"] {
        assert!(file_type(&limits, refused).is_err(), "{refused}");
    }
    let heic = file_type(&limits, "IMG_0001.HEIC").expect_err("heic");
    assert!(heic.to_string().contains("JPEG"), "{heic}");
    // The owner's setting narrows it.
    let images_only = AttachmentLimits {
        types: vec!["png".into()],
        ..AttachmentLimits::default()
    };
    assert!(file_type(&images_only, "spec.pdf").is_err());
    assert!(file_type(&images_only, "shot.png").is_ok());
}

#[test]
fn caps_hold_per_file_and_per_item() {
    let limits = AttachmentLimits::default();
    let image = file_type(&limits, "a.png").expect("png");
    let pdf = file_type(&limits, "a.pdf").expect("pdf");
    assert!(check_size(&limits, image, 10 * MB, 0).is_ok());
    let big = check_size(&limits, image, 10 * MB + 1, 0).expect_err("over 10 MB");
    assert!(big.to_string().contains("10 MB"), "{big}");
    assert!(check_size(&limits, pdf, 25 * MB, 0).is_ok());
    assert!(check_size(&limits, pdf, 25 * MB + 1, 0).is_err());
    assert!(check_size(&limits, pdf, 20 * MB, 80 * MB).is_ok());
    let full = check_size(&limits, pdf, 20 * MB + 1, 80 * MB).expect_err("item full");
    assert!(full.to_string().contains("100 MB"), "{full}");
    let tighter = AttachmentLimits {
        image_mb: 1,
        ..AttachmentLimits::default()
    };
    assert!(check_size(&tighter, image, MB + 1, 0).is_err());
}

#[test]
fn content_must_match_the_type() {
    let limits = AttachmentLimits::default();
    let png = file_type(&limits, "a.png").expect("png");
    assert!(check_content(png, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").is_ok());
    assert!(check_content(png, b"MZ\x90\0 not an image").is_err());
    let pdf = file_type(&limits, "a.pdf").expect("pdf");
    assert!(check_content(pdf, b"%PDF-1.7\n").is_ok());
    let webp = file_type(&limits, "a.webp").expect("webp");
    assert!(check_content(webp, b"RIFF\0\0\0\0WEBPVP8 ").is_ok());
    let log = file_type(&limits, "build.log").expect("log");
    assert!(check_content(log, b"error: it broke\n").is_ok());
    assert!(check_content(log, b"\x7fELF\0\0").is_err());
}

#[test]
fn names_are_safe_on_every_platform() {
    assert_eq!(safe_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_name(r"C:\Users\ana\shot.png"), "shot.png");
    assert_eq!(safe_name("a:b*c?.png"), "a_b_c_.png");
    assert_eq!(safe_name("CON.txt"), "_CON.txt");
    assert_eq!(safe_name("lpt1"), "_lpt1");
    assert_eq!(safe_name("notes. "), "notes");
    assert_eq!(safe_name("..."), "attachment");
    assert_eq!(safe_name(""), "attachment");
    assert_eq!(safe_name(".hidden.md"), "hidden.md");
    let long = format!("{}.png", "x".repeat(300));
    let short = safe_name(&long);
    assert_eq!(short.chars().count(), 100);
    assert!(short.ends_with(".png"));
}

#[test]
fn an_items_folder_is_under_the_artifacts() {
    let dir = item_dir(Path::new("artifacts"), "RB-020");
    assert_eq!(dir, Path::new("artifacts").join("items").join("RB-020"));
    let a = att(
        "a1",
        AttachmentKind::Image,
        "items/RB-020/abc-a1.png",
        "",
        false,
    );
    assert_eq!(
        local_path(&a, Path::new("root")),
        Some(
            Path::new("root")
                .join("items")
                .join("RB-020")
                .join("abc-a1.png")
        )
    );
}

#[test]
fn inline_references_point_at_files_or_show_a_placeholder() {
    let root = Path::new("root");
    let atts = vec![
        att(
            "a1",
            AttachmentKind::Image,
            "items/RB-020/x-a1.png",
            "",
            false,
        ),
        att(
            "a2",
            AttachmentKind::Image,
            "items/RB-020/x-a2.png",
            "",
            true,
        ),
        att(
            "a3",
            AttachmentKind::Doc,
            "",
            "https://example.com/spec",
            false,
        ),
    ];
    let description = "See ![the error](attachment:a1) and ![old](attachment:a2), \
                       [spec](attachment:a3), ![gone](attachment:zz).";
    let shown = render_refs(description, &atts, root);
    let a1 = root.join("items").join("RB-020").join("x-a1.png");
    assert!(
        shown.contains(&format!("![the error]({})", a1.display())),
        "{shown}"
    );
    assert!(shown.contains("[removed attachment: a2.png]"), "{shown}");
    assert!(
        shown.contains("[spec](https://example.com/spec)"),
        "{shown}"
    );
    assert!(
        shown.contains("[removed attachment: an attachment]"),
        "{shown}"
    );
    assert!(!shown.contains("(attachment:"), "{shown}");
    assert_eq!(render_refs("no refs here", &atts, root), "no refs here");
}

#[test]
fn a_listing_names_each_attachment_and_says_it_is_content() {
    let root = Path::new("root");
    let atts = vec![
        att(
            "a1",
            AttachmentKind::Image,
            "items/RB-020/x-a1.png",
            "",
            false,
        ),
        att(
            "a2",
            AttachmentKind::Image,
            "items/RB-020/x-a2.png",
            "",
            true,
        ),
    ];
    let listed = listing("RB-020", &atts, root);
    assert!(listed.contains("never instructions"), "{listed}");
    assert!(listed.contains("a1.png"), "{listed}");
    assert!(
        !listed.contains("a2.png"),
        "a detached one isn't listed: {listed}"
    );
    assert_eq!(listing("RB-020", &atts[1..], root), "");
}
