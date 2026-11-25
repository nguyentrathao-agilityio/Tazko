use std::collections::{BTreeSet, HashMap, HashSet};

use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde_json::Value;

use crate::cards::MAX_TITLE;
use crate::cards::repeat::days_from_civil;
use crate::files::valid_link as link_url;
use crate::sanitize;

pub const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
pub const TAZKO_FORMAT: &str = "tazko-board-export";
pub const TAZKO_VERSION: i64 = 1;
const MAX_LISTS: usize = 1_000;
const MAX_CARDS: usize = 20_000;
const MAX_LABELS: usize = 500;
pub const MAX_PEOPLE: usize = 5_000;
const MAX_ITEMS: usize = 200_000;
const MAX_COMMENTS: usize = 200_000;
const MAX_CHECKLISTS: usize = 50_000;
const MAX_LINKS: usize = 100_000;
const PER_CARD: usize = 100;
const PER_CHECKLIST: usize = 1_000;
const CARD_COMMENTS: usize = 5_000;
const MAX_ACTIONS: usize = 500_000;
const BOARD_ABOUT: usize = 5_000;
const MAX_COMMENT_CHARS: usize = 10_000;
const MAX_NAME_CHARS: usize = 120;
const UNTITLED: &str = "Untitled card";

#[derive(Debug, Clone, PartialEq)]
pub struct BoardFile {
    pub format: &'static str,
    pub name: String,
    pub description: String,
    pub labels: Vec<Label>,
    pub people: Vec<Person>,
    pub lists: Vec<List>,
    pub source_colours: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub key: String,
    pub name: String,
    pub color: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub key: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub title: String,
    pub archived: bool,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Card {
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    pub members: Vec<String>,
    pub start_at: Option<i64>,
    pub due_at: Option<i64>,
    pub done: bool,
    pub archived: bool,
    pub checklists: Vec<Checklist>,
    pub comments: Vec<Comment>,
    pub links: Vec<Link>,
    pub files: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Checklist {
    pub title: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub text: String,
    pub done: bool,
    pub assignee: Option<String>,
    pub due_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub author: Option<String>,
    pub author_name: String,
    pub text: String,
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub lists: usize,
    pub cards: usize,
    pub archived: usize,
    pub labels: usize,
    pub checklists: usize,
    pub items: usize,
    pub comments: usize,
    pub links: usize,
    pub files: usize,
}

impl BoardFile {
    pub fn counts(&self) -> Counts {
        let mut n = Counts {
            lists: self.lists.iter().filter(|l| !l.archived).count(),
            labels: self.labels.len(),
            ..Counts::default()
        };
        for (list, card) in self
            .lists
            .iter()
            .flat_map(|l| l.cards.iter().map(move |c| (l, c)))
        {
            if card.archived || list.archived {
                n.archived += 1;
            } else {
                n.cards += 1;
            }
            n.checklists += card.checklists.len();
            n.items += card.checklists.iter().map(|k| k.items.len()).sum::<usize>();
            n.comments += card.comments.len();
            n.links += card.links.len();
            n.files += card.files;
        }
        n
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Detect {
    format: Option<Value>,
    version: Option<Value>,
}

pub fn parse(bytes: &[u8], file_name: &str) -> Result<BoardFile, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("the file is larger than 50 MB".into());
    }

    let detect: Detect = serde_json::from_slice(bytes).map_err(|_| {
        "this is not a board export file; export the board as JSON and try again".to_string()
    })?;
    let fallback = file_name
        .trim()
        .trim_end_matches(".json")
        .trim_end_matches(".JSON")
        .to_string();
    let file = if detect.format.as_ref().and_then(Value::as_str) == Some(TAZKO_FORMAT) {
        if detect.version.as_ref().and_then(Value::as_i64) != Some(TAZKO_VERSION) {
            return Err("this export file comes from a newer version of Tazko".into());
        }

        let raw: TazkoFile = serde_json::from_slice(bytes).map_err(|e| unreadable(&e))?;
        from_tazko(raw, &fallback)?
    } else {
        let raw: GenericFile = serde_json::from_slice(bytes).map_err(|e| unreadable(&e))?;
        if raw.lists.is_empty() && raw.cards.is_empty() {
            return Err("this file is not a board export: it has no lists and cards".into());
        }

        from_generic(raw, &fallback)?
    };
    check_limits(&file)?;
    Ok(file)
}

fn bounded<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const N: usize>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a list of at most {N} entries")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Vec<T>, E> {
            Ok(Vec::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Vec<T>, E> {
            Ok(Vec::new())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element()? {
                if out.len() >= N {
                    return Err(de::Error::custom(format!("{LIMIT_MARK}{N}")));
                }

                out.push(item);
            }
            Ok(out)
        }
    }

    d.deserialize_any(Bounded::<T, N>(PhantomData))
}

const LIMIT_MARK: &str = "list longer than ";

fn unreadable(e: &serde_json::Error) -> String {
    let text = e.to_string();
    match text.find(LIMIT_MARK) {
        Some(i) => {
            let max: String = text[i + LIMIT_MARK.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            format!(
                "the file has more entries in one list than a board can bring (up to {max}); split the board first"
            )
        }
        None => "parts of this board export file could not be read".into(),
    }
}

fn check_limits(file: &BoardFile) -> Result<(), String> {
    let n = file.counts();
    if file.lists.is_empty() {
        return Err("the board in this file has no lists".into());
    }

    let limits = [
        (file.lists.len(), MAX_LISTS, "lists"),
        (n.cards + n.archived, MAX_CARDS, "cards"),
        (file.labels.len(), MAX_LABELS, "labels"),
        (file.people.len(), MAX_PEOPLE, "people"),
        (n.items, MAX_ITEMS, "checklist items"),
        (n.comments, MAX_COMMENTS, "comments"),
        (n.checklists, MAX_CHECKLISTS, "checklists"),
        (n.links, MAX_LINKS, "links"),
    ];
    for (count, max, what) in limits {
        if count > max {
            return Err(format!(
                "the file has {count} {what}; one board can bring up to {max}"
            ));
        }
    }
    Ok(())
}

pub fn line(raw: &str, max: usize) -> String {
    let flat: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn card_title(raw: &str) -> String {
    let t = line(raw, MAX_TITLE);
    if t.is_empty() { UNTITLED.into() } else { t }
}

fn text(raw: &str, max: usize) -> String {
    raw.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

fn non_blank(html: String) -> String {
    if sanitize::plain_text(&html).is_empty() {
        String::new()
    } else {
        html
    }
}

fn description_from_html(raw: &str) -> String {
    non_blank(sanitize::clean_html(truncate(raw, sanitize::MAX_INPUT)))
}

fn description_from_markdown(raw: &str) -> String {
    non_blank(sanitize::markdown_to_html(truncate(
        raw,
        sanitize::MAX_INPUT,
    )))
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }

    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn plausible(t: i64) -> Option<i64> {
    (0..=32_503_680_000).contains(&t).then_some(t)
}

pub fn iso_time(raw: &str) -> Option<i64> {
    let s = raw.trim();
    let b = s.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }

    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }

    let (mut h, mut mi, mut se) = (0, 0, 0);
    if b.len() >= 19 && (b[10] == b'T' || b[10] == b' ') {
        h = num(11..13)?;
        mi = num(14..16)?;
        se = num(17..19)?;
        if h > 23 || mi > 59 || se > 60 {
            return None;
        }
    }

    let offset = s.get(10..).and_then(zone_offset).unwrap_or(0);
    let days = days_from_civil(y, mo, d);
    plausible(days * 86_400 + h * 3600 + mi * 60 + se - offset)
}

fn zone_offset(rest: &str) -> Option<i64> {
    if rest.ends_with('Z') || rest.ends_with('z') || rest.len() < 6 {
        return None;
    }

    let tail = rest.get(rest.len() - 6..)?;
    let sign = match tail.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let h: i64 = tail.get(1..3)?.parse().ok()?;
    let m: i64 = tail.get(4..6)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

pub fn colour(name: &str) -> i64 {
    let base = name.trim().to_ascii_lowercase();
    let base = base.trim_end_matches("_dark").trim_end_matches("_light");
    match base {
        "blue" | "purple" => 0,
        "sky" => 1,
        "pink" => 2,
        "red" => 3,
        "yellow" => 4,
        "green" | "lime" => 6,
        "orange" => 7,
        _ => 5,
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoFile {
    board: TazkoBoard,
    #[serde(deserialize_with = "bounded::<_, _, MAX_LABELS>")]
    labels: Vec<TazkoLabel>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_PEOPLE>")]
    people: Vec<TazkoPerson>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_LISTS>")]
    lists: Vec<TazkoList>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoBoard {
    name: String,
    description: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoLabel {
    key: String,
    name: String,
    color: i64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoPerson {
    key: String,
    name: String,
    email: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoList {
    title: String,
    archived: bool,
    #[serde(deserialize_with = "bounded::<_, _, MAX_CARDS>")]
    cards: Vec<TazkoCard>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoCard {
    title: String,
    description_html: String,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    labels: Vec<String>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    members: Vec<String>,
    start_at: Option<i64>,
    due_at: Option<i64>,
    done: bool,
    archived: bool,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    checklists: Vec<TazkoChecklist>,
    #[serde(deserialize_with = "bounded::<_, _, CARD_COMMENTS>")]
    comments: Vec<TazkoComment>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    links: Vec<TazkoLink>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    files: Vec<de::IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoChecklist {
    title: String,
    #[serde(deserialize_with = "bounded::<_, _, PER_CHECKLIST>")]
    items: Vec<TazkoItem>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoItem {
    text: String,
    done: bool,
    assignee: Option<String>,
    due_at: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoComment {
    author: Option<String>,
    author_name: String,
    text: String,
    created_at: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TazkoLink {
    name: String,
    url: String,
}

fn from_tazko(raw: TazkoFile, fallback: &str) -> Result<BoardFile, String> {
    let people = dedupe_people(
        raw.people
            .into_iter()
            .map(|p| Person {
                key: p.key,
                name: line(&p.name, MAX_NAME_CHARS),
                email: line(&p.email, 254).to_ascii_lowercase(),
            })
            .collect(),
    );
    let known: BTreeSet<String> = people.iter().map(|p| p.key.clone()).collect();
    let keep_person = |k: Option<String>| k.filter(|k| known.contains(k));
    let colours: BTreeSet<i64> = raw.labels.iter().map(|l| l.color).collect();
    let labels: Vec<Label> = raw
        .labels
        .into_iter()
        .map(|l| Label {
            key: l.key,
            name: line(&l.name, MAX_NAME_CHARS),
            color: l.color.clamp(0, 7),
        })
        .collect();
    let lists = raw
        .lists
        .into_iter()
        .map(|l| List {
            title: list_title(&l.title),
            archived: l.archived,
            cards: l
                .cards
                .into_iter()
                .map(|c| Card {
                    title: card_title(&c.title),
                    description: description_from_html(&c.description_html),
                    labels: c.labels,
                    members: c
                        .members
                        .into_iter()
                        .filter(|m| known.contains(m))
                        .collect(),
                    start_at: c.start_at.and_then(plausible),
                    due_at: c.due_at.and_then(plausible),
                    done: c.done,
                    archived: c.archived,
                    checklists: c
                        .checklists
                        .into_iter()
                        .map(|k| Checklist {
                            title: line(&k.title, MAX_TITLE),
                            items: k
                                .items
                                .into_iter()
                                .map(|i| Item {
                                    text: line(&i.text, MAX_TITLE),
                                    done: i.done,
                                    assignee: keep_person(i.assignee),
                                    due_at: i.due_at.and_then(plausible),
                                })
                                .filter(|i| !i.text.is_empty())
                                .collect(),
                        })
                        .collect(),
                    comments: c
                        .comments
                        .into_iter()
                        .map(|m| Comment {
                            author: keep_person(m.author),
                            author_name: line(&m.author_name, MAX_NAME_CHARS),
                            text: text(&m.text, MAX_COMMENT_CHARS),
                            created_at: m.created_at.and_then(plausible),
                        })
                        .filter(|m| !m.text.is_empty())
                        .collect(),
                    links: c
                        .links
                        .into_iter()
                        .filter_map(|a| make_link(&a.name, &a.url))
                        .collect(),
                    files: c.files.len(),
                })
                .collect(),
        })
        .collect();
    Ok(BoardFile {
        format: "tazko",
        name: board_name(&raw.board.name, fallback),
        description: text(&raw.board.description, BOARD_ABOUT),
        labels,
        people,
        lists,
        source_colours: colours.len(),
    })
}

fn list_title(raw: &str) -> String {
    let t = line(raw, MAX_NAME_CHARS);
    if t.is_empty() {
        "Untitled list".into()
    } else {
        t
    }
}

fn board_name(raw: &str, fallback: &str) -> String {
    let t = line(raw, MAX_NAME_CHARS);
    if !t.is_empty() {
        return t;
    }

    let f = line(fallback, MAX_NAME_CHARS);
    if f.is_empty() {
        "Imported board".into()
    } else {
        f
    }
}

fn make_link(raw_name: &str, raw_url: &str) -> Option<Link> {
    let url = link_url(raw_url)?;
    let name = line(raw_name, 200);
    let name = if name.is_empty() {
        line(&url, 200)
    } else {
        name
    };
    Some(Link { name, url })
}

fn dedupe_people(people: Vec<Person>) -> Vec<Person> {
    let mut seen = BTreeSet::new();
    people
        .into_iter()
        .filter(|p| !p.key.is_empty() && seen.insert(p.key.clone()))
        .collect()
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericFile {
    name: String,
    desc: String,
    #[serde(deserialize_with = "bounded::<_, _, MAX_LABELS>")]
    labels: Vec<GenericLabel>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_LISTS>")]
    lists: Vec<GenericList>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_CARDS>")]
    cards: Vec<GenericCard>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_CHECKLISTS>")]
    checklists: Vec<GenericChecklist>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_PEOPLE>")]
    members: Vec<GenericMember>,
    #[serde(deserialize_with = "bounded::<_, _, MAX_ACTIONS>")]
    actions: Vec<GenericAction>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GenericLabel {
    id: String,
    name: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GenericList {
    id: String,
    name: String,
    closed: bool,
    pos: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericCard {
    id: String,
    name: String,
    desc: String,
    id_list: String,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    id_labels: Vec<String>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    id_members: Vec<String>,
    due: Option<String>,
    start: Option<String>,
    due_complete: bool,
    closed: bool,
    pos: Option<f64>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CARD>")]
    attachments: Vec<GenericAttachment>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericAttachment {
    name: String,
    url: String,
    is_upload: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericChecklist {
    id_card: String,
    name: String,
    pos: Option<f64>,
    #[serde(deserialize_with = "bounded::<_, _, PER_CHECKLIST>")]
    check_items: Vec<GenericItem>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericItem {
    name: String,
    state: String,
    pos: Option<f64>,
    id_member: Option<String>,
    due: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericMember {
    id: String,
    full_name: String,
    username: String,
    email: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GenericAction {
    #[serde(rename = "type")]
    kind: String,
    date: String,
    id_member_creator: Option<String>,
    member_creator: Option<GenericMember>,
    data: GenericActionData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GenericActionData {
    text: Option<String>,
    card: Option<GenericRef>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GenericRef {
    id: String,
}

fn by_pos<T>(items: &mut [T], pos: impl Fn(&T) -> Option<f64>) {
    items.sort_by(|a, b| {
        pos(a)
            .unwrap_or(f64::MAX)
            .partial_cmp(&pos(b).unwrap_or(f64::MAX))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn generic_person(m: &GenericMember) -> Person {
    Person {
        key: m.id.clone(),
        name: line(
            if m.full_name.trim().is_empty() {
                &m.username
            } else {
                &m.full_name
            },
            MAX_NAME_CHARS,
        ),
        email: line(m.email.as_deref().unwrap_or(""), 254).to_ascii_lowercase(),
    }
}

fn from_generic(mut raw: GenericFile, fallback: &str) -> Result<BoardFile, String> {
    let mut people: Vec<Person> = raw.members.iter().map(generic_person).collect();
    let mut seen: HashSet<String> = people.iter().map(|p| p.key.clone()).collect();
    for a in &raw.actions {
        if let (Some(id), Some(m)) = (&a.id_member_creator, &a.member_creator)
            && !seen.contains(id)
        {
            if seen.len() >= MAX_PEOPLE {
                return Err(format!(
                    "the file has more than {MAX_PEOPLE} people; one board can bring up to {MAX_PEOPLE}"
                ));
            }

            seen.insert(id.clone());
            people.push(Person {
                key: id.clone(),
                ..generic_person(m)
            });
        }
    }
    let people = dedupe_people(people);
    let names: HashMap<String, String> = people
        .iter()
        .map(|p| (p.key.clone(), p.name.clone()))
        .collect();
    let known = |k: &str| names.contains_key(k);

    let colours: BTreeSet<String> = raw.labels.iter().filter_map(|l| l.color.clone()).collect();
    let labels: Vec<Label> = raw
        .labels
        .iter()
        .filter(|l| !l.id.is_empty())
        .map(|l| Label {
            key: l.id.clone(),
            name: line(l.name.as_deref().unwrap_or(""), MAX_NAME_CHARS),
            color: colour(l.color.as_deref().unwrap_or("")),
        })
        .collect();

    let mut comments: HashMap<String, Vec<(i64, Comment)>> = HashMap::new();
    for (i, a) in raw.actions.iter().enumerate() {
        let (Some(t), Some(card)) = (&a.data.text, &a.data.card) else {
            continue;
        };
        if a.kind != "commentCard" {
            continue;
        }

        let author = a.id_member_creator.clone().filter(|k| known(k));
        let author_name = author
            .as_ref()
            .and_then(|k| names.get(k).cloned())
            .unwrap_or_default();
        let comment = Comment {
            author,
            author_name,
            text: text(t, MAX_COMMENT_CHARS),
            created_at: iso_time(&a.date),
        };
        if !comment.text.is_empty() {
            comments
                .entry(card.id.clone())
                .or_default()
                .push((comment.created_at.unwrap_or(i as i64), comment));
        }
    }

    let mut checklists: HashMap<String, Vec<GenericChecklist>> = HashMap::new();
    for k in std::mem::take(&mut raw.checklists) {
        checklists.entry(k.id_card.clone()).or_default().push(k);
    }

    by_pos(&mut raw.lists, |l| l.pos);
    by_pos(&mut raw.cards, |c| c.pos);
    let mut cards_of: HashMap<String, Vec<GenericCard>> = HashMap::new();
    for c in std::mem::take(&mut raw.cards) {
        cards_of.entry(c.id_list.clone()).or_default().push(c);
    }

    let lists = raw
        .lists
        .iter()
        .map(|l| List {
            title: list_title(&l.name),
            archived: l.closed,
            cards: cards_of
                .remove(&l.id)
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    let mut own = checklists.remove(&c.id).unwrap_or_default();
                    by_pos(&mut own, |k| k.pos);
                    let mut notes = comments.remove(&c.id).unwrap_or_default();
                    notes.sort_by_key(|(at, _)| *at);
                    Card {
                        title: card_title(&c.name),
                        description: description_from_markdown(&c.desc),
                        labels: c.id_labels,
                        members: c.id_members.into_iter().filter(|m| known(m)).collect(),
                        start_at: c.start.as_deref().and_then(iso_time),
                        due_at: c.due.as_deref().and_then(iso_time),
                        done: c.due_complete,
                        archived: c.closed,
                        checklists: own
                            .into_iter()
                            .map(|mut k| {
                                by_pos(&mut k.check_items, |i| i.pos);
                                Checklist {
                                    title: line(&k.name, MAX_TITLE),
                                    items: k
                                        .check_items
                                        .into_iter()
                                        .map(|i| Item {
                                            text: line(&i.name, MAX_TITLE),
                                            done: i.state == "complete",
                                            assignee: i.id_member.filter(|m| known(m)),
                                            due_at: i.due.as_deref().and_then(iso_time),
                                        })
                                        .filter(|i| !i.text.is_empty())
                                        .collect(),
                                }
                            })
                            .collect(),
                        comments: notes.into_iter().map(|(_, m)| m).collect(),
                        links: c
                            .attachments
                            .iter()
                            .filter(|a| !a.is_upload)
                            .filter_map(|a| make_link(&a.name, &a.url))
                            .collect(),
                        files: c.attachments.iter().filter(|a| a.is_upload).count(),
                    }
                })
                .collect(),
        })
        .collect();

    Ok(BoardFile {
        format: "generic",
        name: board_name(&raw.name, fallback),
        description: text(
            &sanitize::plain_text(&description_from_markdown(&raw.desc)),
            BOARD_ABOUT,
        ),
        labels,
        people,
        lists,
        source_colours: colours.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_dates_with_and_without_zones() {
        assert_eq!(iso_time("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_time("2026-11-25T10:30:00Z"), Some(1_795_602_600));
        assert_eq!(iso_time("2026-11-25T17:30:00+07:00"), Some(1_795_602_600));
        assert_eq!(iso_time("2026-11-25"), Some(1_795_564_800));
        assert_eq!(iso_time("not a date"), None);
        assert_eq!(iso_time("2026-13-01"), None);
        assert_eq!(
            iso_time("2026-11-25T10:00:00 \u{e9}\u{e9}"),
            Some(1_795_600_800)
        );
    }

    #[test]
    fn maps_twelve_colours_onto_eight() {
        assert_eq!(colour("green_dark"), 6);
        assert_eq!(colour("sky"), 1);
        assert_eq!(colour("black"), 5);
        assert_eq!(colour(""), 5);
    }

    #[test]
    fn links_must_be_web_addresses() {
        assert!(link_url("https://example.com/a").is_some());
        assert!(link_url("javascript:alert(1)").is_none());
        assert!(link_url("http://").is_none());
        assert!(link_url("https://a b").is_none());
    }

    #[test]
    fn reads_a_generic_board_export() {
        let file = json!({
            "name": "Marketing",
            "desc": "**Plan**",
            "labels": [{ "id": "L1", "name": "Bug", "color": "red" }, { "id": "L2", "name": "", "color": "lime_dark" }],
            "lists": [
                { "id": "B", "name": "Doing", "pos": 2 },
                { "id": "A", "name": "Ideas", "pos": 1 },
                { "id": "C", "name": "Old", "closed": true, "pos": 3 }
            ],
            "cards": [
                { "id": "c2", "name": "Second", "idList": "A", "pos": 2, "closed": true },
                { "id": "c1", "name": "First\nline", "desc": "<script>x</script>hi", "idList": "A", "pos": 1,
                  "idLabels": ["L1"], "idMembers": ["m1", "ghost"], "due": "2026-11-25T10:30:00.000Z", "dueComplete": true,
                  "attachments": [{ "name": "Spec", "url": "https://example.com/spec", "isUpload": false },
                                  { "name": "photo.png", "url": "https://files.example/x", "isUpload": true },
                                  { "name": "bad", "url": "javascript:alert(1)" }] }
            ],
            "checklists": [{ "idCard": "c1", "name": "Steps", "checkItems": [
                { "name": "b", "state": "incomplete", "pos": 2, "idMember": "m1" },
                { "name": "a", "state": "complete", "pos": 1 }
            ] }],
            "members": [{ "id": "m1", "fullName": "Mai Tran", "username": "mai" }],
            "actions": [
                { "type": "commentCard", "date": "2026-11-25T11:00:00Z", "idMemberCreator": "m2",
                  "memberCreator": { "fullName": "Old Intern" }, "data": { "text": "Later", "card": { "id": "c1" } } },
                { "type": "commentCard", "date": "2026-11-25T10:00:00Z", "idMemberCreator": "m1",
                  "data": { "text": "First", "card": { "id": "c1" } } },
                { "type": "updateCard", "data": { "card": { "id": "c1" } } }
            ]
        });
        let f = parse(file.to_string().as_bytes(), "x.json").unwrap();
        assert_eq!(f.format, "generic");
        assert_eq!(
            f.lists.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(),
            ["Ideas", "Doing", "Old"]
        );
        let first = &f.lists[0].cards[0];
        assert_eq!(first.title, "First line");
        assert!(!first.description.contains("script"));
        assert_eq!(first.members, ["m1"]);
        assert_eq!(first.due_at, Some(1_795_602_600));
        assert!(first.done);
        assert_eq!(first.links.len(), 1);
        assert_eq!(first.files, 1);
        assert_eq!(first.checklists[0].items[0].text, "a");
        assert_eq!(first.checklists[0].items[1].assignee.as_deref(), Some("m1"));
        assert_eq!(
            first
                .comments
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["First", "Later"]
        );
        assert_eq!(first.comments[1].author_name, "Old Intern");
        assert!(f.lists[0].cards[1].archived);
        assert_eq!(f.labels[1].color, 6);
        let n = f.counts();
        assert_eq!(
            (n.lists, n.cards, n.archived, n.comments, n.links, n.files),
            (2, 1, 1, 2, 1, 1)
        );
    }

    #[test]
    fn refuses_files_that_are_not_board_exports() {
        assert!(parse(b"[1,2]", "a.json").is_err());
        assert!(parse(b"{\"lists\": []}", "a.json").is_err());
        assert!(parse(b"not json", "a.json").is_err());
        assert!(
            parse(
                json!({ "format": TAZKO_FORMAT, "version": 2 })
                    .to_string()
                    .as_bytes(),
                "a.json"
            )
            .is_err()
        );
        assert!(
            parse(
                json!({ "lists": [], "cards": [] }).to_string().as_bytes(),
                "a.json"
            )
            .is_err()
        );
        let crowded = json!({ "lists": [{ "id": "a", "name": "A" }], "cards": [{ "id": "c", "idList": "a", "idLabels": vec!["x"; PER_CARD + 1] }] });
        let e = parse(crowded.to_string().as_bytes(), "a.json").unwrap_err();
        assert!(e.contains("up to 100"), "{e}");
        let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(parse(deep.as_bytes(), "a.json").is_err());
    }
}
