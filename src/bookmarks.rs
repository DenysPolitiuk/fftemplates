use rusqlite;
use rusqlite::{params, Connection};

use std::collections::HashMap;
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
        Option<HashMap<i64, Place>>,
        Option<HashMap<i64, Origin>>,
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
) -> Result<Option<HashMap<i64, Place>>, Box<dyn Error>> {
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

    let mut places = HashMap::new();
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
                    places.insert(places_id, place);
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
    places: &HashMap<i64, Place>,
) -> Result<Option<HashMap<i64, Origin>>, Box<dyn Error>> {
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

    let mut origins = HashMap::new();
    for place in places.values() {
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
                    origins.insert(origin_id, origin);
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

pub fn insert_new_entries(
    profile_folder: &str,
    new_bookmarks: Option<&mut Vec<Bookmark>>,
    mut new_places: Option<&mut HashMap<i64, Place>>,
    mut new_origins: Option<&mut HashMap<i64, Origin>>,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref mut new_origins) = new_origins {
        if let Err(e) = insert_new_origins(profile_folder, new_origins) {
            eprintln!("Error during insert new origins : {}", e);
        }
    }
    // hack to transform Option<&mut ...> into Option<&...>
    let new_origins = match new_origins {
        None => None,
        Some(v) => Some(&*v),
    };
    if let Some(ref mut new_places) = new_places {
        if let Err(e) = insert_new_places(profile_folder, new_places, new_origins) {
            eprintln!("Error during insert new places : {}", e);
        }
    }
    // hack to transform Option<&mut ...> into Option<&...>
    let new_places = match new_places {
        None => None,
        Some(v) => Some(&*v),
    };
    if let Some(mut new_bookmarks) = new_bookmarks {
        if let Err(e) = insert_new_bookmarks(profile_folder, &mut new_bookmarks, new_places) {
            eprintln!("Error during insert new bookmarks : {}", e);
        }
    }

    Ok(())
}

pub fn insert_new_bookmarks(
    profile_folder: &str,
    new_bookmarks: &mut [Bookmark],
    new_places: Option<&HashMap<i64, Place>>,
) -> Result<(), Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    // not doing a check for duplicate, assuming this will not happened

    let mut max_id_statement = conn.prepare(
        "
            select max(id) from moz_bookmarks;
        ",
    )?;

    for bookmark in new_bookmarks.iter_mut() {
        // get max id in the table just in case something was already inserted
        let max_id = max_id_statement.query_map(params![], |row| row.get(0))?;
        for max_id in max_id {
            let max_id = match max_id {
                Err(e) => return Err(e)?,
                Ok(max_id) => max_id,
            };
            // check if current max id is not the one
            // before inserting current entry
            if max_id != bookmark.id - 1 {
                bookmark.id = max_id;
                bookmark.id += 1;
            }
        }

        if let Some(new_places) = new_places {
            if let Some(fk) = bookmark.fk {
                bookmark.fk = match new_places.get(&fk) {
                    None => return Err("unable to find fk place from bookmark")?,
                    Some(v) => Some(v.id),
                };
            }
        }

        conn.execute(
            "
                insert  into moz_bookmarks (
                    id, type, fk, parent, position,
                    title, keyword_id, folder_type, dateAdded, lastModified,
                    guid, syncStatus, syncChangeCounter)
                values(
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13)
            ",
            params![
                bookmark.id,
                bookmark.r#type,
                bookmark.fk,
                bookmark.parent,
                bookmark.position,
                bookmark.title,
                bookmark.keyword_id,
                bookmark.folder_type,
                bookmark.date_added,
                bookmark.last_modified,
                bookmark.guid,
                bookmark.sync_status,
                bookmark.sync_change_counter
            ],
        )?;
    }

    Ok(())
}

pub fn insert_new_places(
    profile_folder: &str,
    new_places: &mut HashMap<i64, Place>,
    new_origins: Option<&HashMap<i64, Origin>>,
) -> Result<(), Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    // not doing a check for duplicate, as it seems unlikely to have duplicate

    let mut max_id_statement = conn.prepare(
        "
            select max(id) from moz_places;
        ",
    )?;
    for place in new_places.values_mut() {
        // get max id in the table just in case something was already inserted
        let max_id = max_id_statement.query_map(params![], |row| row.get(0))?;
        for max_id in max_id {
            let max_id = match max_id {
                Err(e) => return Err(e)?,
                Ok(max_id) => max_id,
            };
            // check if current max id is not the one
            // before inserting current entry
            if max_id != place.id - 1 {
                place.id = max_id;
                place.id += 1;
            }
        }

        // check to see if origin had it's id changed
        // this can happened if a different origin was inserted
        // with an id of current origin and place needs to match to
        // the correct origin
        if let Some(new_origins) = new_origins {
            if let Some(origin_id) = place.origin_id {
                place.origin_id = match new_origins.get(&origin_id) {
                    None => return Err("unable to find origin from place")?,
                    Some(v) => Some(v.id),
                };
            }
        }

        conn.execute(
            "insert into moz_places (id, url, title, rev_host,
                visit_count, hidden, typed, favicon_id,
                frecency, last_visit_date, guid, foreign_count,
                url_hash, description, preview_image_url, origin_id)
            values(?1, ?2, ?3, ?4,
                ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16)",
            params![
                place.id,
                place.url,
                place.title,
                place.rev_host,
                place.visit_count,
                place.hidden,
                place.typed,
                place.favicon_id,
                place.frecency,
                place.last_visit_date,
                place.guid,
                place.foreign_count,
                place.url_hash,
                place.description,
                place.preview_image_url,
                place.origin_id
            ],
        )?;
    }

    Ok(())
}

pub fn insert_new_origins(
    profile_folder: &str,
    new_origins: &mut HashMap<i64, Origin>,
) -> Result<(), Box<dyn Error>> {
    let database_file = Path::new(profile_folder).join(Path::new("places.sqlite"));
    let conn = Connection::open(database_file)?;

    let mut statement = conn.prepare(
        "
            select id
            from moz_origins
            where 1=1
            and prefix = :prefix
            and host = :host
            and frecency = :frecency
        ",
    )?;
    let mut max_id_statement = conn.prepare(
        "
            select max(id) from moz_origins;
        ",
    )?;

    for origin in new_origins.values_mut() {
        // get new id for this origin, if it already exists
        let results = statement.query_map_named(
            &[
                (":prefix", &origin.prefix),
                (":host", &origin.host),
                (":frecency", &origin.frecency),
            ],
            |row| row.get(0),
        )?;
        let mut new_id: Option<i64> = None;
        for result in results {
            match result {
                Err(e) => return Err(e)?,
                Ok(result) => new_id = Some(result),
            };
        }
        if let Some(new_id) = new_id {
            origin.id = new_id;
        } else {
            // get max id in the table just in case something was already inserted
            let max_id = max_id_statement.query_map(params![], |row| row.get(0))?;
            for max_id in max_id {
                let max_id = match max_id {
                    Err(e) => return Err(e)?,
                    Ok(max_id) => max_id,
                };
                // check if current max id is not the one
                // before inserting current entry
                if max_id != origin.id - 1 {
                    origin.id = max_id;
                    origin.id += 1;
                }
            }

            // insert in the case that origin doesn't exist
            conn.execute(
                "insert into moz_origins (id, prefix, host, frecency)
                values(?1, ?2, ?3, ?4)",
                params![origin.id, origin.prefix, origin.host, origin.frecency],
            )?;
        }
    }

    Ok(())
}
