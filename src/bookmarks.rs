use rusqlite;
use rusqlite::{params, Connection};

use std::error::Error;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Bookmark {
    pub id: i64,
    pub r#type: Option<i64>,
    pub fk: Option<i64>,
    pub parent: Option<i64>,
    pub position: Option<i64>,
    pub title: Option<String>,
    pub keyword_id: Option<i64>,
    pub folder_type: Option<String>,
    pub date_added: Option<i64>,
    pub last_modified: Option<i64>,
    pub guid: Option<String>,
    pub sync_status: i64,
    pub sync_change_counter: i64,
}

#[derive(Debug, PartialEq)]
pub struct Place {
    pub id: i64,
    pub url: Option<String>,
    pub title: Option<String>,
    pub rev_host: Option<String>,
    pub visit_count: Option<i64>,
    pub hidden: i64,
    pub typed: i64,
    pub favicon_id: Option<i64>,
    pub frecency: i64,
    pub last_visit_date: Option<i64>,
    pub guid: Option<String>,
    pub foreign_count: i64,
    pub url_hash: i64,
    pub description: Option<String>,
    pub preview_image_url: Option<String>,
    pub origin_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub struct Origin {
    pub id: i64,
    pub prefix: String,
    pub host: String,
    pub frecency: i64,
}

pub fn get_latest_bookmark(profile_folder: &str) -> Result<Option<Bookmark>, Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    let mut statement = conn.prepare(
        "
            select
                id, type, fk, parent, position, title, keyword_id,
                folder_type, dateAdded, lastModified, guid, syncStatus, syncChangeCounter
            from moz_bookmarks
            order by id desc
            limit 1",
    )?;
    let bookmark_iter = statement.query_map(params![], |row| {
        Ok(Bookmark {
            id: row.get(0)?,
            r#type: row.get(1)?,
            fk: row.get(2)?,
            parent: row.get(3)?,
            position: row.get(4)?,
            title: row.get(5)?,
            keyword_id: row.get(6)?,
            folder_type: row.get(7)?,
            date_added: row.get(8)?,
            last_modified: row.get(9)?,
            guid: row.get(10)?,
            sync_status: row.get(11)?,
            sync_change_counter: row.get(12)?,
        })
    })?;
    let mut last_bookmark = None;
    for bookmark in bookmark_iter {
        match bookmark {
            Ok(bookmark) => last_bookmark = Some(bookmark),
            Err(e) => return Err(e)?,
        };
    }

    Ok(last_bookmark)
}

pub fn get_new_entries(
    profile_folder: &str,
    first_bookmark: &Bookmark,
) -> Result<
    (
        Option<Vec<Bookmark>>,
        Option<Vec<Place>>,
        Option<Vec<Origin>>,
    ),
    Box<dyn Error>,
> {
    let new_bookmarks = match get_bookmarks_between_two(profile_folder, first_bookmark) {
        Err(e) => {
            return Err(format!("Error during get bookmarks between two : {}", e))?;
        }
        Ok(new_bookmarks) => new_bookmarks,
    };
    match new_bookmarks {
        None => return Ok((None, None, None)),
        Some(new_bookmarks) => {
            let new_places = match get_new_places(profile_folder, &new_bookmarks) {
                Err(e) => {
                    return Err(format!("Error during get new places : {}", e))?;
                }
                Ok(new_places) => new_places,
            };

            match new_places {
                None => return Ok((Some(new_bookmarks), None, None)),
                Some(new_places) => {
                    let new_origins = match get_new_origins(profile_folder, &new_places) {
                        Err(e) => {
                            return Err(format!("Error during get new origins : {}", e))?;
                        }
                        Ok(new_origins) => new_origins,
                    };

                    match new_origins {
                        None => return Ok((Some(new_bookmarks), Some(new_places), None)),
                        Some(new_origins) => {
                            return Ok((Some(new_bookmarks), Some(new_places), Some(new_origins)))
                        }
                    };
                }
            };
        }
    };
}

pub fn get_bookmarks_between_two(
    profile_folder: &str,
    first_bookmark: &Bookmark,
) -> Result<Option<Vec<Bookmark>>, Box<dyn Error>> {
    let latest_bookmark = match get_latest_bookmark(profile_folder) {
        Err(e) => return Err(e)?,
        Ok(bookmark) => match bookmark {
            // no bookmarks exist
            // might be a case that all got deleted
            // TODO: add deleted case
            None => return Ok(None),
            Some(bookmark) => bookmark,
        },
    };

    if first_bookmark.id >= latest_bookmark.id {
        // either no new bookmarks, or bookmarks were deleted,
        // which is not supported for now
        // TODO: add deleted case
        return Ok(None);
    }

    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    let mut statement = conn.prepare(
        "
            select
                id, type, fk, parent, position, title, keyword_id,
                folder_type, dateAdded, lastModified, guid, syncStatus, syncChangeCounter
            from moz_bookmarks
            where 1=1
            and id > :low_id
            and id <= :high_id
            order by id",
    )?;
    let bookmark_iter = statement.query_map_named(
        &[
            (":low_id", &first_bookmark.id),
            (":high_id", &latest_bookmark.id),
        ],
        |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                r#type: row.get(1)?,
                fk: row.get(2)?,
                parent: row.get(3)?,
                position: row.get(4)?,
                title: row.get(5)?,
                keyword_id: row.get(6)?,
                folder_type: row.get(7)?,
                date_added: row.get(8)?,
                last_modified: row.get(9)?,
                guid: row.get(10)?,
                sync_status: row.get(11)?,
                sync_change_counter: row.get(12)?,
            })
        },
    )?;

    let mut bookmarks = vec![];
    for bookmark in bookmark_iter {
        match bookmark {
            Ok(bookmark) => {
                bookmarks.push(bookmark);
            }
            Err(e) => return Err(e)?,
        };
    }

    if bookmarks.len() == 0 {
        Ok(None)
    } else {
        Ok(Some(bookmarks))
    }
}

pub fn get_new_places(
    profile_folder: &str,
    bookmarks: &[Bookmark],
) -> Result<Option<Vec<Place>>, Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    let mut statement = conn.prepare(
        "
            select
                id, url, title, rev_host, visit_count, hidden,
                typed, favicon_id, frecency, last_visit_date, 
                guid, foreign_count, url_hash, description, preview_image_url, origin_id
            from moz_places
            where 1=1
            and id = :places_id
            order by id desc
        ",
    )?;

    let mut places = vec![];
    for bookmark in bookmarks {
        let places_id = match bookmark.fk {
            None => continue,
            Some(v) => v,
        };

        let places_iter = statement.query_map_named(&[(":places_id", &places_id)], |row| {
            Ok(Place {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                rev_host: row.get(3)?,
                visit_count: row.get(4)?,
                hidden: row.get(5)?,
                typed: row.get(6)?,
                favicon_id: row.get(7)?,
                frecency: row.get(8)?,
                last_visit_date: row.get(9)?,
                guid: row.get(10)?,
                foreign_count: row.get(11)?,
                url_hash: row.get(12)?,
                description: row.get(13)?,
                preview_image_url: row.get(14)?,
                origin_id: row.get(15)?,
            })
        })?;
        for place in places_iter {
            match place {
                Ok(place) => {
                    places.push(place);
                }
                Err(e) => return Err(e)?,
            };
        }
    }

    if places.len() == 0 {
        Ok(None)
    } else {
        Ok(Some(places))
    }
}

pub fn get_new_origins(
    profile_folder: &str,
    places: &[Place],
) -> Result<Option<Vec<Origin>>, Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    let mut statement = conn.prepare(
        "
            select
                id, prefix, host, frecency
            from moz_origins
            where 1=1
            and id = :origin_id
            order by id desc
        ",
    )?;

    let mut origins = vec![];
    for place in places {
        let origin_id = match place.origin_id {
            None => continue,
            Some(v) => v,
        };

        let origins_iter = statement.query_map_named(&[(":origin_id", &origin_id)], |row| {
            Ok(Origin {
                id: row.get(0)?,
                prefix: row.get(1)?,
                host: row.get(2)?,
                frecency: row.get(3)?,
            })
        })?;
        for origin in origins_iter {
            match origin {
                Ok(origin) => {
                    origins.push(origin);
                }
                Err(e) => return Err(e)?,
            };
        }
    }

    if origins.len() == 0 {
        Ok(None)
    } else {
        Ok(Some(origins))
    }
}
