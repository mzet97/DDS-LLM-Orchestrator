use super::{fmt_int, short_call_id};

#[test]
fn fmt_int_groups_thousands() {
    assert_eq!(fmt_int(840), "840");
    assert_eq!(fmt_int(1_791_257_507_167), "1,791,257,507,167");
}

#[test]
fn short_call_id_keeps_short_and_clips_long() {
    assert_eq!(short_call_id("aa-1111"), "aa-1111");
    assert_eq!(short_call_id("aa-11111111-0001"), "111-0001");
    assert_eq!(short_call_id("probe-1a10f44542a"), "0f44542a");
    assert_eq!(short_call_id("probe-1a10f447561"), "0f447561");
}
