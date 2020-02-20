use serde_derive::{Deserialize, Serialize};
use std::{error::Error, fmt, process, thread, time};

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Card {
    pub front: String,
    pub back: String,
    pub tags: Vec<String>,
}

impl Card {
    pub fn new(
        front: impl Into<String>,
        back: impl Into<String>,
        tags: Vec<impl Into<String>>,
    ) -> Card {
        Card {
            front: front.into(),
            back: back.into(),
            tags: tags.into_iter().map(|c| c.into()).collect(),
        }
    }
}

impl From<NoteInfo> for Card {
    fn from(note: NoteInfo) -> Card {
        Card {
            front: note.fields.front.value,
            back: note.fields.back.value,
            tags: note.tags,
        }
    }
}

const ANKI_SERVER_URL: &str = "http://127.0.0.1:8765";

pub type ErrBox = Box<dyn Error + Send + Sync + 'static>;

type AnkiResult<T> = Result<T, ErrBox>;

pub type NoteId = u64;

#[derive(Clone, Debug, Serialize)]
pub struct AnkiRequest {
    version: i32,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "action", content = "params", rename_all = "camelCase")]
pub enum Action {
    AddNote { note: Note },
    AddNotes { notes: Vec<Note> },
    DeleteNotes { notes: Vec<NoteId> },
    FindNotes { query: String },
    NotesInfo { notes: Vec<NoteId> },
    Sync,
    UpdateNoteFields { note: NoteUpdate },
}

impl From<Action> for AnkiRequest {
    fn from(action: Action) -> AnkiRequest {
        AnkiRequest { version: 6, action }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub deck_name: String,
    pub model_name: String,
    pub fields: Fields<String>,
    pub tags: Vec<String>,
}

impl Note {
    pub fn new(front: String, back: String, tags: Vec<String>) -> Note {
        Note {
            deck_name: "Default".into(),
            model_name: "Basic".into(),
            fields: Fields { front, back },
            tags,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInfo {
    pub note_id: NoteId,
    pub model_name: String,
    pub fields: Fields<FieldData>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoteUpdate {
    pub id: NoteId,
    pub fields: Fields<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fields<T> {
    #[serde(rename = "Front")]
    pub front: T,
    #[serde(rename = "Back")]
    pub back: T,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FieldData {
    value: String,
    order: i32,
}

#[derive(Debug)]
pub struct ResponseError(String);

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ResponseError {}

#[derive(Clone, Debug, Deserialize)]
pub struct Response<T> {
    result: Option<T>,
    error: Option<String>,
}

impl<T> Into<AnkiResult<T>> for Response<T> {
    fn into(self) -> AnkiResult<T> {
        match (self.result, self.error) {
            (None, None) => {
                log::warn!("Received malformed response");
                Err(Box::new(ResponseError("n/a".into())))
            }
            (Some(ret), None) => Ok(ret),
            (None, Some(e)) => Err(Box::new(ResponseError(e))),
            (Some(_), Some(e)) => {
                log::warn!("Received malformed response");
                Err(Box::new(ResponseError(e)))
            }
        }
    }
}

struct AnkiConnection {
    anki_process: Option<process::Child>,
}

impl AnkiConnection {
    pub fn new() -> Result<AnkiConnection, ErrBox> {
        log::debug!("Probing for running Anki server...");
        let is_anki_running = {
            let res = reqwest::blocking::get(ANKI_SERVER_URL);
            log::debug!("Response from server: {:?}", res);
            res.is_ok()
        };

        if is_anki_running {
            Ok(AnkiConnection { anki_process: None })
        } else {
            log::info!("Anki not running, starting process");
            match process::Command::new("anki").spawn() {
                Ok(proc) => {
                    log::debug!("Waiting for Anki to start...");
                    thread::sleep(time::Duration::from_secs(3));
                    Ok(AnkiConnection {
                        anki_process: Some(proc),
                    })
                }
                Err(e) => Err(Box::new(e)),
            }
        }
    }

    fn request<T: serde::de::DeserializeOwned>(
        &self,
        query: impl Into<AnkiRequest>,
    ) -> AnkiResult<T> {
        let client = reqwest::blocking::Client::new();
        let query: AnkiRequest = query.into();
        let ret: Response<T> = client.post(ANKI_SERVER_URL).json(&query).send()?.json()?;
        ret.into()
    }

    fn add_note(&self, note: Note) -> AnkiResult<()> {
        self.request(Action::AddNote { note })
    }

    fn add_notes(&self, notes: Vec<Note>) -> AnkiResult<Vec<Option<NoteId>>> {
        self.request(Action::AddNotes { notes })
    }

    fn find_notes(&self) -> AnkiResult<Vec<NoteId>> {
        self.request(Action::FindNotes {
            query: "deck:current".into(),
        })
    }

    fn notes_info(&self, notes: Vec<NoteId>) -> AnkiResult<Vec<NoteInfo>> {
        self.request(Action::NotesInfo { notes })
    }

    fn delete_notes(&self, notes: Vec<NoteId>) -> AnkiResult<()> {
        self.request(Action::DeleteNotes { notes })
    }

    fn sync(&self) -> AnkiResult<()> {
        self.request(Action::Sync)
    }

    fn update_note_fields(&self, id: NoteId, front: String, back: String) -> AnkiResult<()> {
        self.request(Action::UpdateNoteFields {
            note: NoteUpdate {
                id,
                fields: Fields { front, back },
            },
        })
    }
}

impl Drop for AnkiConnection {
    fn drop(&mut self) {
        if let Some(ref mut proc) = self.anki_process {
            let _ = proc.kill();
        }
    }
}

pub fn update_cards(new_set: Vec<Card>) -> Result<(), ErrBox> {
    use std::collections::HashMap;

    let anki = AnkiConnection::new()?;
    let notes = anki.find_notes()?;
    let notes = anki.notes_info(notes)?;

    let old_ids: HashMap<String, NoteId> = notes
        .iter()
        .map(|note| (note.fields.front.value.clone(), note.note_id))
        .collect();

    let new_set: HashMap<String, Card> = new_set
        .iter()
        .map(|c| (c.front.clone(), c.clone()))
        .collect();
    let old_set: HashMap<String, Card> = notes
        .iter()
        .map(|n| (n.fields.front.value.clone(), n.clone().into()))
        .collect();

    for (key, card) in &old_set {
        if !new_set.contains_key(key) {
            log::debug!("Will delete {:?}", card);
        }
    }

    for (key, card) in &new_set {
        if !old_set.contains_key(key) {
            log::debug!("Will add new {:?}", card);
        } else {
            let old_card = &old_set[key];
            if old_card != card {
                log::debug!("Will update {:?} to {:?}", old_card, card);
            }
        }
    }

    // TODO
    Ok(())
}
/*
pub fn update_cards(new_set: Vec<(String, (String, Vec<String>))>) -> Result<(), ErrBox> {
    use std::collections::HashMap;

    let anki = AnkiConnection::new()?;
    let card_ids = anki.find_cards()?;
    let cards = anki.cards_info(card_ids.clone())?;

    let current: HashMap<String, (CardId, String)> = cards
        .iter()
        .map(|c| (c.fields.front.value.clone(), (c.card_id, c.fields.back.value.clone())))
        .collect();

    let new: HashMap<String, (String, Vec<String>)> = new_set.iter().cloned().collect();

    /*
     * The algorithm:
     *
     * C: set of current card fronts
     * N: set of new card fronts
     *
     * Suspend cards in C - N
     * Update cards in C ∩ N where card back is different in N
     * Add cards in N - C
     */

    let mut suspend_list = Vec::new();

    for (front, (id, back)) in &current {
        if !new.contains_key(front) {
            log::info!("Suspending card {:?}", front);
            // Card does not exist in current deck, suspend it.
            suspend_list.push(*id);
            continue;
        }

        let new_back = &new[front];
        if new_back != back {
            log::info!("Updating card {:?} from {:?} to {:?}", front, back, new_back);
            // Card exists but the back text has changed. Update.
            anki.update_note_fields(*id, front.clone(), new_back.clone())?;
        }
    }

    anki.suspend(suspend_list.clone())?;

    let mut add_list = Vec::new();

    /*
    for (front, back) in &new {
        if !current.contains_key(front) {
            log::info!("Adding new card {:?} :: {:?}", front, back);
            add_list.push(Note {
                deck_name: "Default".into(),
                model_name: "Basic
            })
        }
    }
    */

    todo!();
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    //#[test]
    fn test_connection() {
        // TODO: Not good for a real test since it interacts with external system...
        // scrap before committing.
        env_logger::init();
        let conn = AnkiConnection::new().unwrap();

        let notes = conn.find_notes().unwrap();

        assert_eq!(
            format!("{:?}", conn.notes_info(vec![notes[0]]).unwrap()),
            "This test is just for development, delete when done"
        );
    }
}
