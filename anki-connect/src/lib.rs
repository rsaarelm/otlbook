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

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} :: {}", self.front, self.back)?;
        if !self.tags.is_empty() {
            write!(f, " [{}]", self.tags.join(" "))?;
        }
        Ok(())
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

pub type AnkiResult<T> = Result<T, ErrBox>;

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
    AddTags { notes: Vec<NoteId>, tags: String },
    RemoveTags { notes: Vec<NoteId>, tags: String },
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

impl From<Card> for Note {
    fn from(c: Card) -> Self {
        Note::new(c.front, c.back, c.tags)
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

impl Response<()> {
    fn as_result(self) -> AnkiResult<()> {
        match self.error {
            Some(e) => Err(Box::new(ResponseError(e))),
            _ => Ok(()),
        }
    }
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

pub struct AnkiConnection {
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

    fn command(&self, query: impl Into<AnkiRequest>) -> AnkiResult<()> {
        let client = reqwest::blocking::Client::new();
        let query: AnkiRequest = query.into();
        let ret: Response<()> = client.post(ANKI_SERVER_URL).json(&query).send()?.json()?;
        ret.as_result()
    }

    fn _add_note(&self, note: Note) -> AnkiResult<()> {
        self.command(Action::AddNote { note })
    }

    pub fn add_notes(&self, notes: Vec<Note>) -> AnkiResult<Vec<Option<NoteId>>> {
        self.request(Action::AddNotes { notes })
    }

    pub fn find_notes(&self) -> AnkiResult<Vec<NoteId>> {
        self.request(Action::FindNotes {
            query: "deck:current".into(),
        })
    }

    pub fn notes_info(&self, notes: Vec<NoteId>) -> AnkiResult<Vec<NoteInfo>> {
        self.request(Action::NotesInfo { notes })
    }

    pub fn delete_notes(&self, notes: Vec<NoteId>) -> AnkiResult<()> {
        self.command(Action::DeleteNotes { notes })
    }

    pub fn sync(&self) -> AnkiResult<()> {
        self.command(Action::Sync)
    }

    pub fn update_note_fields(&self, id: NoteId, front: String, back: String) -> AnkiResult<()> {
        self.command(Action::UpdateNoteFields {
            note: NoteUpdate {
                id,
                fields: Fields { front, back },
            },
        })
    }

    pub fn add_tag(&self, notes: Vec<NoteId>, tag: String) -> AnkiResult<()> {
        self.command(Action::AddTags { notes, tags: tag })
    }

    pub fn remove_tag(&self, notes: Vec<NoteId>, tag: String) -> AnkiResult<()> {
        self.command(Action::RemoveTags { notes, tags: tag })
    }
}

impl Drop for AnkiConnection {
    fn drop(&mut self) {
        if let Some(ref mut proc) = self.anki_process {
            let _ = proc.kill();
        }
    }
}
