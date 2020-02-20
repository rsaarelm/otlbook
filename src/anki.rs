use crate::load_database_or_die;
use anki_connect::AnkiConnection;

pub fn anki(dump: bool) {
    let db = load_database_or_die();

    // TODO: Collect tags. (This will probably involve rewriting the nice outline iter traversal
    // thing below into a custom recursive function since tags mustn't propagate to sibling nodes.)
    let new_cards: Vec<anki_connect::Card> = db
        .iter()
        .filter_map(|o| o.headline.as_ref())
        .filter(|h| !h.starts_with(";")) // Don't include comments
        .filter_map(|h| parser::parse_cloze(&Vec::new(), h).ok())
        .flatten() // Each cloze can emit multiple cards, flatten result.
        .collect();

    if dump {
        for card in &new_cards {
            println!("{}\t{}\t{}", card.front, card.back, card.tags.join(" "));
        }
    } else {
        let anki = AnkiConnection::new().expect("Couldn't connect to Anki.");

        let notes = anki.find_notes().unwrap();
        let old_cards = anki
            .notes_info(notes.clone())
            .unwrap()
            .into_iter()
            .map(|n| anki_connect::Card::from(n))
            .collect::<Vec<_>>();
        println!(
            "{} existing cards, {} new cards",
            old_cards.len(),
            new_cards.len()
        );
        // TODO
        // Determine change based on new cards
        // Prompt user if change involves updates or deletions
        // If user agrees, send change to db
        todo!();
    }
}
