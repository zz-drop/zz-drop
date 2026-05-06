//! Context-driven completion provider. Reads the cursor context
//! and emits a list of [`Candidate`]s. The shell script formats
//! them; ranking + content live here.
//!
//! Design reference: `cli-autosuggest.md` §5 (state ranking) and
//! §10 (mockups M1–M11). Chunk C only emits verbs and local
//! candidates — `file_remote` / `dir_remote` arrive in chunk E
//! once the agent serves `LIST_REMOTE`.

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use zz_drop_core::agent_proto::{EntryKindFilter, RemoteKind, RemoteListEntry};

use crate::sacs::render::{Candidate, Kind};
use crate::sacs::state::SacsState;

/// Maximum number of remote candidates surfaced per dropdown
/// position. Independent of the agent-side hard cap (`max_results = 200`
/// enforced by `sacs::agent_source`) so the dropdown stays
/// readable even when the agent has hundreds of entries cached.
/// 50 fits comfortably with menu-select scrolling on a typical
/// terminal, and any spill is summarised by the `footer` row.
const REMOTE_CANDIDATE_LIMIT: usize = 50;

/// Abstraction over the agent's `LIST_REMOTE` endpoint. The
/// production wiring in `sacs/mod.rs` connects to the agent via a
/// `AgentClient`; tests inject a deterministic fake. The trait is
/// intentionally tiny — list one prefix, no other state — so a
/// fake fits in ~30 lines.
pub trait RemoteListSource {
    fn list(
        &mut self,
        prefix: Option<&str>,
        kind_filter: EntryKindFilter,
    ) -> Result<Vec<RemoteListEntry>, RemoteListError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteListError {
    /// The agent reported the unlocked container is locked. Per
    /// design §9 the completion treats this as "no remote
    /// candidates this time", never as a hard error.
    NotUnlocked,
    /// Provider error (network, auth, transient 503, etc.). Same
    /// treatment as `NotUnlocked` from the operator's point of
    /// view — drop the remote candidates silently.
    Provider,
    /// Agent socket missing or refused the connection. We are in
    /// S0/S1; the caller already knows that and the remote
    /// candidates are out of scope anyway.
    Unreachable,
}

/// Maximum number of file/dir candidates to emit before
/// collapsing the rest into a `+N more` footer. Keeps the
/// dropdown legible in any shell.
/// Maximum number of local files/directories surfaced per
/// dropdown position. Mirrors `REMOTE_CANDIDATE_LIMIT`; spill is
/// reported via the `footer` row so the operator knows there is
/// more material if they keep typing to narrow.
const LOCAL_CANDIDATE_LIMIT: usize = 50;

/// What `__complete` was called with, after stripping the
/// `--cwd <path>` hidden flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteArgs {
    pub cwd: PathBuf,
    pub tokens: Vec<String>,
}

/// Parse the raw `argv[1..]` of `zz __complete` into a
/// [`CompleteArgs`]. The hidden `--cwd <path>` flag lets tests
/// run without `chdir` — anywhere else, `cwd` defaults to the
/// process working directory.
pub fn parse_args(argv: &[String]) -> CompleteArgs {
    let mut cwd: Option<PathBuf> = None;
    let mut tokens: Vec<String> = Vec::with_capacity(argv.len());
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--cwd" {
            if let Some(p) = argv.get(i + 1) {
                cwd = Some(PathBuf::from(p));
                i += 2;
                continue;
            }
        }
        tokens.push(a.clone());
        i += 1;
    }
    CompleteArgs {
        cwd: cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        tokens,
    }
}

/// Build the candidate list for the given state + cursor
/// context. Pure function over its inputs (apart from the
/// optional remote-list source) — easy to snapshot.
///
/// Pass `Some(source)` to surface `file_remote` / `dir_remote`
/// candidates. Pass `None` for chunk-C-style local-only output;
/// the completer never blocks on a missing source.
pub fn run(
    state: SacsState,
    args: &CompleteArgs,
    source: Option<&mut dyn RemoteListSource>,
) -> Vec<Candidate> {
    let cur = current_word(&args.tokens);
    let prev = previous_tokens(&args.tokens);

    // S0/S1/S2 ignore the cursor context entirely: the user
    // can't usefully upload until they unlock, so the dropdown
    // stays focused on the unlock/setup verbs. M1, M2, M3 in the
    // design.
    match state {
        SacsState::S0Fresh => return setup_verbs_for_fresh(cur),
        SacsState::S1Down => return setup_verbs_for_down(cur),
        SacsState::S2Locked => return setup_verbs_for_locked(cur),
        SacsState::S3Ready | SacsState::S4ReadyDual => {}
    }

    // S3/S4: cursor context matters. The rules are exhaustive
    // over the command grammar, so any future change to the
    // parser is forced through this match.
    if prev.is_empty() {
        return ready_first_arg(state, cur, &args.cwd);
    }
    let head = prev[0].as_str();
    match head {
        // Composite verbs: dispatch by modifier set.
        h if h.starts_with('s') && is_composite_token(h) => {
            ready_after_s(h, &prev[1..], cur, &args.cwd, state, source)
        }
        h if h.starts_with('d') && is_composite_token(h) => {
            ready_after_d(h, &prev[1..], cur, &args.cwd, state, source)
        }
        // Atomic verbs that take no further completion: empty.
        "q" | "w" | "kalvasflam" | "c" | "f" => Vec::new(),
        // `z` accepts at most one positional (`local`/`remote`):
        // when the user is on the second positional, no more
        // suggestions.
        "z" => z_argument(&prev[1..], cur, state),
        // First token isn't a verb → it's a path → user is doing
        // a default-verb upload (`zz file.md other.md`). Treat
        // like the multi-source upload case.
        _ => ready_after_default_upload(&prev, cur, &args.cwd, source),
    }
}

// ---------------------------------------------------------------------------
// State-specific first-arg rankings (M1, M2, M3, M7).
// ---------------------------------------------------------------------------

fn setup_verbs_for_fresh(cur: &str) -> Vec<Candidate> {
    // M1: highlight `c` first, then the standard atomic verbs.
    let raw = [
        ("c", Kind::Atomic, 1, "open the configuration TUI"),
        ("--help", Kind::Help, 2, "show the cheat sheet"),
    ];
    filter_atomic(&raw, cur)
}

fn setup_verbs_for_down(cur: &str) -> Vec<Candidate> {
    let raw = [
        ("z", Kind::Atomic, 1, "unlock the active container"),
        ("c", Kind::Atomic, 2, "open the configuration TUI"),
        ("q", Kind::Atomic, 3, "lock the agent"),
        ("w", Kind::Atomic, 4, "wipe local zz-drop state"),
        ("f", Kind::Atomic, 5, "doctor / diagnostics"),
    ];
    filter_atomic(&raw, cur)
}

fn setup_verbs_for_locked(cur: &str) -> Vec<Candidate> {
    let raw = [
        ("z", Kind::Atomic, 1, "unlock the active container"),
        ("q", Kind::Atomic, 2, "lock the agent"),
        ("c", Kind::Atomic, 3, "open the configuration TUI"),
        ("w", Kind::Atomic, 4, "wipe local zz-drop state"),
        ("f", Kind::Atomic, 5, "doctor / diagnostics"),
    ];
    filter_atomic(&raw, cur)
}

fn ready_first_arg(state: SacsState, cur: &str, cwd: &Path) -> Vec<Candidate> {
    // M2 / M3: when the user is composing a composite verb
    // (`s`, `sa`, `sx`, `d`, `dx`, `darx`, …), drop the
    // file_local stream — they are not picking a path right
    // now. Without this guard, `zz d<TAB>` would surface
    // `draft.md` next to the verb expansions and confuse the
    // mental model in design §10.
    let composing_verb = !cur.is_empty() && is_composite_token(cur);

    // M7: in S3/S4 ready, "first arg" = primary upload source.
    // Local files come first, then local directories so the
    // operator can navigate (`zz A<TAB>` completes to
    // `Applications/` and a follow-up TAB descends), then
    // composite verbs, then the dual-container `z local` /
    // `z remote` pair when relevant.
    let mut out = Vec::new();
    if !composing_verb {
        let files = collect_local_files(cwd, cur);
        out.extend(rank_local_files(files, "upload", 1, true));
        // Local dirs with trailing `/` so zsh keeps the cursor
        // ready for the next TAB. Empty description → zsh packs
        // dirs into a multi-column grid below the file rows.
        let mut dirs = collect_local(cwd, cur, EntryFilter::DirsOnly);
        dirs.sort_by_key(|e| (Reverse(e.mtime), e.name.clone()));
        let dir_total = dirs.len();
        for (i, e) in dirs.iter().take(LOCAL_CANDIDATE_LIMIT).enumerate() {
            let value = format!("{}/", e.name);
            out.push(Candidate {
                value: value.clone(),
                display: format!("zz {value}"),
                description: String::new(),
                kind: Kind::DirLocal,
                rank: 50 + (i as u32),
            });
        }
        if dir_total > LOCAL_CANDIDATE_LIMIT {
            out.push(footer(
                dir_total - LOCAL_CANDIDATE_LIMIT,
                dir_total,
                "directories",
            ));
        }
    }

    // After the local files, expose the composite verbs the user
    // might still want to type. Ranks start past the file block
    // so files always sort first.
    let verb_rank_base = 100;
    out.extend(filter_starting_with(
        &[
            ("s", Kind::Verb, verb_rank_base, "explicit upload · press space for files"),
            ("d", Kind::Verb, verb_rank_base + 1, "download · press space for files"),
            ("sa", Kind::Verb, verb_rank_base + 2, "+ bulk top-level (dir)"),
            ("sx", Kind::Verb, verb_rank_base + 3, "+ zstd compress"),
            ("dx", Kind::Verb, verb_rank_base + 4, "+ decompress"),
            ("sarx", Kind::Verb, verb_rank_base + 5, "+ recursive bundle + zstd"),
            ("darx", Kind::Verb, verb_rank_base + 6, "+ recursive + decompress"),
        ],
        cur,
    ));

    // Atomic verbs at the bottom — useful but not the daily path.
    out.extend(filter_atomic(
        &[
            ("q", Kind::Atomic, 200, "lock the agent"),
            ("c", Kind::Atomic, 201, "open the configuration TUI"),
            ("f", Kind::Atomic, 202, "doctor / diagnostics"),
        ],
        cur,
    ));

    // `z` is the unlock atomic — same `Kind::Atomic` as `q`, `c`,
    // `f` so the zsh dropdown groups it under `[atomic command]`,
    // not `[verb]`. The `z local` / `z remote` variants stay
    // atomic too (they're disambiguators of the same unlock
    // action, not a separate verb family).
    if matches!(state, SacsState::S4ReadyDual) {
        out.extend(filter_atomic(
            &[
                ("z", Kind::Atomic, 210, "unlock active container (auto)"),
                ("z local", Kind::Atomic, 211, "unlock the local container"),
                ("z remote", Kind::Atomic, 212, "unlock the remote container"),
            ],
            cur,
        ));
    } else {
        out.extend(filter_atomic(
            &[("z", Kind::Atomic, 210, "unlock active container")],
            cur,
        ));
    }

    out
}

// ---------------------------------------------------------------------------
// Composite verb dispatch. M4, M6, M9 today; M5/M10/M11 in chunk E.
// ---------------------------------------------------------------------------

/// Test whether a token "looks like" a composite `s`/`d` form
/// per the grammar in `cli/parser.rs::is_reserved`. Same
/// alphabet (`a`, `r`, `x`, `e`) + a head of `s` or `d`. The
/// completion provider re-implements the test rather than
/// pulling on `cli::parser` to keep the module independently
/// testable; the cli_parser tests still own correctness.
fn is_composite_token(t: &str) -> bool {
    let mut chars = t.chars();
    let head = match chars.next() {
        Some(c @ ('s' | 'd')) => c,
        _ => return false,
    };
    let _ = head; // head identity already checked above
    chars.all(|c| matches!(c, 'a' | 'r' | 'x' | 'e'))
}

fn ready_after_s(
    verb: &str,
    after: &[String],
    cur: &str,
    cwd: &Path,
    _state: SacsState,
    source: Option<&mut dyn RemoteListSource>,
) -> Vec<Candidate> {
    let modifiers = parse_modifiers(verb);
    if modifiers.a {
        // M6 / M10: `sa` / `sar` / `sax` / `sarx` take a 1st
        // local directory and an optional 2nd remote-prefix.
        if after.is_empty() {
            return collect_local_dirs_ranked(cwd, cur, "bulk upload top-level");
        }
        // Operator already chose the local dir. The 2nd
        // positional is a remote-prefix from the unlocked
        // container. Empty when the agent reports locked /
        // missing.
        return rank_remote_dirs(
            try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Directory),
            verb,
            cur,
            "remote-prefix",
            1,
        );
    }
    // M9: plain `s` / `sx`. Without source files yet → list
    // local files (newest first) AND local directories so the
    // operator can navigate (`zz s Documents/<TAB>` lists the
    // contents of `Documents/`). Without dirs in the dropdown,
    // a cwd with a single file would silently auto-insert that
    // file on the first TAB. With at least one source already
    // chosen → mix more local files (rank 1..) with
    // `dir_remote` "close as destination" candidates pulled
    // from the active container.
    if after.is_empty() {
        let label = if modifiers.x { "upload + zstd compress" } else { "upload" };
        let mut out = rank_local_files(collect_local_files(cwd, cur), label, 1, true);
        // Local directories ranked after files. Dir values carry
        // a trailing `/` so a follow-up TAB descends into them
        // via the path-aware branch in `collect_local`.
        let mut dirs = collect_local(cwd, cur, EntryFilter::DirsOnly);
        dirs.sort_by_key(|e| (Reverse(e.mtime), e.name.clone()));
        let dir_total = dirs.len();
        for (i, e) in dirs.iter().take(LOCAL_CANDIDATE_LIMIT).enumerate() {
            let value = format!("{}/", e.name);
            out.push(Candidate {
                value: value.clone(),
                display: format!("zz {value}"),
                // Empty description → zsh packs dirs into a
                // multi-column grid below the file rows. The
                // "[local dir]" group header already conveys
                // the kind.
                description: String::new(),
                kind: Kind::DirLocal,
                rank: 100 + (i as u32),
            });
        }
        if dir_total > LOCAL_CANDIDATE_LIMIT {
            out.push(footer(
                dir_total - LOCAL_CANDIDATE_LIMIT,
                dir_total,
                "directories",
            ));
        }
        return out;
    }
    let mut out = rank_local_files(
        collect_local_files(cwd, cur),
        "add as source",
        1,
        false,
    );
    out.extend(rank_remote_dirs(
        try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Directory),
        verb,
        cur,
        "remote-dir · close as destination",
        50,
    ));
    out
}

fn ready_after_d(
    verb: &str,
    after: &[String],
    cur: &str,
    cwd: &Path,
    _state: SacsState,
    source: Option<&mut dyn RemoteListSource>,
) -> Vec<Candidate> {
    let modifiers = parse_modifiers(verb);
    if modifiers.a {
        // `da` / `dar`: 1st positional = local-dest, optional;
        // 2nd = remote-prefix. M11.
        if after.is_empty() {
            return collect_local_dirs_ranked(cwd, cur, "bulk download into");
        }
        return rank_remote_dirs(
            try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Directory),
            verb,
            cur,
            "remote-prefix",
            1,
        );
    }
    // M5: `d` / `dx` first arg is a remote filename. Locked
    // agent → empty list, no error. Surface remote dirs alongside
    // the files so the operator can navigate (`zz d Documents/<TAB>`
    // descends into `Documents/`).
    if after.is_empty() {
        let entries = try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Both);
        let label = if modifiers.x {
            "download + decompress · remote"
        } else {
            "download (raw bytes) · remote"
        };
        let mut out = rank_remote_files(entries.clone(), verb, cur, label, 1);
        // Remote dirs ranked after the files (rank base 50). Dir
        // values carry a trailing `/` so a follow-up TAB descends
        // into them via the path-aware branch in
        // `parent_for_remote_prefix`.
        out.extend(rank_remote_dirs(entries, verb, cur, "remote-dir · navigate", 50));
        return out;
    }
    // Subsequent positionals: more remote names + `<local-dir>/`
    // (local dirs) as "close as destination".
    let mut out = rank_remote_files(
        try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Both),
        verb,
        cur,
        "add as source · remote",
        1,
    );
    // Mix in local directories the operator might want to use as
    // download destination, ranked below the file candidates so
    // typing keeps narrowing files first.
    let local_dirs = collect_local_dirs_ranked(cwd, cur, "local-dir · close as destination");
    out.extend(local_dirs.into_iter().map(|mut c| {
        c.rank = c.rank.saturating_add(50);
        c
    }));
    out
}

fn ready_after_default_upload(
    prev: &[String],
    cur: &str,
    cwd: &Path,
    source: Option<&mut dyn RemoteListSource>,
) -> Vec<Candidate> {
    // No verb token: every previous token is a source path.
    let _ = prev;
    let mut out = rank_local_files(collect_local_files(cwd, cur), "add as source", 1, false);
    out.extend(rank_remote_dirs(
        try_list_remote(source, parent_for_remote_prefix(cur), EntryKindFilter::Directory),
        "",
        cur,
        "remote-dir · close as destination",
        50,
    ));
    out
}

fn z_argument(after: &[String], cur: &str, state: SacsState) -> Vec<Candidate> {
    if !after.is_empty() {
        return Vec::new();
    }
    let mut raw: Vec<(&str, Kind, u32, &str)> =
        vec![("local", Kind::Atomic, 1, "force the local container")];
    if matches!(state, SacsState::S4ReadyDual) {
        raw.push(("remote", Kind::Atomic, 2, "force the remote container"));
    }
    filter_atomic(&raw, cur)
}

#[derive(Default, Debug, Clone, Copy)]
struct Modifiers {
    a: bool,
    r: bool,
    x: bool,
}

fn parse_modifiers(verb: &str) -> Modifiers {
    let mut m = Modifiers::default();
    let tail = verb.get(1..).unwrap_or("");
    for c in tail.chars() {
        match c {
            'a' => m.a = true,
            'r' => m.r = true,
            'x' => m.x = true,
            _ => {}
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Filesystem walk helpers. Bounded to LOCAL_CANDIDATE_LIMIT to
// keep the dropdown readable; spillover goes into a `footer`.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LocalEntry {
    name: String,
    is_dir: bool,
    size: u64,
    mtime: SystemTime,
}

fn collect_local_files(cwd: &Path, prefix: &str) -> Vec<LocalEntry> {
    collect_local(cwd, prefix, EntryFilter::FilesOnly)
}

fn collect_local_dirs_ranked(cwd: &Path, prefix: &str, label: &str) -> Vec<Candidate> {
    let mut entries = collect_local(cwd, prefix, EntryFilter::DirsOnly);
    entries.sort_by_key(|e| (Reverse(e.mtime), e.name.clone()));
    let total = entries.len();
    let mut out = Vec::new();
    // No per-row description: zsh would otherwise split the
    // dropdown into "one wide row + multi-column block" or
    // "multi-column block with a trailing description tail",
    // both of which look messy when many dirs share the same
    // verb context. With empty descriptions zsh packs the dirs
    // into a clean multi-column grid; the verb label is already
    // visible in the section header (`[local dir]`) and "newest
    // first" is implied by sort order.
    let _ = label; // intentionally unused — kept so the call signature stays stable across callers
    for (i, e) in entries.iter().take(LOCAL_CANDIDATE_LIMIT).enumerate() {
        let rank = (i as u32) + 1;
        let value = format!("{}/", e.name);
        out.push(Candidate {
            value: value.clone(),
            display: format!("zz sa {value}"),
            description: String::new(),
            kind: Kind::DirLocal,
            rank,
        });
    }
    if total > LOCAL_CANDIDATE_LIMIT {
        out.push(footer(total - LOCAL_CANDIDATE_LIMIT, total, "directories"));
    }
    out
}

#[derive(Clone, Copy)]
enum EntryFilter {
    FilesOnly,
    DirsOnly,
}

/// Split a path-shaped prefix into the directory portion and the
/// trailing partial basename. The directory portion includes its
/// trailing slash, so `"Documents/foo"` → (`"Documents/"`, `"foo"`)
/// and `"Documents/"` → (`"Documents/"`, `""`). Without a slash
/// the directory portion is empty.
fn split_path_prefix(prefix: &str) -> (&str, &str) {
    match prefix.rfind('/') {
        Some(i) => (&prefix[..=i], &prefix[i + 1..]),
        None => ("", prefix),
    }
}

fn collect_local(cwd: &Path, prefix: &str, filter: EntryFilter) -> Vec<LocalEntry> {
    // Path-aware completion: when the operator has typed
    // `Documents/sub`, peel off `Documents/` and read that
    // subdirectory instead of `cwd`. Returned names retain the
    // typed prefix so the candidate value is the full relative
    // path the operator can keep extending.
    let (dir_part, base) = split_path_prefix(prefix);
    let read_root: PathBuf = if dir_part.is_empty() {
        cwd.to_path_buf()
    } else {
        cwd.join(dir_part)
    };
    let read = match fs::read_dir(&read_root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if name.starts_with('.') {
            // Hidden entries (`.git/`, dotfiles) — same skip
            // policy as the upload commands.
            continue;
        }
        if !base.is_empty() && !name.starts_with(base) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let is_dir = metadata.is_dir();
        match filter {
            EntryFilter::FilesOnly if is_dir => continue,
            EntryFilter::DirsOnly if !is_dir => continue,
            _ => {}
        }
        out.push(LocalEntry {
            name: format!("{dir_part}{name}"),
            is_dir,
            size: metadata.len(),
            mtime: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    out
}

fn rank_local_files(
    mut entries: Vec<LocalEntry>,
    upload_label: &str,
    rank_base: u32,
    annotate_newest: bool,
) -> Vec<Candidate> {
    entries.sort_by_key(|e| (Reverse(e.mtime), e.name.clone()));
    let total = entries.len();
    let mut out = Vec::new();
    for (i, e) in entries.iter().take(LOCAL_CANDIDATE_LIMIT).enumerate() {
        let rank = rank_base + (i as u32);
        let value = if e.is_dir {
            format!("{}/", e.name)
        } else {
            e.name.clone()
        };
        let description = if e.is_dir {
            "directory · navigate".to_string()
        } else {
            let mut d = format!("{} · {}", upload_label, format_size(e.size));
            if annotate_newest && i == 0 {
                d.push_str(" · newest");
            }
            d
        };
        out.push(Candidate {
            value: value.clone(),
            display: format!("zz {value}"),
            description,
            kind: if e.is_dir { Kind::DirLocal } else { Kind::FileLocal },
            rank,
        });
    }
    if total > LOCAL_CANDIDATE_LIMIT {
        out.push(footer(total - LOCAL_CANDIDATE_LIMIT, total, "files"));
    }
    out
}

fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    }
}

// ---------------------------------------------------------------------------
// Remote helpers (chunk E). Talk to the agent via `RemoteListSource`,
// translate the response into ranked Candidates. Empty/Err results
// are silenced by design.
// ---------------------------------------------------------------------------

/// Compute the prefix to ask the agent for, given the cursor word
/// the operator is currently typing. `notes` → query root and let
/// the shell filter; `docs/sub` → query `docs/`. We match on the
/// last `/` because everything after it is the "current segment"
/// the operator is in the middle of typing.
fn parent_for_remote_prefix(cur: &str) -> Option<String> {
    if cur.is_empty() || !cur.contains('/') {
        return None;
    }
    let trimmed = cur.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

/// Run the source if given; swallow every error variant. The
/// completer never surfaces remote errors as a non-zero exit; we
/// just emit fewer candidates.
fn try_list_remote(
    source: Option<&mut dyn RemoteListSource>,
    prefix: Option<String>,
    kind_filter: EntryKindFilter,
) -> Vec<RemoteListEntry> {
    let Some(s) = source else { return Vec::new() };
    s.list(prefix.as_deref(), kind_filter).unwrap_or_default()
}

fn rank_remote_files(
    entries: Vec<RemoteListEntry>,
    verb: &str,
    cur: &str,
    label: &str,
    rank_base: u32,
) -> Vec<Candidate> {
    let last_segment = cur_last_segment(cur);
    let mut filtered: Vec<_> = entries
        .into_iter()
        .filter(|e| matches!(e.kind, RemoteKind::File))
        .filter(|e| last_segment.is_empty() || e.name.starts_with(last_segment))
        .collect();
    filtered.sort_by(|a, b| a.name.cmp(&b.name));
    let total = filtered.len();
    let mut out = Vec::new();
    for (i, e) in filtered.iter().take(REMOTE_CANDIDATE_LIMIT).enumerate() {
        let value = remote_join(cur, &e.name);
        let rank = rank_base + (i as u32);
        // Including the per-entry size makes descriptions
        // distinct, so zsh keeps the dropdown in single column
        // (each row carries its own label). When the provider
        // omits sizes, all rows fall back to the bare label and
        // zsh packs them into a multi-column grid — still
        // readable, just denser.
        let description = match e.size {
            Some(b) => format!("{label} · {}", format_size(b)),
            None => label.to_string(),
        };
        out.push(Candidate {
            value,
            display: format!("{} {}", verb_display(verb), &e.name),
            description,
            kind: Kind::FileRemote,
            rank,
        });
    }
    if total > REMOTE_CANDIDATE_LIMIT {
        out.push(footer(total - REMOTE_CANDIDATE_LIMIT, total, "remote files"));
    }
    out
}

fn rank_remote_dirs(
    entries: Vec<RemoteListEntry>,
    verb: &str,
    cur: &str,
    label: &str,
    rank_base: u32,
) -> Vec<Candidate> {
    let last_segment = cur_last_segment(cur);
    let mut filtered: Vec<_> = entries
        .into_iter()
        .filter(|e| matches!(e.kind, RemoteKind::Directory))
        .filter(|e| last_segment.is_empty() || e.name.starts_with(last_segment))
        .collect();
    filtered.sort_by(|a, b| a.name.cmp(&b.name));
    let total = filtered.len();
    let mut out = Vec::new();
    let _ = label; // reserved for future per-entry annotation; today providers don't ship sizes for dirs
    for (i, e) in filtered.iter().take(REMOTE_CANDIDATE_LIMIT).enumerate() {
        let mut value = remote_join(cur, &e.name);
        if !value.ends_with('/') {
            value.push('/');
        }
        let rank = rank_base + (i as u32);
        out.push(Candidate {
            value,
            display: format!("{} {}/", verb_display(verb), &e.name),
            // Empty description → multi-column grid, same
            // policy as local dirs in `collect_local_dirs_ranked`.
            // The `[remote dir]` section header already conveys
            // the kind.
            description: String::new(),
            kind: Kind::DirRemote,
            rank,
        });
    }
    if total > REMOTE_CANDIDATE_LIMIT {
        out.push(footer(total - REMOTE_CANDIDATE_LIMIT, total, "remote dirs"));
    }
    out
}

/// Trailing segment of the cursor word, used to filter the agent
/// response client-side: the operator typing `docs/api` against a
/// prefix of `docs/` means "show me names under docs/ that start
/// with `api`".
fn cur_last_segment(cur: &str) -> &str {
    match cur.rfind('/') {
        Some(i) => &cur[i + 1..],
        None => cur,
    }
}

/// Join a parent-path prefix with a child name for the dropdown
/// `value` so the inserted text is the full remote path the
/// operator was building.
fn remote_join(cur: &str, child: &str) -> String {
    if let Some(i) = cur.rfind('/') {
        format!("{}/{}", &cur[..i], child)
    } else {
        child.to_string()
    }
}

/// Display prefix for a candidate. `verb_display("s")` → `"zz s"`;
/// `verb_display("")` → `"zz"` (default verb upload).
fn verb_display(verb: &str) -> String {
    if verb.is_empty() {
        "zz".to_string()
    } else {
        format!("zz {verb}")
    }
}

fn footer(extra: usize, total: usize, label: &str) -> Candidate {
    Candidate {
        value: String::new(),
        display: String::new(),
        description: format!(
            "+{extra} more matches — keep typing to narrow  {total} {label} total · newest first"
        ),
        kind: Kind::Footer,
        rank: u32::MAX,
    }
}

fn current_word(tokens: &[String]) -> &str {
    tokens.last().map(|s| s.as_str()).unwrap_or("")
}

fn previous_tokens(tokens: &[String]) -> &[String] {
    if tokens.is_empty() {
        return &[];
    }
    &tokens[..tokens.len() - 1]
}

fn filter_atomic(raw: &[(&str, Kind, u32, &str)], cur: &str) -> Vec<Candidate> {
    raw.iter()
        .filter(|(v, _, _, _)| cur.is_empty() || v.starts_with(cur))
        .map(|(value, kind, rank, description)| Candidate {
            value: value.to_string(),
            display: format!("zz {value}"),
            description: description.to_string(),
            kind: *kind,
            rank: *rank,
        })
        .collect()
}

fn filter_starting_with(raw: &[(&str, Kind, u32, &str)], cur: &str) -> Vec<Candidate> {
    filter_atomic(raw, cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tokens: &[&str], cwd: &Path) -> CompleteArgs {
        CompleteArgs {
            cwd: cwd.to_path_buf(),
            tokens: tokens.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_fixture() -> tempfile::TempDir {
        // Create a deterministic tempdir with a known set of
        // files and subdirectories.
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path();
        for name in ["readme.md", "draft.md", "notes.txt", "changelog.md"] {
            std::fs::write(p.join(name), b"x").unwrap();
        }
        for name in ["src", "docs", "vendor"] {
            std::fs::create_dir(p.join(name)).unwrap();
        }
        // A few entries inside one of the subdirs so the
        // path-navigation tests have something to discover.
        std::fs::write(p.join("docs").join("api.md"), b"x").unwrap();
        std::fs::write(p.join("docs").join("guide.md"), b"x").unwrap();
        std::fs::create_dir(p.join("docs").join("internal")).unwrap();
        // Add a hidden file to confirm we skip it.
        std::fs::write(p.join(".secret"), b"shh").unwrap();
        dir
    }

    #[test]
    fn parse_args_extracts_cwd_flag() {
        let p = parse_args(&[
            "--cwd".to_string(),
            "/tmp/foo".to_string(),
            "d".to_string(),
            "rea".to_string(),
        ]);
        assert_eq!(p.cwd, PathBuf::from("/tmp/foo"));
        assert_eq!(p.tokens, vec!["d".to_string(), "rea".to_string()]);
    }

    #[test]
    fn s0_fresh_offers_c_first() {
        let cur = make_fixture();
        let r = run(SacsState::S0Fresh, &args(&[""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert_eq!(values.first(), Some(&"c"));
        // No file/dir candidates in S0.
        assert!(!r.iter().any(|c| matches!(c.kind, Kind::FileLocal | Kind::DirLocal)));
    }

    #[test]
    fn s1_down_offers_z_first() {
        let cur = make_fixture();
        let r = run(SacsState::S1Down, &args(&[""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert_eq!(values.first(), Some(&"z"));
    }

    #[test]
    fn s3_first_arg_offers_files_then_verbs() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&[""], cur.path()), None);
        // First few candidates must be files; verbs sort after.
        let kinds: Vec<&Kind> = r.iter().map(|c| &c.kind).collect();
        let first_verb = kinds.iter().position(|k| matches!(k, Kind::Verb));
        let first_file = kinds.iter().position(|k| matches!(k, Kind::FileLocal));
        assert!(first_file.is_some(), "no FileLocal in S3 first-arg");
        assert!(
            first_file.unwrap() < first_verb.unwrap_or(usize::MAX),
            "FileLocal must appear before Verb in S3"
        );
    }

    #[test]
    fn first_arg_includes_local_dirs_for_navigation() {
        // `zz <TAB>` (no verb yet) must surface local dirs in
        // addition to local files, so `zz A<TAB>` completes to
        // `Applications/` and a follow-up TAB can descend into
        // it. Without dirs, the operator has to type the slash
        // by hand.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&[""], cur.path()), None);
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        assert!(
            kinds.contains(&Kind::DirLocal),
            "first-arg dropdown must include DirLocal, got {kinds:?}"
        );
        // Dir values must end with `/` for nested completion.
        let dir_values: Vec<&str> = r
            .iter()
            .filter(|c| matches!(c.kind, Kind::DirLocal))
            .map(|c| c.value.as_str())
            .collect();
        for v in &dir_values {
            assert!(
                v.ends_with('/'),
                "dir value {v:?} must end with '/' for navigation"
            );
        }
        // Files still rank above dirs above verbs.
        let kinds_in_order: Vec<&Kind> = r.iter().map(|c| &c.kind).collect();
        let first_file = kinds_in_order.iter().position(|k| matches!(k, Kind::FileLocal));
        let first_dir = kinds_in_order.iter().position(|k| matches!(k, Kind::DirLocal));
        let first_verb = kinds_in_order.iter().position(|k| matches!(k, Kind::Verb));
        assert!(first_file.unwrap() < first_dir.unwrap());
        assert!(first_dir.unwrap() < first_verb.unwrap_or(usize::MAX));
    }

    #[test]
    fn s4_dual_adds_z_local_and_z_remote() {
        let cur = make_fixture();
        let r = run(SacsState::S4ReadyDual, &args(&[""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(values.contains(&"z local"));
        assert!(values.contains(&"z remote"));
    }

    #[test]
    fn s3_dual_does_not_offer_z_remote() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&[""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(!values.contains(&"z remote"));
    }

    #[test]
    fn s_partial_expands_modifier_variants() {
        // `zz s<TAB>` — context: previous tokens empty, cur = "s".
        // The user is asking "which composite forms exist?". M2.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["s"], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        // All s-prefixed verbs should appear; no `d`-only verbs.
        for v in ["s", "sa", "sx", "sarx"] {
            assert!(values.contains(&v), "missing {v} in s<TAB>: {values:?}");
        }
        for v in ["d", "dx", "darx"] {
            assert!(!values.contains(&v), "unexpected {v} in s<TAB>");
        }
    }

    #[test]
    fn d_partial_expands_modifier_variants() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["d"], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        for v in ["d", "dx", "darx"] {
            assert!(values.contains(&v), "missing {v} in d<TAB>: {values:?}");
        }
    }

    #[test]
    fn sa_first_arg_is_local_dirs_only() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["sa", ""], cur.path()), None);
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        assert!(
            kinds.iter().all(|k| matches!(k, Kind::DirLocal | Kind::Footer)),
            "sa first arg should only return DirLocal/Footer, got {kinds:?}"
        );
    }

    #[test]
    fn s_first_arg_mixes_files_and_dirs() {
        // `zz s <TAB>` must surface BOTH local files and local
        // directories: dirs are not upload targets for plain `s`,
        // but they are needed as navigation aids so the dropdown
        // doesn't silently auto-insert a single file when cwd
        // contains exactly one regular file.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["s", ""], cur.path()), None);
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        assert!(
            kinds.contains(&Kind::FileLocal),
            "expected FileLocal in s first-arg, got {kinds:?}"
        );
        assert!(
            kinds.contains(&Kind::DirLocal),
            "expected DirLocal in s first-arg, got {kinds:?}"
        );
    }

    #[test]
    fn s_first_arg_dir_values_end_with_slash() {
        // Trailing `/` on dir values is what lets zsh chain
        // another TAB into the directory. Without it, the user
        // would have to type the slash manually.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["s", ""], cur.path()), None);
        let dir_values: Vec<&str> = r
            .iter()
            .filter(|c| matches!(c.kind, Kind::DirLocal))
            .map(|c| c.value.as_str())
            .collect();
        assert!(!dir_values.is_empty(), "no DirLocal entries surfaced");
        for v in &dir_values {
            assert!(
                v.ends_with('/'),
                "dir value {v:?} must end with '/' for nested completion"
            );
        }
    }

    #[test]
    fn s_navigates_into_subdirectory() {
        // `zz s docs/<TAB>` lists the contents of `docs/`. Both
        // files and dirs inside the subdir should appear, with
        // values carrying the full relative path so a follow-up
        // TAB keeps descending.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["s", "docs/"], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(
            values.iter().any(|v| *v == "docs/api.md"),
            "expected docs/api.md in {values:?}"
        );
        assert!(
            values.iter().any(|v| *v == "docs/guide.md"),
            "expected docs/guide.md in {values:?}"
        );
        assert!(
            values.iter().any(|v| *v == "docs/internal/"),
            "expected docs/internal/ (with trailing slash) in {values:?}"
        );
        // Top-level entries must NOT leak in once we've descended.
        assert!(
            !values.iter().any(|v| *v == "readme.md"),
            "top-level readme.md leaked into docs/ listing"
        );
    }

    #[test]
    fn s_partial_basename_in_subdir_filters_correctly() {
        // `zz s docs/api<TAB>` — base prefix is `api`, so only
        // entries inside `docs/` whose basename starts with `api`
        // should match.
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["s", "docs/api"], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(values.iter().any(|v| *v == "docs/api.md"));
        assert!(
            !values.iter().any(|v| *v == "docs/guide.md"),
            "guide.md should not match prefix 'api'"
        );
    }

    #[test]
    fn sx_first_arg_lists_files_newest_first() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["sx", ""], cur.path()), None);
        // First non-footer candidate must be `FileLocal` and
        // carry the "newest" annotation.
        let first = r.iter().find(|c| !matches!(c.kind, Kind::Footer)).unwrap();
        assert_eq!(first.kind, Kind::FileLocal);
        assert!(first.description.contains("newest"));
        assert!(first.description.contains("upload + zstd compress"));
    }

    #[test]
    fn z_with_one_arg_offers_local_or_remote() {
        let cur = make_fixture();
        // S3: only `local` is available.
        let r = run(SacsState::S3Ready, &args(&["z", ""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(values.contains(&"local"));
        assert!(!values.contains(&"remote"));
        // S4: both offered.
        let r = run(SacsState::S4ReadyDual, &args(&["z", ""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(values.contains(&"local"));
        assert!(values.contains(&"remote"));
    }

    #[test]
    fn z_with_two_args_returns_no_candidates() {
        let cur = make_fixture();
        let r = run(SacsState::S4ReadyDual, &args(&["z", "local", ""], cur.path()), None);
        assert!(r.is_empty());
    }

    #[test]
    fn atomic_verbs_take_no_completion() {
        let cur = make_fixture();
        for verb in ["q", "w", "kalvasflam", "c", "f"] {
            let r = run(SacsState::S3Ready, &args(&[verb, ""], cur.path()), None);
            assert!(r.is_empty(), "atomic {verb} returned: {r:?}");
        }
    }

    #[test]
    fn hidden_files_are_skipped() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&[""], cur.path()), None);
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(!values.iter().any(|v| v.starts_with('.')));
    }

    #[test]
    fn prefix_filters_local_entries() {
        let cur = make_fixture();
        let r = run(SacsState::S3Ready, &args(&["d"], cur.path()), None);
        // `d` is a verb prefix, files starting with `d` (draft.md)
        // should NOT appear because we route to the verb-expansion
        // path. Instead, M3 is in effect.
        let values: Vec<&str> = r.iter().map(|c| c.value.as_str()).collect();
        assert!(!values.iter().any(|v| *v == "draft.md"));
        assert!(values.contains(&"d"));
    }

    #[test]
    fn footer_appears_when_more_than_limit_files() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(LOCAL_CANDIDATE_LIMIT + 3) {
            std::fs::write(dir.path().join(format!("file-{i:02}.md")), b"x").unwrap();
        }
        let r = run(SacsState::S3Ready, &args(&[""], dir.path()), None);
        let footers: Vec<_> = r.iter().filter(|c| matches!(c.kind, Kind::Footer)).collect();
        assert_eq!(footers.len(), 1);
        assert!(footers[0].description.contains("more"));
    }

    // ------------------------------------------------------------------
    // Chunk E: remote-aware completions. Use a `FakeSource` keyed on
    // `(prefix, kind_filter)` so the snapshots never depend on real
    // network state or even the running agent.
    // ------------------------------------------------------------------

    #[derive(Default)]
    struct FakeSource {
        entries: std::collections::HashMap<
            (Option<String>, EntryKindFilter),
            Vec<RemoteListEntry>,
        >,
        error_for: Option<RemoteListError>,
    }

    impl FakeSource {
        fn put(
            &mut self,
            prefix: Option<&str>,
            kind: EntryKindFilter,
            entries: Vec<RemoteListEntry>,
        ) {
            self.entries
                .insert((prefix.map(String::from), kind), entries);
        }
    }

    impl RemoteListSource for FakeSource {
        fn list(
            &mut self,
            prefix: Option<&str>,
            kind_filter: EntryKindFilter,
        ) -> Result<Vec<RemoteListEntry>, RemoteListError> {
            if let Some(e) = &self.error_for {
                return Err(e.clone());
            }
            // First try the exact-match key; fall back to the
            // `Both` bucket because the FakeSource user often
            // populates with `Both` and lets the production
            // filter step do the kind narrowing.
            if let Some(v) = self.entries.get(&(prefix.map(String::from), kind_filter)) {
                return Ok(v.clone());
            }
            Ok(self
                .entries
                .get(&(prefix.map(String::from), EntryKindFilter::Both))
                .cloned()
                .unwrap_or_default())
        }
    }

    fn rfile(name: &str) -> RemoteListEntry {
        RemoteListEntry {
            name: name.into(),
            size: Some(123),
            kind: RemoteKind::File,
            mtime_secs: None,
        }
    }

    fn rdir(name: &str) -> RemoteListEntry {
        RemoteListEntry {
            name: name.into(),
            size: None,
            kind: RemoteKind::Directory,
            mtime_secs: None,
        }
    }

    /// M5: `zz d <TAB>` in S3 lists remote files (raw bytes
    /// description), no local files surface.
    #[test]
    fn m5_d_first_arg_lists_remote_files() {
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Both,
            vec![rfile("readme.md"), rfile("notes.txt"), rdir("docs")],
        );
        let r = run(SacsState::S3Ready, &args(&["d", ""], cur.path()), Some(&mut src));
        let kinds: Vec<&Kind> = r.iter().map(|c| &c.kind).collect();
        // FileRemote present, FileLocal NOT — `d` queries the
        // remote, never the local fs.
        assert!(kinds.iter().any(|k| matches!(k, Kind::FileRemote)));
        assert!(!kinds.iter().any(|k| matches!(k, Kind::FileLocal)));
        let descs: Vec<&str> = r.iter().map(|c| c.description.as_str()).collect();
        assert!(
            descs.iter().any(|d| d.contains("download (raw bytes) · remote")),
            "expected download description, got {descs:?}"
        );
    }

    #[test]
    fn m5_d_first_arg_also_surfaces_remote_dirs_for_navigation() {
        // `zz d <TAB>` must list remote dirs alongside files so
        // `zz d Documents/<TAB>` can descend. Dir values must
        // end with `/`.
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Both,
            vec![rfile("readme.md"), rdir("Documents"), rdir("backup")],
        );
        let r = run(SacsState::S3Ready, &args(&["d", ""], cur.path()), Some(&mut src));
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&Kind::FileRemote));
        assert!(kinds.contains(&Kind::DirRemote));
        let dir_values: Vec<&str> = r
            .iter()
            .filter(|c| matches!(c.kind, Kind::DirRemote))
            .map(|c| c.value.as_str())
            .collect();
        for v in &dir_values {
            assert!(
                v.ends_with('/'),
                "DirRemote value {v:?} must end with '/' for nested completion"
            );
        }
    }

    /// M5 negative: agent locked → empty remote list, dropdown
    /// silently empty (no FileRemote, no error surface).
    #[test]
    fn m5_d_first_arg_when_agent_locked_returns_empty() {
        let cur = make_fixture();
        let mut src = FakeSource {
            error_for: Some(RemoteListError::NotUnlocked),
            ..Default::default()
        };
        let r = run(SacsState::S3Ready, &args(&["d", ""], cur.path()), Some(&mut src));
        assert!(!r.iter().any(|c| matches!(c.kind, Kind::FileRemote)));
    }

    /// M9: `zz s file1.md <TAB>` — multi-source upload → mix of
    /// more local files (rank 1..) and `dir_remote` "close as
    /// destination" (rank 50+).
    #[test]
    fn m9_s_with_first_source_offers_more_sources_and_dir_remote() {
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Directory,
            vec![rdir("docs"), rdir("backup"), rdir("snapshots")],
        );
        let r = run(
            SacsState::S3Ready,
            &args(&["s", "file1.md", ""], cur.path()),
            Some(&mut src),
        );
        assert!(r.iter().any(|c| matches!(c.kind, Kind::FileLocal)));
        assert!(r.iter().any(|c| matches!(c.kind, Kind::DirRemote)));
        // dir_remote candidates have empty descriptions in v1
        // (so zsh packs them into a multi-column grid below the
        // file rows). The "[remote dir]" section header is what
        // conveys the kind to the operator.
        let dir_descs: Vec<&str> = r
            .iter()
            .filter(|c| matches!(c.kind, Kind::DirRemote))
            .map(|c| c.description.as_str())
            .collect();
        assert!(
            dir_descs.iter().all(|d| d.is_empty()),
            "DirRemote descriptions must be empty for grid layout, got {dir_descs:?}"
        );
    }

    /// M10: `zz sa src <TAB>` — local dir already chosen, second
    /// positional is a remote-prefix from the agent.
    #[test]
    fn m10_sa_with_local_dir_offers_remote_prefix() {
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Directory,
            vec![rdir("backup"), rdir("snapshots"), rdir("project")],
        );
        let r = run(
            SacsState::S3Ready,
            &args(&["sa", "src", ""], cur.path()),
            Some(&mut src),
        );
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        // Only DirRemote (+ optional Footer); no local entries
        // bleed through at this position.
        assert!(
            kinds.iter().all(|k| matches!(k, Kind::DirRemote | Kind::Footer)),
            "expected only DirRemote/Footer, got {kinds:?}"
        );
        // Remote-dir descriptions are empty in v1 — the grouping
        // and the "[remote dir]" header are what convey context.
        let descs: Vec<&str> = r.iter().filter(|c| matches!(c.kind, Kind::DirRemote))
            .map(|c| c.description.as_str()).collect();
        assert!(
            descs.iter().all(|d| d.is_empty()),
            "DirRemote descriptions must be empty, got {descs:?}"
        );
    }

    /// M11: `zz da ./out <TAB>` — same shape as M10 but for
    /// download. Local-dest already chosen, second positional
    /// is a remote-prefix.
    #[test]
    fn m11_da_with_local_dest_offers_remote_prefix() {
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Directory,
            vec![rdir("docs"), rdir("backup"), rdir("project")],
        );
        let r = run(
            SacsState::S3Ready,
            &args(&["da", "./out", ""], cur.path()),
            Some(&mut src),
        );
        let kinds: std::collections::HashSet<_> = r.iter().map(|c| c.kind).collect();
        assert!(
            kinds.iter().all(|k| matches!(k, Kind::DirRemote | Kind::Footer)),
            "expected only DirRemote/Footer, got {kinds:?}"
        );
    }

    #[test]
    fn parent_for_remote_prefix_handles_paths() {
        // No slash → root prefix (None) so the agent returns the
        // top-level listing and the shell filters by the typed
        // segment.
        assert_eq!(parent_for_remote_prefix(""), None);
        assert_eq!(parent_for_remote_prefix("notes"), None);
        // Single slash mid-word → the part before the last `/`.
        assert_eq!(parent_for_remote_prefix("docs/api"), Some("docs".into()));
        // Trailing slash → drop it before splitting.
        assert_eq!(parent_for_remote_prefix("docs/"), None);
        assert_eq!(parent_for_remote_prefix("docs/api/"), Some("docs".into()));
    }

    #[test]
    fn try_list_remote_swallows_errors() {
        let mut src = FakeSource {
            error_for: Some(RemoteListError::Provider),
            ..Default::default()
        };
        let r = try_list_remote(Some(&mut src), None, EntryKindFilter::Both);
        assert!(r.is_empty());
    }

    /// Privacy lock: snapshots must not contain `zz-drop.net`,
    /// real-looking emails, or token-shaped strings. Built into
    /// `complete::tests` rather than a separate harness so a
    /// future contributor adding a description can't slip a
    /// host name past a forgotten file.
    #[test]
    fn no_secrets_or_remote_host_in_emitted_strings() {
        let cur = make_fixture();
        let mut src = FakeSource::default();
        src.put(
            None,
            EntryKindFilter::Both,
            vec![rfile("readme.md"), rdir("docs")],
        );
        let r = run(
            SacsState::S3Ready,
            &args(&["d", ""], cur.path()),
            Some(&mut src),
        );
        for c in &r {
            for s in [&c.value, &c.display, &c.description] {
                assert!(!s.contains("zz-drop.net"), "leak: {s:?}");
                assert!(!s.contains("@example.org"), "leak: {s:?}");
                assert!(!s.contains("@example.com"), "leak: {s:?}");
                assert!(!s.contains("passphrase"), "leak: {s:?}");
            }
        }
    }
}
