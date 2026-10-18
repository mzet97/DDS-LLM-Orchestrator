use super::fmt_int;

#[test]
fn fmt_int_groups_thousands() {
    assert_eq!(fmt_int(0), "0");
    assert_eq!(fmt_int(98), "98");
    assert_eq!(fmt_int(1_840), "1,840");
    assert_eq!(fmt_int(2_080), "2,080");
    assert_eq!(fmt_int(1_000_000), "1,000,000");
}
