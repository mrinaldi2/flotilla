use super::*;

#[test]
fn stored_spellings_parse_back() {
    for kind in LinkKind::ALL {
        assert_eq!(LinkKind::parse(kind.as_str()), Some(*kind));
    }
    assert_eq!(Role::parse("reviewer.ux"), Some(Role::ReviewerUx));
    assert_eq!(Size::parse("XL"), None);
}
