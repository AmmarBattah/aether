use super::*;
use aether_core::model::ModelRef;

/// VISION §3.1: the only argument these take is `subtree`, and a typo is
/// refused rather than quietly pausing one agent when a subtree was meant.
#[test]
fn operator_commands_require_content_and_explicit_broader_scopes() {
    for text in [
        "/rename",
        "/remember",
        "/remember --global",
        "/remember --repository",
        "/remember --globla fact",
    ] {
        assert!(
            matches!(parse(text), InputAction::Command(SlashCommand::Invalid(_))),
            "{text}"
        );
    }
    assert_eq!(
        parse("/rename 名前 café"),
        InputAction::Command(SlashCommand::Rename("名前 café".into()))
    );
    for (prefix, scope) in [
        ("", RememberScope::Agent),
        ("--agent ", RememberScope::Agent),
        ("--global ", RememberScope::Global),
        ("--repository ", RememberScope::Repository),
    ] {
        assert_eq!(
            parse(&format!("/remember {prefix}Keep Unicode 名前 intact.")),
            InputAction::Command(SlashCommand::OperatorRemember {
                text: "Keep Unicode 名前 intact.".into(),
                scope
            })
        );
    }
    assert_eq!(
        parse("/remember -- --global is literal"),
        InputAction::Command(SlashCommand::OperatorRemember {
            text: "--global is literal".into(),
            scope: RememberScope::Agent
        })
    );
    assert_eq!(
        parse("remember this message"),
        InputAction::Message("remember this message".into())
    );
}

#[test]
fn pause_and_resume_take_only_a_subtree_argument() {
    assert_eq!(
        parse("/pause"),
        InputAction::Command(SlashCommand::Pause { subtree: false })
    );
    assert_eq!(
        parse("/pause subtree"),
        InputAction::Command(SlashCommand::Pause { subtree: true })
    );
    assert_eq!(
        parse("/resume --subtree"),
        InputAction::Command(SlashCommand::Resume { subtree: true })
    );
    assert_eq!(
        parse("/resume"),
        InputAction::Command(SlashCommand::Resume { subtree: false })
    );
    assert_eq!(
        parse("/pause subtre"),
        InputAction::Command(SlashCommand::Invalid("usage: /pause [subtree]".into()))
    );
}

#[test]
fn parses_messages_and_commands() {
    assert_eq!(parse("  "), InputAction::Empty);
    assert_eq!(
        parse("hello there"),
        InputAction::Message("hello there".into())
    );
    assert_eq!(
        parse("/new fix the build"),
        InputAction::Command(SlashCommand::New {
            objective: Some("fix the build".into())
        })
    );
    assert_eq!(
        parse("/new"),
        InputAction::Command(SlashCommand::New { objective: None })
    );
    assert_eq!(
        parse("/model anthropic/claude-opus-5-5"),
        InputAction::Command(SlashCommand::Model {
            chain: ModelChain::single(ModelRef::new("anthropic", "claude-opus-5-5")),
            default_only: false
        })
    );
    assert_eq!(
        parse("/model anthropic/opus,claude-code/haiku"),
        InputAction::Command(SlashCommand::Model {
            chain: ModelChain::new(
                ModelRef::new("anthropic", "opus"),
                vec![ModelRef::new("claude-code", "haiku")]
            ),
            default_only: false
        })
    );
    assert_eq!(parse("/model"), InputAction::Command(SlashCommand::Models));
    // --default sets the session default instead of switching the agent.
    assert_eq!(
        parse("/model --default mock/m"),
        InputAction::Command(SlashCommand::Model {
            chain: ModelChain::single(ModelRef::new("mock", "m")),
            default_only: true
        })
    );
    assert!(matches!(
        parse("/model --default"),
        InputAction::Command(SlashCommand::Invalid(_))
    ));
    // "/model --defaultmock/m" used to
    // parse as "--default mock/m", silently moving the session default
    // instead of switching the agent to a model named "--defaultmock".
    // Wrong-looking beats silently wrong: the whole thing must parse as
    // a (garbage) model switch, not the flag.
    assert_eq!(
        parse("/model --defaultmock/m"),
        InputAction::Command(SlashCommand::Model {
            chain: ModelChain::single(ModelRef::new("--defaultmock", "m")),
            default_only: false
        })
    );
    assert_eq!(
        parse("/effort"),
        InputAction::Command(SlashCommand::Efforts)
    );
    assert_eq!(
        palette_matches("/m")
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>(),
        vec!["model", "memory", "mcp", "mode"],
        "rank order; hidden aliases (models, motion, mouse) are not listed"
    );
    assert!(palette_matches("/model x").is_empty());
    assert!(palette_matches("hello").is_empty());
    assert_eq!(palette_matches("/").len(), COMMANDS.len());
    assert!(matches!(
        parse("/model nope"),
        InputAction::Command(SlashCommand::Invalid(_))
    ));
    assert_eq!(
        parse("/effort xhigh"),
        InputAction::Command(SlashCommand::Effort(Effort::XHigh))
    );
    assert_eq!(parse("/rules"), InputAction::Command(SlashCommand::Rules));
    assert_eq!(
        parse("/approvals"),
        InputAction::Command(SlashCommand::Rules)
    );
    assert!(matches!(
        parse("/effort huge"),
        InputAction::Command(SlashCommand::Invalid(_))
    ));
    assert_eq!(parse("/Help"), InputAction::Command(SlashCommand::Help));
    for (arg, name) in [
        ("auto", "auto"),
        ("dark", "dark"),
        ("dusk", "dark"),
        ("Light", "light"),
        ("dawn", "light"),
        ("mono", "mono"),
        ("nord", "nord"),
        ("gruvbox-light", "gruvbox-light"),
        ("neon", "neon"),
    ] {
        let InputAction::Command(SlashCommand::SetTheme(choice)) = parse(&format!("/theme {arg}"))
        else {
            panic!("/theme {arg} is not a theme command");
        };
        assert_eq!(choice.to_string(), name, "{arg}");
    }
    assert_eq!(parse("/theme"), InputAction::Command(SlashCommand::Theme));
    assert!(matches!(
        parse("/theme ../neon"),
        InputAction::Command(SlashCommand::Invalid(_))
    ));
    assert_eq!(
        parse("/motion reduced"),
        InputAction::Command(SlashCommand::Motion(Some(true)))
    );
    assert_eq!(
        parse("/motion full"),
        InputAction::Command(SlashCommand::Motion(Some(false)))
    );
    assert_eq!(
        parse("/motion"),
        InputAction::Command(SlashCommand::Motion(None))
    );
    for (arg, mode) in [
        ("", None),
        (" off", Some(MouseMode::Off)),
        (" scroll", Some(MouseMode::Scroll)),
        (" FULL", Some(MouseMode::Full)),
    ] {
        assert_eq!(
            parse(&format!("/mouse{arg}")),
            InputAction::Command(SlashCommand::Mouse(mode)),
            "{arg}"
        );
    }
    assert!(matches!(
        parse("/mouse wheel"),
        InputAction::Command(SlashCommand::Invalid(_))
    ));
    assert_eq!(
        parse("/agent"),
        InputAction::Command(SlashCommand::Agent { name: None })
    );
    assert_eq!(
        parse("/agents reviewer"),
        InputAction::Command(SlashCommand::Agent {
            name: Some("reviewer".into())
        })
    );
    assert_eq!(
        parse("/settings"),
        InputAction::Command(SlashCommand::Settings)
    );
    assert_eq!(
        parse("/artifacts"),
        InputAction::Command(SlashCommand::Artifacts)
    );
    assert_eq!(
        parse("/frobnicate"),
        InputAction::Command(SlashCommand::Unknown("frobnicate".into()))
    );
}

/// §6.2: every REMOVE candidate is gone, not hidden.
#[test]
fn removed_commands_are_unknown() {
    for name in [
        "approve",
        "yes",
        "y",
        "deny",
        "no",
        "n",
        "always",
        "never",
        "find",
        "show",
        "timeline",
        "tree",
        "search",
        "replay",
        "github-watcher",
        "budget",
        "priority",
        "concurrency",
        "trigger",
        "subtree",
    ] {
        assert_eq!(
            parse(&format!("/{name}")),
            InputAction::Command(SlashCommand::Unknown(name.into())),
            "/{name}"
        );
    }
}
