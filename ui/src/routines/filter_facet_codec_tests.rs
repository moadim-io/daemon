use super::*;

// ── RoutineStatusFacet codecs ─────────────────────────────────────────────────

#[test]
fn status_facet_roundtrips_and_defaults_to_all() {
    for f in [
        RoutineStatusFacet::All,
        RoutineStatusFacet::Enabled,
        RoutineStatusFacet::Disabled,
        RoutineStatusFacet::Dormant,
        RoutineStatusFacet::DueSoon,
        RoutineStatusFacet::Snoozed,
        RoutineStatusFacet::HasFlags,
        RoutineStatusFacet::AgentUnregistered,
    ] {
        assert_eq!(RoutineStatusFacet::from_str(f.as_str()), f);
    }
    assert_eq!(
        RoutineStatusFacet::from_str("nonsense"),
        RoutineStatusFacet::All
    );
    assert_eq!(RoutineStatusFacet::default(), RoutineStatusFacet::All);
}

// ── AgentFacet codecs ─────────────────────────────────────────────────────────

#[test]
fn agent_facet_roundtrips_and_defaults_to_all() {
    let all = AgentFacet::All;
    let named = AgentFacet::Named("claude".into());
    assert_eq!(AgentFacet::from_value(&all.as_value()), all);
    assert_eq!(AgentFacet::from_value(&named.as_value()), named);
    assert_eq!(AgentFacet::default(), AgentFacet::All);
}

#[test]
fn agent_facet_decodes_a_plain_name_as_named() {
    assert_eq!(
        AgentFacet::from_value("codex"),
        AgentFacet::Named("codex".into())
    );
}

// ── RepositoryFacet codecs ─────────────────────────────────────────────────────

#[test]
fn repository_facet_roundtrips_and_defaults_to_all() {
    let all = RepositoryFacet::All;
    let named = RepositoryFacet::Named("github.com/org/repo".into());
    assert_eq!(RepositoryFacet::from_value(&all.as_value()), all);
    assert_eq!(RepositoryFacet::from_value(&named.as_value()), named);
    assert_eq!(RepositoryFacet::default(), RepositoryFacet::All);
}

#[test]
fn repository_facet_decodes_a_plain_url_as_named() {
    assert_eq!(
        RepositoryFacet::from_value("github.com/org/repo"),
        RepositoryFacet::Named("github.com/org/repo".into())
    );
}

// ── TagFacet codecs ────────────────────────────────────────────────────────────

#[test]
fn tag_facet_roundtrips_and_defaults_to_all() {
    let all = TagFacet::All;
    let named = TagFacet::Named("nightly".into());
    assert_eq!(TagFacet::from_value(&all.as_value()), all);
    assert_eq!(TagFacet::from_value(&named.as_value()), named);
    assert_eq!(TagFacet::default(), TagFacet::All);
}

#[test]
fn tag_facet_decodes_a_plain_value_as_named() {
    assert_eq!(
        TagFacet::from_value("nightly"),
        TagFacet::Named("nightly".into())
    );
}

// ── RoutineMachineFacet codecs ────────────────────────────────────────────────

#[test]
fn machine_facet_roundtrips_through_select_value() {
    let any = RoutineMachineFacet::Any;
    let unassigned = RoutineMachineFacet::Unassigned;
    let specific = RoutineMachineFacet::Machine("alpha".into());
    assert_eq!(RoutineMachineFacet::from_value(&any.as_value()), any);
    assert_eq!(
        RoutineMachineFacet::from_value(&unassigned.as_value()),
        unassigned
    );
    assert_eq!(
        RoutineMachineFacet::from_value(&specific.as_value()),
        specific
    );
    assert_eq!(RoutineMachineFacet::default(), RoutineMachineFacet::Any);
}

#[test]
fn machine_facet_decodes_a_plain_id_as_specific() {
    assert_eq!(
        RoutineMachineFacet::from_value("worker-1"),
        RoutineMachineFacet::Machine("worker-1".into())
    );
}
