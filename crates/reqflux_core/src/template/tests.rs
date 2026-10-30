use pretty_assertions::assert_eq;

use super::*;

fn scope(entries: &[(&str, &str, bool)]) -> Scope {
    entries
        .iter()
        .map(|(key, value, enabled)| ScopeEntry::new(*key, *value, *enabled))
        .collect()
}

fn resolve_fixed(text: &str, scopes: &Scopes) -> Resolved {
    resolve(text, scopes, &mut SequentialDynamicSource::default())
}

fn unresolved(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

// S11: precedence, lowest to highest: globals, collection, environment, iteration data, local.
#[test]
fn higher_precedence_scope_wins() {
    let kinds = [
        ScopeKind::Globals,
        ScopeKind::Collection,
        ScopeKind::Environment,
        ScopeKind::IterationData,
        ScopeKind::Local,
    ];
    let mut scopes = Scopes::default();
    for (index, kind) in kinds.iter().enumerate() {
        *scopes.scope_mut(*kind) = scope(&[("host", kind.name(), true)]);
        let resolved = resolve_fixed("{{host}}", &scopes);
        assert_eq!(resolved.text, kind.name(), "after adding scope {index}");
        assert_eq!(resolved.unresolved, Vec::<String>::new());
    }
}

#[test]
fn lookup_falls_through_to_lower_scopes() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("a", "global", true)]);
    *scopes.scope_mut(ScopeKind::Local) = scope(&[("b", "local", true)]);
    assert_eq!(scopes.lookup("a"), Some("global"));
    assert_eq!(scopes.lookup("b"), Some("local"));
    assert_eq!(scopes.lookup("c"), None);
}

#[test]
fn scope_kinds_are_listed_lowest_precedence_first() {
    assert_eq!(
        ScopeKind::ALL.map(ScopeKind::name),
        [
            "globals",
            "collection",
            "environment",
            "iteration data",
            "local"
        ]
    );
}

// S11: disabled variables do not resolve.
#[test]
fn disabled_entry_does_not_resolve_and_does_not_shadow() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("host", "global", true)]);
    *scopes.scope_mut(ScopeKind::Environment) = scope(&[("host", "env", false)]);
    assert_eq!(resolve_fixed("{{host}}", &scopes).text, "global");

    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("host", "global", false)]);
    let resolved = resolve_fixed("{{host}}", &scopes);
    assert_eq!(resolved.text, "{{host}}");
    assert_eq!(resolved.unresolved, unresolved(&["host"]));
}

#[test]
fn last_enabled_entry_in_a_scope_wins() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Environment) = scope(&[
        ("host", "first", true),
        ("host", "second", true),
        ("host", "third", false),
    ]);
    assert_eq!(resolve_fixed("{{host}}", &scopes).text, "second");
}

// S11: parsing rules.
#[test]
fn names_are_trimmed() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("name", "v", true)]);
    assert_eq!(
        resolve_fixed("[{{ name }}][{{name\t}}]", &scopes).text,
        "[v][v]"
    );
}

#[test]
fn text_without_valid_references_is_untouched() {
    let scopes = Scopes::default();
    for text in [
        "",
        "plain text",
        "{\"a\": {\"b\": 1}}",
        "single { brace } here",
        "open {{ never closed",
        "empty {{}} and {{   }}",
        "close only }}",
        "ünïcödé {{ ✓",
    ] {
        let resolved = resolve_fixed(text, &scopes);
        assert_eq!(resolved.text, text);
        assert_eq!(resolved.unresolved, Vec::<String>::new(), "{text}");
        assert_eq!(referenced_names(text), Vec::<String>::new(), "{text}");
    }
}

#[test]
fn braces_inside_a_reference_are_not_part_of_the_name() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("id", "7", true)]);
    assert_eq!(resolve_fixed("{{{id}}}", &scopes).text, "{7}");
    assert_eq!(resolve_fixed("{{a{{id}}", &scopes).text, "{{a7");
}

#[test]
fn substitutes_inside_json_and_urls() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Environment) =
        scope(&[("base", "https://api.test", true), ("id", "42", true)]);
    let resolved = resolve_fixed("{{base}}/users/{{id}}?q={\"id\": {{id}}}", &scopes);
    assert_eq!(resolved.text, "https://api.test/users/42?q={\"id\": 42}");
}

// S11: unresolved variables stay literal and are reported by name.
#[test]
fn unresolved_names_stay_literal_and_are_unique_in_order() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("known", "k", true)]);
    let resolved = resolve_fixed("{{ b }}-{{known}}-{{a}}-{{b}}-{{$nope}}", &scopes);
    assert_eq!(resolved.text, "{{ b }}-k-{{a}}-{{b}}-{{$nope}}");
    assert_eq!(resolved.unresolved, unresolved(&["b", "a", "$nope"]));
}

// S11: resolution is single pass.
#[test]
fn substituted_values_are_not_resolved_again() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) =
        scope(&[("outer", "{{inner}}", true), ("inner", "deep", true)]);
    let resolved = resolve_fixed("{{outer}}", &scopes);
    assert_eq!(resolved.text, "{{inner}}");
    assert_eq!(resolved.unresolved, Vec::<String>::new());
}

// S11: dynamic variables, a fresh value per occurrence.
#[test]
fn dynamic_variables_use_the_source_per_occurrence() {
    let scopes = Scopes::default();
    let resolved = resolve_fixed(
        "{{$guid}} {{$randomUUID}} {{$timestamp}} {{$isoTimestamp}} {{$randomInt}} {{$guid}}",
        &scopes,
    );
    assert_eq!(
        resolved.text,
        "00000000-0000-4000-8000-000000000001 00000000-0000-4000-8000-000000000002 \
         1700000003 2023-11-14T22:13:24Z 5 00000000-0000-4000-8000-000000000006"
    );
    assert_eq!(resolved.unresolved, Vec::<String>::new());
}

#[test]
fn dynamic_names_are_case_sensitive_and_trimmed() {
    let resolved = resolve_fixed("{{ $guid }} {{$GUID}}", &Scopes::default());
    assert_eq!(
        resolved.text,
        "00000000-0000-4000-8000-000000000001 {{$GUID}}"
    );
    assert_eq!(resolved.unresolved, unresolved(&["$GUID"]));
}

#[test]
fn scope_entry_shadows_dynamic_variable() {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("$guid", "fixed", true)]);
    let mut source = SequentialDynamicSource::default();
    assert_eq!(resolve("{{$guid}}", &scopes, &mut source).text, "fixed");
    // The source is not consulted when a scope wins.
    assert_eq!(
        resolve("{{$randomUUID}}", &scopes, &mut source).text,
        "00000000-0000-4000-8000-000000000001"
    );

    *scopes.scope_mut(ScopeKind::Globals) = scope(&[("$guid", "fixed", false)]);
    let mut source = SequentialDynamicSource::default();
    assert_eq!(
        resolve("{{$guid}}", &scopes, &mut source).text,
        "00000000-0000-4000-8000-000000000001"
    );
}

#[test]
fn dynamic_variable_parses_known_names_only() {
    assert_eq!(DynamicVariable::parse("$guid"), Some(DynamicVariable::Guid));
    assert_eq!(
        DynamicVariable::parse("$randomUUID"),
        Some(DynamicVariable::RandomUuid)
    );
    assert_eq!(
        DynamicVariable::parse("$timestamp"),
        Some(DynamicVariable::Timestamp)
    );
    assert_eq!(
        DynamicVariable::parse("$isoTimestamp"),
        Some(DynamicVariable::IsoTimestamp)
    );
    assert_eq!(
        DynamicVariable::parse("$randomInt"),
        Some(DynamicVariable::RandomInt)
    );
    assert_eq!(DynamicVariable::parse("guid"), None);
    assert_eq!(DynamicVariable::parse("$nope"), None);
}

#[test]
fn system_source_produces_well_formed_values() {
    let mut source = SystemDynamicSource;
    let first = source.generate(DynamicVariable::Guid);
    let second = source.generate(DynamicVariable::RandomUuid);
    for value in [&first, &second] {
        let parsed = uuid::Uuid::parse_str(value).map(|id| id.get_version_num());
        assert_eq!(parsed, Ok(4), "{value}");
    }
    assert_ne!(first, second);

    let seconds: Result<i64, _> = source.generate(DynamicVariable::Timestamp).parse();
    assert!(seconds.is_ok_and(|s| s > 1_700_000_000));

    let iso = source.generate(DynamicVariable::IsoTimestamp);
    // `time` is built without parsing, so check the RFC 3339 UTC shape by hand.
    let shape: String = iso
        .chars()
        .map(|c| if c.is_ascii_digit() { 'd' } else { c })
        .collect();
    assert_eq!(shape, "dddd-dd-ddTdd:dd:ddZ", "{iso}");

    for _ in 0..200 {
        let value: Result<u32, _> = source.generate(DynamicVariable::RandomInt).parse();
        assert!(value.is_ok_and(|v| v <= 1000));
    }
}

// Listing references without resolving.
#[test]
fn referenced_names_are_unique_in_first_occurrence_order() {
    assert_eq!(
        referenced_names("{{b}} {{ a }} {{b}} {{$guid}} {{}} {{open"),
        unresolved(&["b", "a", "$guid"])
    );
}
