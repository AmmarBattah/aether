//! The operator's choice of view against trees and replies racing it: a tree
//! may fill an empty choice, never replace one, and a stale tree cannot
//! remove an agent this client just met.
use super::*;

/// The agent the daemon makes for `start`, before any reply names it.
fn made(start: &(CreateAgent, Then)) -> Agent {
    let mut made = agent(AgentId::new(), "assistant");
    made.scope = start.0.scope.clone().unwrap_or_default();
    made
}

fn reply(app: &mut App, start: &(CreateAgent, Then), made: &Agent) -> Vec<Effect> {
    app.reduce(AppEvent::Reply {
        then: start.1.clone(),
        result: Ok(Reply::Agent(made.clone())),
    })
}

/// Daemon activity on `agent`; the reducer asks for a tree in reply.
fn activity(app: &mut App, agent: AgentId, seq: u64) -> Vec<Effect> {
    app.reduce(AppEvent::Frame(SubscriptionFrame::Event(Box::new(event(
        seq,
        agent,
        EventKind::AgentInterrupted { turn: 1 },
    )))))
}

fn asks_tree(effects: &[Effect]) -> bool {
    effects.iter().any(|e| {
        matches!(
            e,
            Effect::Request {
                then: Then::Tree,
                ..
            }
        )
    })
}

fn patches(effects: &[Effect], id: AgentId) -> bool {
    effects.iter().any(|e| {
        matches!(
            e,
            Effect::Request { then: Then::PendingRootModelPatched(to), .. } if *to == id
        )
    })
}

fn refuse_patch(app: &mut App, id: AgentId) -> Vec<Effect> {
    app.reduce(AppEvent::Reply {
        then: Then::PendingRootModelPatched(id),
        result: Err(ClientError::Protocol(aether_protocol::ErrorReply::new(
            aether_protocol::ErrorCode::InvalidArgument,
            "model unavailable",
        ))),
    })
}

fn flash(app: &App) -> String {
    app.live_flash().unwrap_or_default().to_owned()
}

/// R1: a tree requested before B's create reply cannot know B, so leaving it
/// out neither drops the view of B nor moves messages to A's workspace. A
/// fresh tree confirms B; once confirmed, a tree without B does remove it.
#[test]
fn a_tree_requested_before_a_creation_cannot_drop_its_root() {
    let (_tmp, [_, b, _], live, mut app) = live_in_first();
    let for_b = one_start(&switch(&mut app, &b));
    assert!(asks_tree(&activity(&mut app, live.id, 2)), "outstanding");
    let b_root = made(&for_b);
    let effects = reply(&mut app, &for_b, &b_root);
    assert_eq!(app.selected, Some(b_root.id));
    assert!(
        !asks_tree(&effects),
        "coalesced behind the outstanding tree"
    );

    let effects = apply(&mut app, tree_of(vec![live.clone()]));
    assert_eq!(app.selected, Some(b_root.id), "a stale tree took the view");
    assert!(created(&effects).is_empty(), "{effects:?}");
    assert!(asks_tree(&effects), "the fresh tree follows");
    assert_eq!(
        sent(&submit(&mut app, "for b")),
        [(b_root.id, "for b".into())]
    );

    apply(&mut app, tree_of(vec![live.clone(), b_root.clone()]));
    assert_eq!(app.selected, Some(b_root.id));
    assert!(asks_tree(&activity(&mut app, live.id, 3)));
    apply(&mut app, tree_of(vec![live.clone()]));
    assert_ne!(
        app.selected,
        Some(b_root.id),
        "a removal must still clear it"
    );
}

/// R1, other order: a tree requested after B's create reply is authoritative,
/// so one that leaves B out does clear it.
#[test]
fn a_tree_requested_after_a_creation_can_drop_its_root() {
    let (_tmp, [_, b, _], live, mut app) = live_in_first();
    let for_b = one_start(&switch(&mut app, &b));
    let b_root = made(&for_b);
    let effects = reply(&mut app, &for_b, &b_root);
    assert_eq!(app.selected, Some(b_root.id));
    assert!(asks_tree(&effects), "requested after the reply");
    apply(&mut app, tree_of(vec![live.clone()]));
    assert_ne!(app.selected, Some(b_root.id));
}

/// R2: `/workspace B` before the first tree is the operator's choice, so the
/// first tree's `--agent` loses to it, in either order.
#[test]
fn cli_focus_loses_to_a_workspace_switch_made_before_or_after_the_first_tree() {
    for switch_first in [true, false] {
        let (_tmp, a, b) = dirs();
        let focused = root_in("focused", &a, 10);
        let mut app = launched_with(TuiOptions {
            cwd: Some(a.clone()),
            auto_create_root: true,
            focus: Some(focused.id),
            ..Default::default()
        });
        let for_b = if switch_first {
            let for_b = one_start(&switch(&mut app, &b));
            let effects = apply(&mut app, tree_of(vec![focused.clone()]));
            assert!(starts(&effects).is_empty(), "{effects:?}");
            for_b
        } else {
            apply(&mut app, tree_of(vec![focused.clone()]));
            assert_eq!(app.selected, Some(focused.id));
            one_start(&switch(&mut app, &b))
        };
        assert_eq!(app.selected, None, "switch_first={switch_first}");
        assert!(
            sent(&submit(&mut app, "task for b")).is_empty(),
            "switch_first={switch_first}"
        );
        let b_root = made(&for_b);
        let effects = reply(&mut app, &for_b, &b_root);
        assert_eq!(sent(&effects), [(b_root.id, "task for b".into())]);
        assert_eq!(app.selected, Some(b_root.id), "switch_first={switch_first}");
        apply(&mut app, tree_of(vec![focused.clone(), b_root.clone()]));
        assert_eq!(app.selected, Some(b_root.id), "switch_first={switch_first}");
    }
}

/// R3: while `/new` waits for its root, a tree showing the automatic first
/// root does not take the view, whether it lands before or after that root's
/// own reply; the follow-up reaches the new conversation.
#[test]
fn a_tree_with_another_root_here_does_not_cancel_a_pending_new() {
    for tree_first in [true, false] {
        let (_tmp, here, _) = dirs();
        let mut app = launched_in(&here);
        let first = one_start(&apply(&mut app, tree_of(vec![])));
        let second = one_start(&submit(&mut app, "/new second objective"));
        let (one, two) = (made(&first), made(&second));
        if tree_first {
            apply(&mut app, tree_of(vec![one.clone()]));
            assert!(sent(&reply(&mut app, &first, &one)).is_empty());
        } else {
            assert!(sent(&reply(&mut app, &first, &one)).is_empty());
            apply(&mut app, tree_of(vec![one.clone()]));
        }
        assert_eq!(app.selected, None, "tree_first={tree_first}");
        let effects = reply(&mut app, &second, &two);
        assert_eq!(sent(&effects), [(two.id, "second objective".into())]);
        assert_eq!(app.selected, Some(two.id), "tree_first={tree_first}");
        assert_eq!(
            sent(&submit(&mut app, "follow-up")),
            [(two.id, "follow-up".into())],
            "tree_first={tree_first}"
        );
    }
}

/// R4: a tree may show B's root before its create reply. Messages keep
/// queueing behind the start, picking B again keeps waiting, and once the
/// reply lands the chosen model goes first, then the messages in order.
#[test]
fn a_root_seen_before_its_reply_keeps_its_model_and_message_order() {
    for tree_first in [true, false] {
        let (_tmp, [_, b, _], live, mut app) = live_in_first();
        let for_b = one_start(&switch(&mut app, &b));
        submit(&mut app, "/model mock/changed");
        assert!(sent(&submit(&mut app, "objective")).is_empty());
        let b_root = made(&for_b);
        let mut effects = Vec::new();
        if tree_first {
            effects.extend(apply(&mut app, tree_of(vec![live.clone(), b_root.clone()])));
            assert_eq!(app.selected, None, "the tree took the view");
            assert!(starts(&switch(&mut app, &b)).is_empty());
            assert_eq!(app.selected, None, "picking B again took the view");
            effects.extend(submit(&mut app, "later"));
            effects.extend(reply(&mut app, &for_b, &b_root));
        } else {
            effects.extend(reply(&mut app, &for_b, &b_root));
            effects.extend(apply(&mut app, tree_of(vec![live.clone(), b_root.clone()])));
            effects.extend(submit(&mut app, "later"));
        }
        assert!(
            sent(&effects).is_empty(),
            "tree_first={tree_first}: {effects:?}"
        );
        assert!(patches(&effects, b_root.id), "tree_first={tree_first}");
        assert_eq!(app.selected, Some(b_root.id));
        assert_eq!(
            sent(&patched(&mut app, b_root.id)),
            [(b_root.id, "objective".into()), (b_root.id, "later".into())],
            "tree_first={tree_first}"
        );
    }
}

/// R5: Tab on a directory whose name is not UTF-8 is refused out loud,
/// before its lossy spelling becomes the query; Enter then opens nothing.
#[cfg(unix)]
#[test]
fn tab_refuses_a_non_utf8_directory_instead_of_completing_its_lookalike() {
    use std::os::unix::ffi::OsStringExt;

    use crate::ui::views::workspace::{Sources, WorkspaceView};

    let (tmp, a, _) = dirs();
    let parent = crate::workspace::canonical(tmp.path());
    let raw = parent.join(std::ffi::OsString::from_vec(b"proj-\xff".to_vec()));
    let lookalike = parent.join("proj-\u{FFFD}");
    std::fs::create_dir(&raw).expect("mkdir raw");
    std::fs::create_dir(&lookalike).expect("mkdir lookalike");
    let live = root_in("live", &a, 10);
    let mut app = launched_in(&a);
    apply(&mut app, tree_of(vec![live.clone()]));

    let sources = Sources {
        discovered: vec![raw.clone()],
        ..Default::default()
    };
    let view = WorkspaceView::from_sources(&sources, "|", &|_| None);
    app.bottom.push_view(Box::new(view));
    app.reduce(key(KeyCode::Tab));
    assert!(flash(&app).contains("not UTF-8"), "{:?}", flash(&app));
    assert_eq!(
        app.bottom.top_view().and_then(|view| view.kind()),
        Some(ViewKind::Workspace),
        "the picker stays open"
    );
    let effects = app.reduce(key(KeyCode::Enter));
    assert!(starts(&effects).is_empty(), "{effects:?}");
    assert_eq!(app.selected, Some(live.id));
    assert_eq!(app.active_workspace(), Some(a.as_path()));
}

/// R6: messages held for a new root's model change are dropped out loud when
/// that root ends, is archived, is restarted or is gone from a fresh tree,
/// and never reach its successor.
#[test]
fn messages_held_for_a_root_that_ended_are_reported_not_moved() {
    for ending in ["cancelled", "archived", "restarted", "gone"] {
        let (_tmp, [_, b, _], live, mut app) = live_in_first();
        let for_b = one_start(&switch(&mut app, &b));
        submit(&mut app, "/model mock/changed");
        submit(&mut app, "objective for b");
        let mut b_root = made(&for_b);
        reply(&mut app, &for_b, &b_root);
        refuse_patch(&mut app, b_root.id);
        apply(&mut app, tree_of(vec![live.clone(), b_root.clone()]));
        assert!(app.holds_root(b_root.id), "{ending}");

        let successor = root_in("successor", &b, 0);
        let mut agents = vec![live.clone()];
        match ending {
            "cancelled" => b_root.status = AgentStatus::Cancelled { reason: None },
            "archived" => b_root.archived = true,
            "restarted" => {
                b_root.status = AgentStatus::Cancelled { reason: None };
                b_root.restarted_as = Some(successor.id);
                agents.push(successor.clone());
            }
            _ => {}
        }
        if ending != "gone" {
            agents.insert(1, b_root.clone());
        }
        let effects = apply(&mut app, tree_of(agents));
        assert!(sent(&effects).is_empty(), "{ending}: {effects:?}");
        let notice = flash(&app);
        assert!(
            notice.contains("1 message for") && notice.contains("not sent"),
            "{ending}: {notice:?}"
        );
        assert!(!app.holds_root(b_root.id), "{ending}");

        if ending == "restarted" {
            // `/model` follows the restart and changes the successor like any agent.
            let effects = submit(&mut app, "/model mock/changed");
            assert!(effects.iter().any(|e| matches!(
                e,
                Effect::Request { then: Then::ModelPatched(id), .. } if *id == successor.id
            )));
            let mut changed = successor.clone();
            changed.model = ModelRef::new("mock", "changed");
            let effects = app.reduce(AppEvent::Reply {
                then: Then::ModelPatched(successor.id),
                result: Ok(Reply::Agent(changed)),
            });
            assert!(sent(&effects).is_empty(), "{effects:?}");
        }
    }
}

/// R6: a root the tree already shows ended when its create reply lands gets
/// nothing; what waited for it is reported instead.
#[test]
fn a_root_that_ended_before_its_reply_gets_none_of_its_messages() {
    let (_tmp, [_, b, _], live, mut app) = live_in_first();
    let for_b = one_start(&switch(&mut app, &b));
    submit(&mut app, "objective for b");
    let mut b_root = made(&for_b);
    b_root.status = AgentStatus::Cancelled { reason: None };
    apply(&mut app, tree_of(vec![live.clone(), b_root.clone()]));
    let effects = reply(&mut app, &for_b, &b_root);
    assert!(sent(&effects).is_empty(), "{effects:?}");
    let notice = flash(&app);
    assert!(
        notice.contains("1 message for") && notice.contains("not sent"),
        "{notice:?}"
    );
}

/// R6: a tree requested before the new root was met leaves it out without
/// dropping its held messages; they go out once its model change lands.
#[test]
fn a_stale_tree_keeps_messages_held_for_a_just_created_root() {
    let (_tmp, [_, b, _], live, mut app) = live_in_first();
    let for_b = one_start(&switch(&mut app, &b));
    submit(&mut app, "/model mock/changed");
    submit(&mut app, "objective for b");
    assert!(asks_tree(&activity(&mut app, live.id, 2)), "outstanding");
    let b_root = made(&for_b);
    reply(&mut app, &for_b, &b_root);
    apply(&mut app, tree_of(vec![live.clone()]));
    assert!(app.holds_root(b_root.id), "{:?}", flash(&app));
    assert!(!flash(&app).contains("not sent"), "{:?}", flash(&app));
    assert_eq!(
        sent(&patched(&mut app, b_root.id)),
        [(b_root.id, "objective for b".into())]
    );
}

/// R6 notice: a creation cut off by a disconnect may still have made its
/// agent, so its messages are "not sent", said once, and the outage notice
/// that follows does not replace it, even with nothing selected.
#[test]
fn a_disconnected_creation_says_its_messages_were_not_sent() {
    let (_tmp, [_, b, c], _, mut app) = live_in_first();
    let for_b = one_start(&switch(&mut app, &b));
    submit(&mut app, "one");
    submit(&mut app, "two");
    one_start(&switch(&mut app, &c));
    assert_eq!(app.selected, None);
    app.reduce(AppEvent::Reply {
        then: for_b.1.clone(),
        result: Err(ClientError::Disconnected),
    });
    app.reduce(AppEvent::Disconnected { because: None });
    let notice = flash(&app);
    assert!(
        notice.contains("2 messages for") && notice.contains("not sent"),
        "{notice:?}"
    );
    assert!(notice.contains("did not complete"), "{notice:?}");
    assert!(!notice.contains("never started"), "{notice:?}");
}

/// The ordinary single-workspace flow: a message and `/model` before the
/// first tree start one root here, the model first; `/new` then views its own.
#[test]
fn the_ordinary_single_workspace_flow_is_unchanged() {
    let (_tmp, here, _) = dirs();
    let mut app = launched_in(&here);
    let first = one_start(&submit(&mut app, "before root"));
    assert_eq!(workspace_of(&first.0), Some(here.as_path()));
    submit(&mut app, "/model mock/changed");
    assert!(starts(&apply(&mut app, tree_of(vec![]))).is_empty());
    assert!(sent(&submit(&mut app, "also before")).is_empty());
    let one = made(&first);
    let effects = reply(&mut app, &first, &one);
    assert!(sent(&effects).is_empty() && patches(&effects, one.id));
    assert_eq!(app.selected, Some(one.id));
    assert_eq!(
        sent(&patched(&mut app, one.id)),
        [
            (one.id, "before root".into()),
            (one.id, "also before".into())
        ]
    );
    apply(&mut app, tree_of(vec![one.clone()]));
    assert_eq!(app.selected, Some(one.id));

    let next = one_start(&submit(&mut app, "/new objective"));
    let two = made(&next);
    assert_eq!(
        sent(&reply(&mut app, &next, &two)),
        [(two.id, "objective".into())]
    );
    assert_eq!(app.selected, Some(two.id));
    apply(&mut app, tree_of(vec![one, two.clone()]));
    assert_eq!(app.selected, Some(two.id));
}
