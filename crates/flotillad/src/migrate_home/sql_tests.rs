use super::{is_under, msys_spelling, rewrite, rewrite_as};

#[test]
fn rewrites_whole_paths_and_prefixes_only() {
    let (from, to) = ("/Users/u/.gravity", "/Users/u/.flotilla");
    assert_eq!(
        rewrite("/Users/u/.gravity/projects/p/bots/b/workspace", from, to),
        "/Users/u/.flotilla/projects/p/bots/b/workspace"
    );
    assert_eq!(rewrite("/Users/u/.gravity", from, to), to);
    assert_eq!(
        rewrite("see /Users/u/.gravity/a and /Users/u/.gravity/b", from, to),
        "see /Users/u/.flotilla/a and /Users/u/.flotilla/b"
    );
    assert_eq!(
        rewrite("/Users/u/.gravity-old/x", from, to),
        "/Users/u/.gravity-old/x"
    );
    assert_eq!(
        rewrite(
            r"C:\Users\u\.gravity\projects",
            r"C:\Users\u\.gravity",
            r"C:\Users\u\.flotilla"
        ),
        r"C:\Users\u\.flotilla\projects"
    );
}

#[test]
fn a_loose_rewrite_takes_every_windows_spelling() {
    let (from, to) = (r"C:\Users\u\.gravity", r"C:\Users\u\.flotilla");
    for (text, want) in [
        (r"C:\Users\u\.gravity\p", r"C:\Users\u\.flotilla\p"),
        ("C:/Users/u/.gravity/p", "C:/Users/u/.flotilla/p"),
        (r"c:\users\U\.Gravity\p", r"C:\Users\u\.flotilla\p"),
        (r"C:/Users\u/.gravity", r"C:\Users\u\.flotilla"),
        (r"C:\Users\u\.gravity-old\p", r"C:\Users\u\.gravity-old\p"),
    ] {
        assert_eq!(rewrite_as(text, from, to, true), want, "{text}");
    }
    // Exact matching (macOS, Linux) leaves the variants alone.
    assert_eq!(
        rewrite_as("C:/Users/u/.gravity/p", from, to, false),
        "C:/Users/u/.gravity/p"
    );
    let (from, to) = (
        msys_spelling(from).expect("msys"),
        msys_spelling(to).expect("msys"),
    );
    assert_eq!(from, "/c/Users/u/.gravity");
    assert_eq!(
        rewrite_as("cd /c/users/u/.gravity/p", &from, &to, true),
        "cd /c/Users/u/.flotilla/p"
    );
    assert_eq!(msys_spelling("/Users/u/.gravity"), None);
}

#[test]
fn any_spelling_under_the_home_counts_as_under_it() {
    let home = "/Users/u/.gravity";
    assert!(is_under("/Users/u/.gravity", home));
    assert!(is_under("/users/U/.GRAVITY/projects/x", home));
    assert!(is_under(r"C:\Users\u\.gravity\p", r"c:/users/u/.gravity"));
    assert!(!is_under("/Users/u/.gravity-old/x", home));
    assert!(!is_under("/Users/u/.flotilla/x", home));
}
