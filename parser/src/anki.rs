use anki_connect::Card;

/// Parse a cloze line into a set of Anki cards.
///
/// A cloze line must end in a period and have at least one `{{cloze}}` segment.
///
/// Eg. `The capital of Australia is {{Canberra}}.`
///
/// One card will be generated for each cloze where that cloze is occluded in the front of the
/// card.
pub fn parse_cloze(tags: &[String], s: impl AsRef<str>) -> Result<Vec<Card>, ()> {
    let s = s.as_ref();

    #[derive(Copy, Clone, Debug)]
    enum Fragment<'a> {
        Text(&'a str),
        Cloze(&'a str),
    }

    impl<'a> Fragment<'a> {
        fn is_cloze(self) -> bool {
            match self {
                Fragment::Cloze(_) => true,
                _ => false,
            }
        }

        fn as_str(self) -> &'a str {
            match self {
                Fragment::Cloze(s) => s,
                Fragment::Text(s) => s,
            }
        }
    }

    fn fragment(i: &str) -> Result<(Fragment<'_>, &str), &str> {
        /// Parse a cloze fragment
        fn cloze(i: &str) -> Result<(Fragment<'_>, &str), &str> {
            if let (true, Some(idx)) = (i.starts_with("{{"), i.find("}}")) {
                Ok((Fragment::Cloze(&i[2..idx]), &i[idx + 2..]))
            } else {
                Err(i)
            }
        }

        /// Parse a non-cloze text fragment
        fn text(i: &str) -> Result<(Fragment<'_>, &str), &str> {
            fn not_text(i: &str) -> bool {
                i.is_empty() || i == "." || i.starts_with("{{")
            }

            if not_text(i) {
                return Err(i);
            }

            // Scan ahead until we find the start of a non-text fragment.
            if let Some(end) =
                i.char_indices()
                    .find_map(|(idx, _)| if not_text(&i[idx..]) { Some(idx) } else { None })
            {
                Ok((Fragment::Text(&i[..end]), &i[end..]))
            } else {
                Err(i)
            }
        }

        cloze(i).or_else(|_| text(i))
    }

    let mut fragments = Vec::new();
    let mut input = s;

    loop {
        if let Ok((frag, rest)) = fragment(input) {
            fragments.push(frag);
            input = rest;
        } else if input == "." {
            // Hit the final period, everything is fragmentized.
            break;
        } else {
            // Something went wrong, can't parse this.
            return Err(());
        }
    }

    if fragments.len() < 2 {
        // Must have at least one cloze and non-cloze.
        return Err(());
    }

    // Normalize so that the sequence always starts with a text fragment.
    if fragments[0].is_cloze() {
        fragments.insert(0, Fragment::Text(""));
    }

    // Generate card fronts with each cloze elided.
    let mut cloze_fronts = Vec::new();
    for i in (1..fragments.len()).step_by(2) {
        debug_assert!(fragments[i].is_cloze() && !fragments[i - 1].is_cloze());
        let mut front = String::new();
        for (j, elt) in fragments.iter().enumerate() {
            if j != i {
                // Generating card for cloze that's the last item on the line, special
                // treatment for the element before.
                if i == fragments.len() - 1 && j == i - 1 {
                    let s = elt.as_str().trim_end();

                    if s.chars()
                        .rev()
                        .next()
                        .map_or(false, |c| c.is_alphanumeric())
                    {
                        front.push_str(&format!("{}...", s));
                    // Last character is alphanumeric, end with ellipsis.
                    } else {
                        front.push_str(s);
                    }
                } else {
                    front.push_str(elt.as_str());
                    if j == fragments.len() - 1 {
                        front.push_str(".");
                    }
                }
            } else if i != fragments.len() - 1 {
                // Just insert an ellipsis for clozes that aren't the last element.
                front.push_str("...");
            }
        }
        cloze_fronts.push(front);
    }

    let back = fragments
        .iter()
        .map(|c| c.as_str().to_string())
        .collect::<Vec<String>>()
        .join("")
        + ".";

    Ok(cloze_fronts
        .into_iter()
        .map(|f| Card {
            front: f,
            back: back.clone(),
            tags: tags.to_vec(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test(input: &str, back: &str, fronts: &[&str]) {
        let parsed = parse_cloze(&Vec::new(), input).unwrap();

        for (c, f) in parsed.iter().zip(fronts) {
            assert_eq!(c, &Card::new(*f, back, Vec::new() as Vec<String>));
        }
    }

    #[test]
    fn test_cloze_parse() {
        assert!(parse_cloze(&Vec::new(), "Zomg").is_err());
        assert!(parse_cloze(&Vec::new(), "Here's {{clozes}} but no terminating period").is_err());
        assert!(parse_cloze(&Vec::new(), "No clozes but it ends in a period.").is_err());

        test(
            "The capital of Foonia is {{Barston}}.",
            "The capital of Foonia is Barston.",
            &["The capital of Foonia is..."],
        );

        test(
            "The capital of Foonia? {{Barston}}.",
            "The capital of Foonia? Barston.",
            &["The capital of Foonia?"],
        );

        test(
            "The capital of {{Foonia}} is {{Barston}}.",
            "The capital of Foonia is Barston.",
            &[
                "The capital of ... is Barston.",
                "The capital of Foonia is...",
            ],
        );
    }
}
