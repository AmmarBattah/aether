//! `/workspace` and Ctrl+O: where agents work. Pinned workspaces first, then
//! ones with live agents, recents, the launch directory and discovered git
//! projects; a typed path lists its directories instead.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use super::picker::{Picker, PickerEvent, PickerRow};
use super::{Hint, frame_height, title, unhandled};
use crate::app::{App, LiveWorkspace};
use crate::commands::SlashCommand;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;
use crate::workspace;

const TITLE: &str = "Workspace";
const HINTS: &[Hint] = &[
    ("↑↓", "choose"),
    ("type", "to filter or a path"),
    ("tab", "complete"),
    ("ctrl+p", "pin"),
    ("enter", "open"),
    ("esc", "back"),
];

/// Everything the list is made of, gathered once when the picker opens.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// In pin order, as saved; directories that are gone are skipped.
    pub pinned: Vec<PathBuf>,
    pub live: Vec<LiveWorkspace>,
    /// Newest first, as saved; directories that are gone are skipped.
    pub recent: Vec<PathBuf>,
    pub launch: Option<PathBuf>,
    pub discovered: Vec<PathBuf>,
    /// The active workspace, marked `current`.
    pub current: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Live {
        agents: usize,
        running: usize,
    },
    Recent,
    Launch,
    Found,
    /// Pinned, and from no other source.
    Pinned,
}

/// The rows, one per directory: pinned, live, recent, launch, discovered, in
/// that order. `branch` reads a checkout's branch; injected so tests stay pure.
pub fn rows(
    sources: &Sources,
    dot: &str,
    branch: &dyn Fn(&Path) -> Option<String>,
) -> Vec<(PickerRow, PathBuf)> {
    arrange(&unpinned_rows(sources, dot, branch), &pins(sources), dot)
}

/// The pinned paths, canonical and once each, in pin order.
fn pins(sources: &Sources) -> Vec<PathBuf> {
    let mut pins: Vec<PathBuf> = Vec::new();
    for pin in sources.pinned.iter().map(|path| workspace::canonical(path)) {
        if !pins.contains(&pin) {
            pins.push(pin);
        }
    }
    pins
}

/// Pinned rows first, in pin order and marked, then the rest as they come.
/// A pin with no row (its directory gone) shows nothing.
fn arrange(
    unpinned: &[(PickerRow, PathBuf)],
    pinned: &[PathBuf],
    dot: &str,
) -> Vec<(PickerRow, PathBuf)> {
    let mut out: Vec<(PickerRow, PathBuf)> = pinned
        .iter()
        .filter_map(|pin| unpinned.iter().find(|(_, path)| path == pin))
        .map(|(row, path)| {
            let mut row = row.clone();
            row.detail = format!("{} {dot} pinned", row.detail);
            (row, path.clone())
        })
        .collect();
    out.extend(
        unpinned
            .iter()
            .filter(|(_, path)| !pinned.contains(path))
            .cloned(),
    );
    out
}

/// Every directory's row, unmarked, in source order with pinned-only ones last.
fn unpinned_rows(
    sources: &Sources,
    dot: &str,
    branch: &dyn Fn(&Path) -> Option<String>,
) -> Vec<(PickerRow, PathBuf)> {
    let mut seen = HashSet::new();
    let mut entries: Vec<(PathBuf, Origin)> = Vec::new();
    let mut add = |path: PathBuf, origin: Origin| {
        if seen.insert(path.clone()) {
            entries.push((path, origin));
        }
    };
    for live in &sources.live {
        let origin = Origin::Live {
            agents: live.agents,
            running: live.running,
        };
        add(workspace::canonical(&live.path), origin);
    }
    for recent in sources.recent.iter().filter(|path| path.is_dir()) {
        add(workspace::canonical(recent), Origin::Recent);
    }
    if let Some(launch) = &sources.launch {
        add(workspace::canonical(launch), Origin::Launch);
    }
    for found in &sources.discovered {
        add(workspace::canonical(found), Origin::Found);
    }
    for pinned in sources.pinned.iter().filter(|path| path.is_dir()) {
        add(workspace::canonical(pinned), Origin::Pinned);
    }
    let home = sources.home.as_deref();
    let current = sources.current.as_deref().map(workspace::canonical);
    let paths: Vec<PathBuf> = entries.iter().map(|(path, _)| path.clone()).collect();
    let labels = workspace::shorthands(&paths, home);
    entries
        .into_iter()
        .map(|(path, origin)| {
            let mut detail = vec![workspace::display(&path, home)];
            if origin != Origin::Found {
                detail.extend(branch(&path));
            }
            match origin {
                Origin::Live { agents, running } => detail.push(live_status(agents, running)),
                Origin::Recent => detail.push("recent".to_owned()),
                Origin::Launch => detail.push("launched here".to_owned()),
                Origin::Found | Origin::Pinned => {}
            }
            if current.as_ref() == Some(&path) {
                detail.push("current".to_owned());
            }
            let label = labels
                .get(&path)
                .cloned()
                .unwrap_or_else(|| workspace::shorthand(&path, home));
            let row = PickerRow::new(label, detail.join(&format!(" {dot} ")));
            (row, path)
        })
        .collect()
}

/// `2 agents, 1 running`.
fn live_status(agents: usize, running: usize) -> String {
    let noun = if agents == 1 { "agent" } else { "agents" };
    if running == 0 {
        format!("{agents} {noun}")
    } else {
        format!("{agents} {noun}, {running} running")
    }
}

/// Rows for a typed path: the directory itself, then its completions; a
/// path that leads nowhere is one row that cannot be chosen.
pub fn path_rows(
    typed: &str,
    home: Option<&Path>,
    base: Option<&Path>,
    listing: &mut Option<workspace::Listing>,
) -> Vec<(PickerRow, PathBuf)> {
    let completion = workspace::complete(typed, home, base, listing);
    let paths: Vec<PathBuf> = completion
        .exact
        .iter()
        .chain(&completion.candidates)
        .cloned()
        .collect();
    if paths.is_empty() {
        let row = PickerRow::new(typed, "no such directory").disabled();
        return vec![(row, PathBuf::new())];
    }
    paths
        .into_iter()
        .map(|path| {
            let row = PickerRow::new(
                workspace::shorthand(&path, home),
                workspace::display(&path, home),
            );
            (row, path)
        })
        .collect()
}

#[derive(Debug)]
pub struct WorkspaceView {
    /// The rows without a typed path, before pins reorder them.
    unpinned: Vec<(PickerRow, PathBuf)>,
    /// Canonical, in pin order: what puts rows first.
    pinned: Vec<PathBuf>,
    dot: &'static str,
    picker: Picker<PathBuf>,
    /// What is typed: a filter, or a path once it starts like one.
    query: String,
    home: Option<PathBuf>,
    /// Where a relative path starts: the active workspace.
    base: Option<PathBuf>,
    /// The typed path's parent, read once while typing stays in it.
    listing: Option<workspace::Listing>,
}

impl WorkspaceView {
    /// Discovers projects now; a repository cloned later shows on the next open.
    pub fn new(app: &App) -> WorkspaceView {
        WorkspaceView::discover(app).0
    }

    /// [`WorkspaceView::new`], with the configured roots discovery could not search.
    pub fn discover(app: &App) -> (WorkspaceView, Vec<String>) {
        let home = app.options.home.clone();
        let found = workspace::Discovery::new(
            app.options.workspace_roots.as_deref(),
            app.options.workspace_depth,
            home.as_deref(),
        )
        .search();
        let sources = Sources {
            pinned: app
                .pinned_workspaces
                .iter()
                .filter_map(|pin| workspace::pinned_path(pin, home.as_deref()))
                .collect(),
            live: app.live_workspaces(),
            recent: app.recent_workspaces.iter().map(PathBuf::from).collect(),
            launch: app.options.cwd.clone(),
            discovered: found.projects,
            current: app.active_workspace().map(Path::to_path_buf),
            home,
        };
        let view = WorkspaceView::from_sources(
            &sources,
            app.theme.glyphs().dot,
            &crate::app::git::branch_of,
        );
        (view, found.missing)
    }

    /// The view over `sources`' [`rows`], the cursor on the current workspace.
    pub fn from_sources(
        sources: &Sources,
        dot: &'static str,
        branch: &dyn Fn(&Path) -> Option<String>,
    ) -> WorkspaceView {
        let unpinned = unpinned_rows(sources, dot, branch);
        let pinned = pins(sources);
        let listed = arrange(&unpinned, &pinned, dot);
        let current = sources.current.as_deref().map(workspace::canonical);
        let mut picker = Picker::new(TITLE, listed.clone());
        picker.cursor = listed
            .iter()
            .position(|(_, path)| Some(path) == current.as_ref())
            .unwrap_or(0);
        WorkspaceView {
            unpinned,
            pinned,
            dot,
            picker,
            query: String::new(),
            home: sources.home.clone(),
            base: sources.current.clone(),
            listing: None,
        }
    }

    /// Rebuild the list for what is typed now.
    fn refresh(&mut self) {
        if workspace::looks_like_path(&self.query) {
            let rows = path_rows(
                &self.query,
                self.home.as_deref(),
                self.base.as_deref(),
                &mut self.listing,
            );
            self.picker = Picker::new(TITLE, rows);
        } else {
            let listed = arrange(&self.unpinned, &self.pinned, self.dot);
            let mut picker = Picker::new(TITLE, listed);
            picker.filter.clone_from(&self.query);
            self.picker = picker;
        }
    }

    /// Ctrl+P: pin the highlighted directory, or unpin it, keeping the cursor on it.
    fn toggle_pin(&mut self) -> ViewOutcome {
        let Some(path) = self
            .picker
            .selected_row()
            .filter(|row| !row.disabled)
            .and(self.picker.selected())
            .map(|path| workspace::canonical(path))
        else {
            return ViewOutcome::Consumed;
        };
        if path.to_str().is_none() {
            let refused = workspace::NotUtf8(path).to_string();
            return ViewOutcome::Act(vec![ViewAction::Flash(refused)]);
        }
        let pinned = !self.pinned.contains(&path);
        if pinned {
            // A typed path's directory may be in no list yet.
            if !self.unpinned.iter().any(|(_, listed)| *listed == path) {
                let home = self.home.as_deref();
                let row = PickerRow::new(
                    workspace::shorthand(&path, home),
                    workspace::display(&path, home),
                );
                self.unpinned.push((row, path.clone()));
            }
            self.pinned.push(path.clone());
        } else {
            self.pinned.retain(|pin| *pin != path);
        }
        if !workspace::looks_like_path(&self.query) {
            self.refresh();
            let visible = self.picker.visible();
            self.picker.cursor = visible
                .iter()
                .position(|&i| self.picker.items[i].1 == path)
                .unwrap_or(0);
        }
        ViewOutcome::Act(vec![ViewAction::PinWorkspace { path, pinned }])
    }

    /// Tab: the highlighted directory into the query, ending in `/` so the
    /// next keystrokes go one level down. A path text cannot spell is refused.
    fn complete(&mut self) -> Result<(), workspace::NotUtf8> {
        let Some(row) = self.picker.selected_row() else {
            return Ok(());
        };
        if row.disabled {
            return Ok(());
        }
        let Some(path) = self.picker.selected() else {
            return Ok(());
        };
        self.query = workspace::completed_text(
            &self.query,
            path,
            self.home.as_deref(),
            self.base.as_deref(),
        )?;
        self.refresh();
        Ok(())
    }

    /// Digits choose by number only on a short, untouched list; otherwise they are typed.
    fn digit_chooses(&self, c: char) -> bool {
        c.is_ascii_digit() && self.query.is_empty() && self.picker.items.len() <= 9
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// The rows on screen now, in order: for tests and the frames gate.
    pub fn visible_rows(&self) -> Vec<(&PickerRow, &Path)> {
        self.picker
            .visible()
            .into_iter()
            .map(|i| {
                let (row, path) = &self.picker.items[i];
                (row, path.as_path())
            })
            .collect()
    }

    pub fn highlighted(&self) -> Option<&Path> {
        self.picker.selected().map(PathBuf::as_path)
    }

    fn heading(&self, theme: &Theme) -> Vec<Span<'static>> {
        let mut heading = title(TITLE, "where agents work", theme);
        if !self.query.is_empty() {
            let what = if workspace::looks_like_path(&self.query) {
                "path"
            } else {
                "filter"
            };
            heading.push(Span::styled(format!("  {what} "), theme.secondary()));
            heading.push(Span::styled(self.query.clone(), theme.accent()));
        }
        heading
    }
}

impl BottomView for WorkspaceView {
    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Workspace)
    }

    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn handle_chord(&mut self, key: KeyEvent, _app: &App) -> ViewOutcome {
        if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::CONTROL {
            return self.toggle_pin();
        }
        ViewOutcome::Ignored
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &App) -> ViewOutcome {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return unhandled(key);
        }
        match key.code {
            KeyCode::Tab => {
                return match self.complete() {
                    Ok(()) => ViewOutcome::Consumed,
                    Err(refused) => ViewOutcome::Act(vec![ViewAction::Flash(refused.to_string())]),
                };
            }
            KeyCode::Char(c) if !self.digit_chooses(c) => {
                self.query.push(c);
                self.refresh();
                return ViewOutcome::Consumed;
            }
            KeyCode::Backspace | KeyCode::Esc if !self.query.is_empty() => {
                if key.code == KeyCode::Esc {
                    self.query.clear();
                } else {
                    self.query.pop();
                }
                self.refresh();
                return ViewOutcome::Consumed;
            }
            _ => {}
        }
        match self.picker.handle_key(key) {
            PickerEvent::Chosen(index) => {
                let path = self.picker.items[index].1.clone();
                ViewOutcome::Close(vec![ViewAction::Command(SlashCommand::Workspace(Some(
                    path,
                )))])
            }
            PickerEvent::Moved | PickerEvent::Filtered => ViewOutcome::Consumed,
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
            PickerEvent::Ignored => unhandled(key),
        }
    }

    fn handle_paste(&mut self, text: &str) -> ViewOutcome {
        let line = text.lines().next().unwrap_or("").trim();
        self.query.push_str(line);
        self.refresh();
        ViewOutcome::Consumed
    }

    fn desired_height(&self, width: u16, _app: &App) -> u16 {
        let _ = width;
        frame_height(self.picker.body_len())
    }

    fn render(&self, frame: &mut Frame, area: Rect, _app: &App, theme: &Theme) {
        self.picker
            .render_titled(frame, area, self.heading(theme), theme, HINTS);
    }
}

#[cfg(test)]
mod tests;
