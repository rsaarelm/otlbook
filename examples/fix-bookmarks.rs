use base::{Collection, Section};
use std::collections::BTreeSet;

// Temporary tool used to normalize bookmarks imported by an earlier version.

fn main() {
    env_logger::init();

    let mut collection = Collection::new().unwrap();

    for Section(h, b) in collection.outline_mut().walk_mut() {
        if let Ok(Some(_uri)) = b.attr::<String>("uri") {
            // We don't actually need uri value, it's presence is used to
            // recognize bookmark-style entries.

            if let Ok(Some(title)) = b.attr::<String>("title") {
                // Remove title attribute if it's rendundant with headline.
                if &Some(title) == h {
                    b.remove_attr("title").unwrap();
                }
            }

            if let Ok(Some(mut tags)) = b.attr::<BTreeSet<String>>("tags") {
                // Move "important item" marker from tags to the end of the
                // headline.
                if tags.contains("*") {
                    // Add marker to title.
                    *h = match h {
                        Some(h) => Some(format!("{} *", h)),
                        None => Some(format!("*")),
                    };

                    // Remove marker from tags.
                    tags.remove("*");
                    b.set_attr("tags", &tags).unwrap();
                }
            }
        }
    }

    // Smash everything back into DB.
    collection.save().unwrap();
}
