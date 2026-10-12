//! Model profiles: the authored, per-family layer of the context assembler
//! (VISION §4.4, §6.1).
//!
//! A family is declared in the registry, never guessed from a model id: a
//! provider that serves anything (a local server, a router) declares none and
//! its models get no profile rather than a wrong one.
//!
//! These profiles are explicit opt-ins. Native harnesses own their default
//! instructions; Aether adds no model-family guidance unless configured.

use serde::{Deserialize, Serialize};

/// A lineage of models that answers to the same prompt wording, tool
/// presentation and reasoning hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelFamily {
    Claude,
    Gpt,
    Codex,
}

impl ModelFamily {
    /// Every family, so a suite can iterate them. Adding a variant without
    /// adding it here fails `every_family_is_listed`.
    pub const ALL: &'static [Self] = &[Self::Claude, Self::Gpt, Self::Codex];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gpt => "gpt",
            Self::Codex => "codex",
        }
    }

    pub fn profile(self) -> &'static ModelProfile {
        match self {
            Self::Claude => &CLAUDE,
            Self::Gpt => &GPT,
            Self::Codex => &CODEX,
        }
    }

    /// Authored model guidance is opt-in; native harnesses own their prompts.
    pub fn on_by_default(self) -> bool {
        false
    }
}

/// Which families' profiles are sent, per family. A family the operator does
/// not name keeps [`ModelFamily::on_by_default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct FamilyProfiles {
    pub claude: Option<bool>,
    pub gpt: Option<bool>,
    pub codex: Option<bool>,
}

impl FamilyProfiles {
    /// The operator's setting for one family, if they gave one. Exhaustive on
    /// purpose: a new family cannot compile until it is addressable here.
    fn get(self, family: ModelFamily) -> Option<bool> {
        match family {
            ModelFamily::Claude => self.claude,
            ModelFamily::Gpt => self.gpt,
            ModelFamily::Codex => self.codex,
        }
    }
}

/// `[model] profiles`: `true` or `false` for every family at once, or a table
/// naming families individually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ProfileSetting {
    /// One answer for every family, whatever the eval says.
    Every(bool),
    /// Per family; unnamed families keep their recorded default.
    PerFamily(FamilyProfiles),
}

impl Default for ProfileSetting {
    fn default() -> Self {
        Self::PerFamily(FamilyProfiles::default())
    }
}

impl ProfileSetting {
    /// Whether this family's profile goes into the turn.
    pub fn enabled(self, family: ModelFamily) -> bool {
        match self {
            Self::Every(on) => on,
            Self::PerFamily(named) => named.get(family).unwrap_or(family.on_by_default()),
        }
    }
}

/// One family's authored profile, in two parts so the engine path can take
/// the half that applies to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelProfile {
    /// Wording and reasoning hints. Always sent.
    pub guidance: &'static str,
    /// How to present and call tools. Dropped on the engine path: a CLI
    /// engine owns its own tools and how it shows them, so this half would be
    /// advice about something Aether does not control.
    pub tools: &'static str,
}

impl ModelProfile {
    /// The layer text for one turn. `engine` drops the tool half.
    pub fn text(&self, engine: bool) -> String {
        if engine {
            return self.guidance.to_owned();
        }
        format!("{}\n{}", self.guidance, self.tools)
    }
}

const CLAUDE: ModelProfile = ModelProfile {
    guidance: "\
Model profile: Claude.
- Decide, then act. Put the reasoning that changes the outcome in the reply; leave out the rest.
- Read before you edit, and change the least that does the job.
- Report what you actually ran and what it said. Never claim a check you did not run.",
    tools: "\
- Call tools with arguments that validate against the schema; when a value is unknown, look it up rather than inventing one.
- Independent calls can go in one message; a call that depends on an earlier result waits for it.
- Use paths and identifiers exactly as a tool returned them.",
};

const GPT: ModelProfile = ModelProfile {
    guidance: "\
Model profile: GPT.
- Reasoning effort is set by the harness. Do not spend the reply restating the plan.
- Read before you edit, and change the least that does the job.
- Report what you actually ran and what it said. Never claim a check you did not run.",
    tools: "\
- Emit tool calls as tool calls, never as text describing a call.
- One call per message when calls depend on each other; independent calls can share a message.
- Use paths and identifiers exactly as a tool returned them.",
};

const CODEX: ModelProfile = ModelProfile {
    guidance: "\
Model profile: Codex.
- You are working in a repository. Read the surrounding code before changing it, and keep the patch minimal.
- Run the project's own checks after a change and report their output.
- Do not reformat or reorganize files you did not need to change.",
    tools: "\
- Emit tool calls as tool calls, never as text describing a call.
- One call per message when calls depend on each other; independent calls can share a message.
- Use paths and identifiers exactly as a tool returned them.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_is_listed_and_has_a_profile() {
        // A family without a profile would silently send an empty layer.
        assert_eq!(ModelFamily::ALL.len(), 3);
        for family in ModelFamily::ALL {
            let profile = family.profile();
            assert!(!profile.guidance.trim().is_empty(), "{family:?}");
            assert!(!profile.tools.trim().is_empty(), "{family:?}");
            assert!(
                profile.guidance.starts_with("Model profile: "),
                "{family:?} must name itself so a snapshot says which profile it got"
            );
        }
    }

    #[test]
    fn the_engine_variant_drops_the_tool_half() {
        let profile = ModelFamily::Claude.profile();
        let api = profile.text(false);
        let engine = profile.text(true);
        assert!(api.contains("validate against the schema"));
        assert!(
            !engine.contains("validate against the schema"),
            "an engine presents its own tools; advising it about ours is noise"
        );
        assert!(engine.contains("Model profile: Claude."));
    }

    #[test]
    fn authored_profiles_are_opt_in_for_every_family() {
        for family in ModelFamily::ALL {
            assert!(!family.on_by_default(), "{family:?}");
        }
    }

    #[test]
    fn claude_is_off_by_default() {
        assert!(
            !ModelFamily::Claude.on_by_default(),
            "the recorded Claude eval hurt on rename, 3/8 against 6/8"
        );
    }

    #[test]
    fn gpt_is_off_by_default() {
        assert!(
            !ModelFamily::Gpt.on_by_default(),
            "GPT was measured against a structural mock only, never a real model"
        );
    }

    #[test]
    fn an_unset_family_keeps_its_recorded_default() {
        let setting = ProfileSetting::default();
        for family in ModelFamily::ALL {
            assert_eq!(
                setting.enabled(*family),
                family.on_by_default(),
                "{family:?}"
            );
        }
    }

    /// The operator's word beats the eval, in both directions, and says
    /// nothing about the families they did not name.
    #[test]
    fn a_named_family_overrides_its_default() {
        let setting = ProfileSetting::PerFamily(FamilyProfiles {
            claude: Some(true),
            codex: Some(false),
            gpt: None,
        });
        assert!(setting.enabled(ModelFamily::Claude));
        assert!(!setting.enabled(ModelFamily::Codex));
        assert_eq!(
            setting.enabled(ModelFamily::Gpt),
            ModelFamily::Gpt.on_by_default()
        );
    }

    #[test]
    fn a_bare_bool_answers_for_every_family() {
        for family in ModelFamily::ALL {
            assert!(ProfileSetting::Every(true).enabled(*family), "{family:?}");
            assert!(!ProfileSetting::Every(false).enabled(*family), "{family:?}");
        }
    }

    #[test]
    fn family_names_round_trip() {
        for family in ModelFamily::ALL {
            let json = serde_json::to_value(family).expect("ser");
            assert_eq!(json, serde_json::json!(family.as_str()));
            let back: ModelFamily = serde_json::from_value(json).expect("de");
            assert_eq!(back, *family);
        }
    }
}
