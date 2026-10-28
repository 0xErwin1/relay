//! Variable scopes and their precedence.

/// One variable in a [`Scope`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeEntry {
    /// Variable name, matched exactly against the trimmed reference name.
    pub key: String,
    /// Value substituted for the reference.
    pub value: String,
    /// A disabled entry never resolves and never shadows a lower scope.
    pub enabled: bool,
}

impl ScopeEntry {
    /// Creates an entry.
    pub fn new(key: impl Into<String>, value: impl Into<String>, enabled: bool) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled,
        }
    }
}

/// An ordered list of variables.
///
/// When several enabled entries share a key, the last one wins, so appending an entry
/// overrides earlier ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    /// Entries in their stored order.
    pub entries: Vec<ScopeEntry>,
}

impl Scope {
    /// Returns the value of the last enabled entry named `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.enabled && entry.key == key)
            .map(|entry| entry.value.as_str())
    }
}

impl FromIterator<ScopeEntry> for Scope {
    fn from_iter<I: IntoIterator<Item = ScopeEntry>>(iter: I) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

/// The five variable scopes, lowest to highest precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Workspace globals.
    Globals,
    /// Collection variables.
    Collection,
    /// Active environment.
    Environment,
    /// Iteration data, set by the runner only.
    IterationData,
    /// Local variables, set by scripts for one execution.
    Local,
}

impl ScopeKind {
    /// Every kind, lowest precedence first.
    pub const ALL: [Self; 5] = [
        Self::Globals,
        Self::Collection,
        Self::Environment,
        Self::IterationData,
        Self::Local,
    ];

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Globals => "globals",
            Self::Collection => "collection",
            Self::Environment => "environment",
            Self::IterationData => "iteration data",
            Self::Local => "local",
        }
    }
}

/// All scopes a reference can resolve against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scopes {
    globals: Scope,
    collection: Scope,
    environment: Scope,
    iteration_data: Scope,
    local: Scope,
}

impl Scopes {
    /// Returns the scope of `kind`.
    pub fn scope(&self, kind: ScopeKind) -> &Scope {
        match kind {
            ScopeKind::Globals => &self.globals,
            ScopeKind::Collection => &self.collection,
            ScopeKind::Environment => &self.environment,
            ScopeKind::IterationData => &self.iteration_data,
            ScopeKind::Local => &self.local,
        }
    }

    /// Returns the scope of `kind` for editing.
    pub fn scope_mut(&mut self, kind: ScopeKind) -> &mut Scope {
        match kind {
            ScopeKind::Globals => &mut self.globals,
            ScopeKind::Collection => &mut self.collection,
            ScopeKind::Environment => &mut self.environment,
            ScopeKind::IterationData => &mut self.iteration_data,
            ScopeKind::Local => &mut self.local,
        }
    }

    /// Returns the value of `name` from the highest-precedence scope that defines it as an
    /// enabled entry.
    pub fn lookup(&self, name: &str) -> Option<&str> {
        ScopeKind::ALL
            .iter()
            .rev()
            .find_map(|kind| self.scope(*kind).get(name))
    }
}
