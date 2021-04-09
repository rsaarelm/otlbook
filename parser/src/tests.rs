use crate::{from_outline, outline, outline2::Outline2};
use pretty_assertions::assert_eq;
use serde::{de, Deserialize, Serialize};
use std::fmt;

fn test<T: de::DeserializeOwned + Serialize + fmt::Debug + PartialEq>(
    outline: Outline2,
    value: T,
) {
    print!("\ntesting\n{:?}", outline);

    // Test deserialize

    let outline_value: T =
        from_outline(&outline).expect("Outline did not parse into value");

    assert_eq!(outline_value, value);

    // TODO: Test serialization
}

#[test]
fn test_primitive() {
    test(outline!["123"], 123u32);
    test(outline!["2.71828"], 2.71828f32);
    test(outline!["true"], true);
    test(outline!["false"], false);
    test(outline!["symbol"], "symbol".to_string());
    test(outline!["two words"], "two words".to_string());
}

#[test]
fn test_struct() {
    #[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
    #[serde(default)]
    struct Struct {
        title: String,
        one_two: i32,
        num: i32,
        flag: bool,
    }

    test(
        outline!["title: xyzzy", "one-two: 3", "num: 123", "flag: true"],
        Struct {
            title: "xyzzy".to_string(),
            one_two: 3,
            num: 123,
            flag: true,
        },
    );

    test(
        outline!["title: xyzzy", "num: 123"],
        Struct {
            title: "xyzzy".to_string(),
            one_two: 0,
            num: 123,
            flag: false,
        },
    );
}
