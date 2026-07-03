//! The project forum: a git-tracked, thread-structured discussion hub that lives
//! beside the board under `.wipe/forum/`.
//!
//! Where tickets track *work*, the forum is for everything around it - decisions,
//! discovered rules, gotchas, questions, and knowledge that should compound over a
//! project's life. Any human or agent can post; posts nest into reply trees like a
//! Reddit thread; everything is deterministic JSON so git tracks every message.
//!
//! Reads go through a small on-disk cache (`.wipe/.cache/forum-index.json`) that is
//! rebuilt only when the forum files actually change, so search / list / watch stay
//! fast and cheap even as the forum grows.
//!
//! Concurrency, honestly: like the rest of wipe, thread files are written with a
//! last-writer-wins atomic rename and no cross-process lock. Replies to *different*
//! threads (even on different git branches) never conflict; two writers racing on
//! the *same* thread can lose one update, exactly like two edits to one ticket -
//! git is the merge/audit layer. Thread-id allocation is guarded against reuse so a
//! crash or branch divergence can never overwrite an existing thread. The read
//! cache keys off each file's size + mtime; on filesystems with coarse mtime
//! resolution (e.g. some removable/network mounts) a same-length edit landing in
//! the same tick can serve a stale search result until the next change - deleting
//! `.cache/` always forces a clean rebuild.

use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Attachment, Post, Thread, FORMAT_VERSION};
use crate::store::Store;

/// Spec for opening a new thread.
#[derive(Debug, Default, Clone)]
pub struct NewThread {
    /// Thread headline.
    pub title: String,
    /// Root message body.
    pub body: String,
    /// Labels (same pool as tickets).
    pub labels: Vec<String>,
    /// Free-form references (ticket IDs, post IDs, URLs).
    pub refs: Vec<String>,
    /// Pre-staged attachments (see [`crate::ops::stage_media`]).
    pub attachments: Vec<Attachment>,
}

/// Spec for replying to a post.
#[derive(Debug, Default, Clone)]
pub struct NewReply {
    /// Reply body.
    pub body: String,
    /// Labels.
    pub labels: Vec<String>,
    /// References.
    pub refs: Vec<String>,
    /// Pre-staged attachments.
    pub attachments: Vec<Attachment>,
}

/// The thread ID that owns a post ID (`F-1.2.3` -> `F-1`).
pub fn thread_of(post_id: &str) -> String {
    match post_id.split_once('.') {
        Some((head, _)) => head.to_string(),
        None => post_id.to_string(),
    }
}

/// Open a new thread with a root post. Returns the created thread.
pub fn create_thread(
    store: &Store,
    spec: NewThread,
    author: &str,
    now: DateTime<Utc>,
) -> Result<Thread> {
    let mut board = store.load_board()?;
    // Never reuse an id: skip past any thread file already on disk (board.json and
    // the forum/*.json files can diverge across git operations or a crash between
    // the two writes below).
    let mut n = board.next_thread;
    let mut id = format!("F-{n}");
    while store.thread_exists(&id) {
        n += 1;
        id = format!("F-{n}");
    }
    board.next_thread = n + 1;

    let mut root = Post::new(id.clone(), author, spec.body, now);
    root.labels = spec.labels;
    root.refs = spec.refs;
    root.attachments = spec.attachments;

    let thread = Thread {
        version: FORMAT_VERSION,
        id: id.clone(),
        title: spec.title,
        root,
        created: now,
        updated: now,
    };
    // Persist the counter FIRST: if we're interrupted between the two writes, the
    // id is over-allocated (a harmless gap) rather than reused next time.
    store.save_board(&board)?;
    store.save_thread(&thread)?;
    Ok(thread)
}

/// Reply to an existing post (at any depth). Returns the new post's ID.
pub fn reply(
    store: &Store,
    parent_id: &str,
    spec: NewReply,
    author: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    let tid = thread_of(parent_id);
    let mut thread = store.load_thread(&tid)?;
    let parent = thread
        .root
        .find_mut(parent_id)
        .ok_or_else(|| Error::PostNotFound(parent_id.to_string()))?;

    let child_id = format!("{parent_id}.{}", parent.next_reply);
    parent.next_reply += 1;

    let mut post = Post::new(child_id.clone(), author, spec.body, now);
    post.labels = spec.labels;
    post.refs = spec.refs;
    post.attachments = spec.attachments;
    parent.replies.push(post);

    thread.updated = now;
    store.save_thread(&thread)?;
    Ok(child_id)
}

/// Edit a post's body (records an `edited` timestamp).
pub fn edit_post(store: &Store, id: &str, body: &str, now: DateTime<Utc>) -> Result<()> {
    let tid = thread_of(id);
    let mut thread = store.load_thread(&tid)?;
    let post = thread
        .root
        .find_mut(id)
        .ok_or_else(|| Error::PostNotFound(id.to_string()))?;
    post.body = body.to_string();
    post.edited = Some(now);
    // Editing the root headline is done via a title arg on the CLI; here we only
    // touch the body so the operation is minimal and predictable.
    thread.updated = now;
    store.save_thread(&thread)?;
    Ok(())
}

/// Delete a post and its entire subtree. Deleting a thread's root removes the
/// whole thread file.
pub fn delete_post(store: &Store, id: &str, now: DateTime<Utc>) -> Result<()> {
    let tid = thread_of(id);
    if id == tid {
        // Root: the whole thread goes.
        return store.delete_thread(&tid);
    }
    let mut thread = store.load_thread(&tid)?;
    if !thread.root.remove_child(id) {
        return Err(Error::PostNotFound(id.to_string()));
    }
    thread.updated = now;
    store.save_thread(&thread)?;
    Ok(())
}

/// Load a whole thread by thread ID.
pub fn get_thread(store: &Store, thread_id: &str) -> Result<Thread> {
    store.load_thread(&thread_of(thread_id))
}

// --- flattened view + cache ------------------------------------------------

/// A single post flattened out of its tree, with context for search/list/watch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostView {
    /// Dotted post ID.
    pub id: String,
    /// Owning thread ID.
    pub thread_id: String,
    /// Owning thread title.
    pub thread_title: String,
    /// Author identity.
    pub author: String,
    /// Message body.
    pub body: String,
    /// Labels on this post.
    pub labels: Vec<String>,
    /// References on this post.
    pub refs: Vec<String>,
    /// Depth in the tree (root = 0).
    pub depth: usize,
    /// Number of direct replies.
    pub replies: usize,
    /// Number of attachments.
    pub attachments: usize,
    /// Created timestamp.
    pub created: DateTime<Utc>,
    /// Edited timestamp, if any.
    pub edited: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    /// (file count, summed mtime nanos) - changes on any add/remove/edit.
    signature: (usize, u128),
    posts: Vec<PostView>,
}

/// A cheap fingerprint of the forum directory: the file count plus a running sum
/// of each thread file's size and modification time. Any post added, removed, or
/// edited changes a file's size and/or mtime, and therefore this signature. Size
/// is included so the cache still invalidates when two writes land within the
/// filesystem's mtime resolution (which happens in fast/back-to-back updates).
fn forum_signature(store: &Store) -> (usize, u128) {
    let dir = store.forum_dir();
    let mut count = 0usize;
    let mut sum: u128 = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            count += 1;
            if let Ok(meta) = entry.metadata() {
                sum = sum.wrapping_add(meta.len() as u128);
                if let Ok(mt) = meta.modified() {
                    if let Ok(d) = mt.duration_since(UNIX_EPOCH) {
                        sum = sum.wrapping_add(d.as_nanos());
                    }
                }
            }
        }
    }
    (count, sum)
}

fn cache_path(store: &Store) -> std::path::PathBuf {
    store.cache_dir().join("forum-index.json")
}

fn flatten(thread: &Thread) -> Vec<PostView> {
    let mut out = Vec::new();
    thread.root.walk(0, &mut |p: &Post, depth: usize| {
        out.push(PostView {
            id: p.id.clone(),
            thread_id: thread.id.clone(),
            thread_title: thread.title.clone(),
            author: p.author.clone(),
            body: p.body.clone(),
            labels: p.labels.clone(),
            refs: p.refs.clone(),
            depth,
            replies: p.replies.len(),
            attachments: p.attachments.len(),
            created: p.created,
            edited: p.edited,
        });
    });
    out
}

/// Every post across the forum, flattened, newest-first. Served from an on-disk
/// cache that is rebuilt only when the forum files change.
pub fn index(store: &Store) -> Result<Vec<PostView>> {
    let sig = forum_signature(store);
    let cpath = cache_path(store);

    if let Ok(bytes) = std::fs::read(&cpath) {
        if let Ok(cache) = serde_json::from_slice::<CacheFile>(&bytes) {
            if cache.signature == sig {
                return Ok(cache.posts);
            }
        }
    }

    let mut posts: Vec<PostView> = Vec::new();
    for thread in store.load_all_threads()? {
        posts.extend(flatten(&thread));
    }
    posts.sort_by(|a, b| b.created.cmp(&a.created));

    // Best-effort cache write; never fail a read over a cache issue.
    if let Some(dir) = cpath.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut s) = serde_json::to_string(&CacheFile {
        signature: sig,
        posts: posts.clone(),
    }) {
        s.push('\n');
        let _ = std::fs::write(&cpath, s);
    }
    Ok(posts)
}

// --- search ----------------------------------------------------------------

/// A forum search. All set filters must match (AND).
#[derive(Debug, Default, Clone)]
pub struct SearchQuery {
    /// Regex matched against the body (or the thread title when `titles_only`).
    pub pattern: Option<Regex>,
    /// Author substring (case-insensitive).
    pub author: Option<String>,
    /// Labels that must all be present.
    pub labels: Vec<String>,
    /// Restrict to a thread (or a subtree, by ID prefix) e.g. `F-1` or `F-1.2`.
    pub scope: Option<String>,
    /// Only consider posts at this depth or shallower.
    pub max_depth: Option<usize>,
    /// Match only thread titles (root posts).
    pub titles_only: bool,
    /// Cap the number of results.
    pub limit: Option<usize>,
}

/// Compile a search pattern. `case_insensitive` adds the `i` flag; multi-line
/// mode is on so `^`/`$` anchor to each line of a multi-line post body (the
/// intuitive behavior for line-oriented content). Rust's `regex` is linear-time,
/// so user patterns cannot cause catastrophic backtracking / ReDoS.
pub fn compile_pattern(pat: &str, case_insensitive: bool) -> Result<Regex> {
    regex::RegexBuilder::new(pat)
        .case_insensitive(case_insensitive)
        .multi_line(true)
        .build()
        .map_err(|e| Error::msg(format!("invalid search pattern: {e}")))
}

/// True if `id` is within the `scope` thread/subtree (prefix match on dotted IDs).
/// The trailing `.` makes `F-1` match `F-1.2` but not `F-10`.
fn in_scope(id: &str, scope: &str) -> bool {
    id == scope || id.starts_with(&format!("{scope}."))
}

/// Whether a post matches a query (all set filters AND together). Shared by
/// [`search`] and `wipe forum watch` so both apply identical semantics.
pub fn matches(p: &PostView, q: &SearchQuery) -> bool {
    if q.titles_only && p.depth != 0 {
        return false;
    }
    if let Some(scope) = &q.scope {
        if !in_scope(&p.id, scope) {
            return false;
        }
    }
    if let Some(md) = q.max_depth {
        if p.depth > md {
            return false;
        }
    }
    if let Some(a) = &q.author {
        if !p.author.to_lowercase().contains(&a.to_lowercase()) {
            return false;
        }
    }
    if !q.labels.iter().all(|l| p.labels.contains(l)) {
        return false;
    }
    if let Some(re) = &q.pattern {
        let hit = if q.titles_only {
            re.is_match(&p.thread_title)
        } else {
            // A plain search also finds a thread by its title (via the root post),
            // so title keywords are discoverable without hiding replies.
            re.is_match(&p.body) || (p.depth == 0 && re.is_match(&p.thread_title))
        };
        if !hit {
            return false;
        }
    }
    true
}

/// Run a search over the (cached) flattened forum.
pub fn search(store: &Store, q: &SearchQuery) -> Result<Vec<PostView>> {
    let mut out: Vec<PostView> = index(store)?
        .into_iter()
        .filter(|p| matches(p, q))
        .collect();
    if let Some(n) = q.limit {
        out.truncate(n);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 3, 12, 0, 0).unwrap()
    }

    fn project() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::init(dir.path(), "Forum", now()).unwrap();
        (dir, store)
    }

    fn post(body: &str) -> NewReply {
        NewReply {
            body: body.into(),
            ..Default::default()
        }
    }

    #[test]
    fn threads_and_replies_get_dotted_ids() {
        let (_d, s) = project();
        let t = create_thread(
            &s,
            NewThread {
                title: "Decisions".into(),
                body: "Use OAuth".into(),
                labels: vec!["decision".into()],
                ..Default::default()
            },
            "ada",
            now(),
        )
        .unwrap();
        assert_eq!(t.id, "F-1");
        assert_eq!(t.root.id, "F-1");

        let r1 = reply(&s, "F-1", post("agree"), "bob", now()).unwrap();
        let r2 = reply(&s, "F-1", post("also"), "cara", now()).unwrap();
        assert_eq!(r1, "F-1.1");
        assert_eq!(r2, "F-1.2");
        let nested = reply(&s, "F-1.1", post("why?"), "dan", now()).unwrap();
        assert_eq!(nested, "F-1.1.1");

        // A second thread starts a fresh counter.
        let t2 = create_thread(
            &s,
            NewThread {
                title: "Gotchas".into(),
                body: "watch the cache".into(),
                ..Default::default()
            },
            "ada",
            now(),
        )
        .unwrap();
        assert_eq!(t2.id, "F-2");
    }

    #[test]
    fn deleting_a_post_removes_its_whole_subtree() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "T".into(),
                body: "root".into(),
                ..Default::default()
            },
            "ada",
            now(),
        )
        .unwrap();
        reply(&s, "F-1", post("a"), "b", now()).unwrap(); // F-1.1
        reply(&s, "F-1.1", post("a.a"), "b", now()).unwrap(); // F-1.1.1
        reply(&s, "F-1", post("c"), "b", now()).unwrap(); // F-1.2

        // Delete F-1.1 -> F-1.1 and F-1.1.1 gone, F-1 and F-1.2 remain.
        delete_post(&s, "F-1.1", now()).unwrap();
        let ids: Vec<String> = index(&s).unwrap().into_iter().map(|p| p.id).collect();
        assert!(ids.contains(&"F-1".to_string()));
        assert!(ids.contains(&"F-1.2".to_string()));
        assert!(!ids.iter().any(|i| i.starts_with("F-1.1")));

        // Deleting the root removes the whole thread file.
        delete_post(&s, "F-1", now()).unwrap();
        assert!(matches!(
            s.load_thread("F-1"),
            Err(Error::ThreadNotFound(_))
        ));
        assert!(index(&s).unwrap().is_empty());
    }

    #[test]
    fn search_filters_by_pattern_author_label_and_scope() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "Auth design".into(),
                body: "We will use OAuth 2.1 with PKCE".into(),
                labels: vec!["decision".into()],
                ..Default::default()
            },
            "ada@x.com",
            now(),
        )
        .unwrap();
        reply(
            &s,
            "F-1",
            NewReply {
                body: "beware token refresh races".into(),
                labels: vec!["gotcha".into()],
                ..Default::default()
            },
            "claude",
            now(),
        )
        .unwrap();
        create_thread(
            &s,
            NewThread {
                title: "Unrelated".into(),
                body: "nothing to see".into(),
                ..Default::default()
            },
            "bob",
            now(),
        )
        .unwrap();

        // Regex on body.
        let hits = search(
            &s,
            &SearchQuery {
                pattern: Some(compile_pattern("OAuth|token", false).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 2);

        // Author filter.
        let by_agent = search(
            &s,
            &SearchQuery {
                author: Some("claude".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_agent.len(), 1);
        assert_eq!(by_agent[0].id, "F-1.1");

        // Label filter.
        let gotchas = search(
            &s,
            &SearchQuery {
                labels: vec!["gotcha".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(gotchas.len(), 1);

        // Titles only.
        let titles = search(
            &s,
            &SearchQuery {
                pattern: Some(compile_pattern("auth", true).unwrap()),
                titles_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].id, "F-1");

        // Scope to a thread subtree.
        let scoped = search(
            &s,
            &SearchQuery {
                scope: Some("F-1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(scoped.len(), 2);
    }

    #[test]
    fn default_search_finds_threads_by_title() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "Auth design".into(),
                body: "OAuth 2.1".into(),
                ..Default::default()
            },
            "a",
            now(),
        )
        .unwrap();
        reply(&s, "F-1", post("a reply"), "b", now()).unwrap();
        // "design" appears only in the title; a plain search still finds the root.
        let hits = search(
            &s,
            &SearchQuery {
                pattern: Some(compile_pattern("design", true).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "F-1");
    }

    #[test]
    fn search_anchors_are_line_scoped() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "T".into(),
                body: "context line\nERROR: boom".into(),
                ..Default::default()
            },
            "a",
            now(),
        )
        .unwrap();
        // `^ERROR` must match a line inside a multi-line body.
        let hits = search(
            &s,
            &SearchQuery {
                pattern: Some(compile_pattern("^ERROR", false).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn create_thread_never_reuses_an_id() {
        let (_d, s) = project();
        let t1 = create_thread(
            &s,
            NewThread {
                title: "T".into(),
                body: "one".into(),
                ..Default::default()
            },
            "a",
            now(),
        )
        .unwrap();
        assert_eq!(t1.id, "F-1");
        // Simulate board/forum divergence: rewind the counter while F-1.json stays.
        let mut b = s.load_board().unwrap();
        b.next_thread = 1;
        s.save_board(&b).unwrap();
        let t2 = create_thread(
            &s,
            NewThread {
                title: "T2".into(),
                body: "two".into(),
                ..Default::default()
            },
            "a",
            now(),
        )
        .unwrap();
        // Must skip the existing F-1 and allocate F-2, not overwrite F-1.
        assert_eq!(t2.id, "F-2");
        assert_eq!(s.load_thread("F-1").unwrap().root.body, "one");
    }

    #[test]
    fn path_traversal_ids_are_rejected() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "T".into(),
                body: "x".into(),
                ..Default::default()
            },
            "a",
            now(),
        )
        .unwrap();
        for bad in [
            "F-1/../../secret",
            "..",
            "F-1/../F-1",
            "/etc/passwd",
            "F-1\\..\\x",
        ] {
            assert!(
                get_thread(&s, bad).is_err(),
                "get_thread({bad}) should error"
            );
            assert!(
                reply(&s, bad, post("x"), "a", now()).is_err(),
                "reply({bad}) should error"
            );
            assert!(
                delete_post(&s, bad, now()).is_err(),
                "delete_post({bad}) should error"
            );
        }
        // The legitimate thread is untouched.
        assert!(get_thread(&s, "F-1").is_ok());
    }

    #[test]
    fn index_cache_reflects_new_posts() {
        let (_d, s) = project();
        create_thread(
            &s,
            NewThread {
                title: "T".into(),
                body: "one".into(),
                ..Default::default()
            },
            "ada",
            now(),
        )
        .unwrap();
        assert_eq!(index(&s).unwrap().len(), 1); // builds + caches
        assert_eq!(index(&s).unwrap().len(), 1); // served from cache
        reply(&s, "F-1", post("two"), "bob", now()).unwrap();
        // Signature changed -> cache rebuilt -> new post visible.
        assert_eq!(index(&s).unwrap().len(), 2);
    }
}
