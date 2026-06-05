//! Handle to a Claude Code state root.
//!
//! `ClaudeHome` points at the directory that contains both
//! `.claude.json` (the project registry sidecar) and `.claude/` (the
//! state subdirectory). On a default install that's the user's `$HOME`;
//! tests substitute a tempdir.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, trace};

use crate::config::{self, ConfigFiles, UserWants};
use crate::project::Project;
use crate::session::encode_project_dir;

#[derive(Debug, Clone)]
pub struct ClaudeHome {
    path: PathBuf,
}

impl ClaudeHome {
    /// Build a `ClaudeHome` from the ambient environment. Today that's
    /// the value of `$HOME`; `CLAUDE_CONFIG_DIR` support can slot in
    /// here later without changing the call site.
    pub fn from_env() -> io::Result<Self> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_e| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        Ok(Self { path: home })
    }

    /// Build a `ClaudeHome` rooted at an explicit path. Used by tests
    /// and by any caller pointing at a non-default location.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// List every project Claude Code has recorded for this home. Reads
    /// `<root>/.claude.json` and returns one `Project` per `projects`
    /// key that:
    ///
    /// - still exists as a directory on disk (Claude Code does not
    ///   prune `.claude.json`, so dead cwds are routine), and
    /// - has a corresponding session directory under
    ///   `<root>/.claude/projects/<encoded>/` (no recorded sessions
    ///   means there is nothing useful to report on the project).
    pub fn projects(&self) -> io::Result<Vec<Project>> {
        let claude_json = self.path.join(".claude.json");
        let text = fs::read_to_string(&claude_json)?;
        debug!(path = %claude_json.display(), bytes = text.len(), "read_to_string: claude.json");
        let parsed: ClaudeJson = serde_json::from_str(&text).map_err(io::Error::other)?;
        let sessions_root = self.path.join(".claude").join("projects");
        let raw_count = parsed.projects.len();
        let kept: Vec<Project> = parsed
            .projects
            .into_keys()
            .filter(|path| {
                // A project path equal to the Claude home root is by
                // definition not a project — it's the directory the
                // Claude state lives under. Treating it as one would
                // walk the entire home tree.
                let ok = path != &self.path;
                if !ok {
                    trace!(path = %path.display(), "projects: drop (equals claude home)");
                }
                ok
            })
            .filter(|path| {
                let ok = path.is_dir();
                if !ok {
                    trace!(path = %path.display(), "projects: drop (cwd missing)");
                }
                ok
            })
            .filter(|path| {
                let encoded = encode_project_dir(path);
                let sessions_dir = sessions_root.join(&encoded);
                let ok = sessions_dir.is_dir();
                if !ok {
                    trace!(path = %path.display(), encoded = %encoded, "projects: drop (no sessions dir)");
                }
                ok
            })
            .map(|path| Project { path })
            .collect();
        debug!(
            raw = raw_count,
            kept = kept.len(),
            "projects: filter result"
        );
        Ok(kept)
    }

    /// Start a finder for user-scope config files under
    /// `<root>/.claude/`. Each phase (settings, root `CLAUDE.md`,
    /// skills, commands, agents, installed plugins) is enabled by
    /// default; toggle individual phases off to skip the I/O they
    /// would do. Call `.find()` to get the lazy iterator.
    pub fn config(&self) -> ClaudeHomeFinder {
        ClaudeHomeFinder {
            home: self.path.clone(),
            wants: UserWants {
                settings: true,
                memory: true,
                skills: true,
                commands: true,
                agents: true,
                plugins: true,
            },
        }
    }
}

/// Builder for a user-scope `ConfigFiles` walk. Returned by
/// [`ClaudeHome::config`]. All phases start enabled; chain
/// `.<phase>(false)` calls to disable individual phases, then call
/// `.find()` to consume the builder.
pub struct ClaudeHomeFinder {
    home: PathBuf,
    wants: UserWants,
}

impl ClaudeHomeFinder {
    pub fn settings(mut self, on: bool) -> Self {
        self.wants.settings = on;
        self
    }
    pub fn memory(mut self, on: bool) -> Self {
        self.wants.memory = on;
        self
    }
    /// Skills *and* their rule files. Both come out of one `read_dir`,
    /// so they share a toggle.
    pub fn skills(mut self, on: bool) -> Self {
        self.wants.skills = on;
        self
    }
    pub fn commands(mut self, on: bool) -> Self {
        self.wants.commands = on;
        self
    }
    pub fn agents(mut self, on: bool) -> Self {
        self.wants.agents = on;
        self
    }
    /// The installed-plugins index file *and* every per-plugin walk it
    /// drives. One toggle for both, since reading the index without
    /// walking it is the only thing the bundling forecloses.
    pub fn plugins(mut self, on: bool) -> Self {
        self.wants.plugins = on;
        self
    }

    pub fn find(self) -> ConfigFiles {
        config::user_files(self.home, self.wants)
    }
}

#[derive(Deserialize)]
struct ClaudeJson {
    #[serde(default)]
    projects: BTreeMap<PathBuf, serde::de::IgnoredAny>,
}
