use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

/// Completed-run history only; live and externally referenced state is protected.
/// The retention executor must recheck eligibility in its write transaction.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RetentionSection {
    pub completed_run_days: NonZeroU16,
}

impl Default for RetentionSection {
    fn default() -> Self {
        Self {
            completed_run_days: const { NonZeroU16::new(7).unwrap() },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::AetherConfig;

    #[test]
    fn history_retention_defaults_to_seven_days() {
        let config: AetherConfig = toml::from_str("").unwrap();
        assert_eq!(config.retention.completed_run_days.get(), 7);
    }

    #[test]
    fn history_retention_accepts_ten_days() {
        let config: AetherConfig = toml::from_str("[retention]\ncompleted_run_days = 10").unwrap();
        assert_eq!(config.retention.completed_run_days.get(), 10);
    }

    #[test]
    fn generated_template_documents_a_working_retention_override() {
        let template = super::super::CONFIG_TEMPLATE;
        let defaults: AetherConfig = toml::from_str(template).unwrap();
        assert_eq!(defaults.retention.completed_run_days.get(), 7);
        assert!(template.contains("# [retention]"));
        assert!(template.contains("# completed_run_days = 7"));
        let configured = template
            .replace("# [retention]", "[retention]")
            .replace("# completed_run_days = 7", "completed_run_days = 10");
        let config: AetherConfig = toml::from_str(&configured).unwrap();
        assert_eq!(config.retention.completed_run_days.get(), 10);
    }

    #[test]
    fn invalid_retention_is_not_silently_coerced() {
        for value in ["0", "-1", "1.5", "65536", "\"7\""] {
            assert!(
                toml::from_str::<AetherConfig>(&format!(
                    "[retention]\ncompleted_run_days = {value}"
                ))
                .is_err(),
                "{value}"
            );
        }
        assert!(toml::from_str::<AetherConfig>("[retention]\ncompleted_runs_days = 10").is_err());
    }
}
