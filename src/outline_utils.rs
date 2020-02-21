use anki_connect::Card;
use parser::{outline::Outline, Symbol};
use serde::Deserialize;

pub trait OutlineUtils {
    /// Return list of tags defined in this outline node.
    fn tags(&self) -> Vec<Symbol>;

    /// Recursively find Anki cards for the whole outline.
    fn anki_cards(&self) -> Vec<anki_connect::Card>;
}

impl OutlineUtils for Outline {
    fn tags(&self) -> Vec<Symbol> {
        // TODO: Also handle @tag1 @tag2 style tags

        #[derive(Deserialize)]
        struct TagsData {
            tags: Vec<Symbol>,
        }

        if let Some(tags_data) = self.extract::<TagsData>() {
            tags_data.tags
        } else {
            Vec::new()
        }
    }

    fn anki_cards(&self) -> Vec<Card> {
        fn traverse(cards: &mut Vec<Card>, tags: &Vec<String>, o: &Outline) {
            let mut tags = tags.clone();
            tags.extend_from_slice(&o.tags());

            let new_cards = o
                .headline
                .as_ref()
                .filter(|h| !h.starts_with(";"))
                .map_or(None, |h| parser::parse_cloze(&tags, h).ok())
                .unwrap_or(Vec::new());
            cards.extend_from_slice(&new_cards);

            for c in &o.children {
                traverse(cards, &tags, c);
            }
        }

        let mut cards = Vec::new();
        traverse(&mut cards, &Vec::new(), self);
        cards
    }
}
