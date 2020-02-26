use crate::load_database_or_die;
use crate::outline_utils::OutlineUtils;
use anki_connect::{AnkiConnection, Card};
use dialoguer::Input;
use std::collections::{BTreeMap, BTreeSet};
use termion::color;

pub fn anki(dump: bool) {
    let db = load_database_or_die();
    let new_cards = db.anki_cards();

    if dump {
        for card in &new_cards {
            println!("{}\t{}\t{}", card.front, card.back, card.tags.join(" "));
        }
    } else {
        let anki = AnkiConnection::new().expect("Couldn't connect to Anki.");

        let notes = anki.find_notes().unwrap();

        let mut ids = BTreeMap::new();
        let mut old_cards = BTreeMap::new();

        for info in anki.notes_info(notes).unwrap() {
            let id = info.note_id;
            let card = Card::from(info);
            let front = card.front.clone();
            ids.insert(front.clone(), id);
            old_cards.insert(front, card);
        }

        let new_cards = new_cards
            .into_iter()
            .map(|c| (c.front.clone(), c))
            .collect::<BTreeMap<_, _>>();

        let mut update = BTreeSet::new();
        let mut delete = old_cards.keys().collect::<BTreeSet<_>>();
        let mut add = BTreeSet::new();

        for (k, new_card) in &new_cards {
            delete.remove(k);
            match old_cards.get(k) {
                Some(old_card) if old_card != new_card => {
                    update.insert(k);
                }
                None => {
                    add.insert(k);
                }
                _ => {}
            }
        }

        for k in &add {
            println!(
                "{}will add{} {}",
                color::Fg(color::Green),
                color::Fg(color::Reset),
                new_cards[*k]
            );
        }

        for k in &update {
            println!(
                "{}will update{} {}",
                color::Fg(color::Yellow),
                color::Fg(color::Reset),
                new_cards[*k]
            );
        }

        for k in &delete {
            println!(
                "{}will delete{} {}",
                color::Fg(color::Red),
                color::Fg(color::Reset),
                old_cards[*k]
            );
        }

        if !delete.is_empty() {
            let confirmation = Input::<String>::new()
                .with_prompt(&format!("Proceed to delete {} cards? [y/N]", delete.len()))
                .interact()
                .unwrap();
            match confirmation.as_str() {
                "y" | "Y" => {}
                _ => {
                    return;
                }
            }
        }

        anki.delete_notes(delete.iter().map(|f| ids[*f]).collect())
            .expect("Delete notes failed");
        anki.add_notes(add.iter().map(|f| new_cards[*f].clone().into()).collect())
            .expect("Add notes failed");

        for k in &update {
            let id = ids[*k];
            let old = &old_cards[*k];
            let new = &new_cards[*k];

            for t in &old.tags {
                anki.remove_tag(vec![id], t.clone())
                    .expect("Remove tag failed");
            }
            for t in &new.tags {
                anki.add_tag(vec![id], t.clone()).expect("Add tag failed");
            }

            anki.update_note_fields(id, new.front.clone(), new.back.clone())
                .expect("Update note fields failed");
        }

        anki.sync().unwrap();
    }
}
