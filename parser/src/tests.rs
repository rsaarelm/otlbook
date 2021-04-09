use crate::{from_outline, outline, outline2::Outline2};
use pretty_assertions::assert_eq;
use serde::{de, Deserialize, Serialize};
use std::fmt;
use std::{collections::HashMap, iter::FromIterator};

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
        tags: Vec<String>,
    }

    test(
        outline!["title: xyzzy", "one-two: 3", "num: 123", "flag: true"],
        Struct {
            title: "xyzzy".to_string(),
            one_two: 3,
            num: 123,
            flag: true,
            ..Default::default()
        },
    );

    // Defaults get filled in.
    test(
        outline!["title: xyzzy", "num: 123"],
        Struct {
            title: "xyzzy".to_string(),
            num: 123,
            ..Default::default()
        },
    );

    // Tags is parsed differently because it's a sequence type.
    test(
        outline!["title: foo bar", "tags: foo bar"],
        Struct {
            title: "foo bar".to_string(),
            tags: vec!["foo".to_string(), "bar".to_string()],
            ..Default::default()
        },
    );

    test(
        outline!["title: xyzzy", "num: 123", "some: junk"],
        Struct {
            title: "xyzzy".to_string(),
            num: 123,
            ..Default::default()
        },
    );
}

#[test]
fn test_seq() {
    test(outline!["1", "2", "3", "4"], vec![1u32, 2, 3, 4]);

    test(
        outline!["foo", "bar", "baz"],
        vec!["foo".to_string(), "bar".to_string(), "baz".to_string()],
    );

    test(
        outline![["head line", "body1"], ["head2", "body2"]],
        vec![
            ("head line".to_string(), "body1".to_string()),
            ("head2".to_string(), "body2".to_string()),
        ],
    );
    test(
        outline![["map line", "body1"], ["head2", "body2"]],
        HashMap::<String, String>::from_iter(
            vec![
                ("map line".to_string(), "body1".to_string()),
                ("head2".to_string(), "body2".to_string()),
            ]
            .into_iter(),
        ),
    );
}
