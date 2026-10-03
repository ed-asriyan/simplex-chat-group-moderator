use super::{CategoryTrigger, OpenAiCategory, OpenAiCategoryTriggers};

fn only(category: OpenAiCategory, trigger: CategoryTrigger) -> OpenAiCategoryTriggers {
    let mut triggers = OpenAiCategoryTriggers::default();
    triggers.set(category, trigger);
    triggers
}

// ---------------------------------------------------------------------------
// Category names
// ---------------------------------------------------------------------------

#[test]
fn test_api_names_are_the_ones_openai_uses() {
    let expected = [
        (OpenAiCategory::Sexual, "sexual"),
        (OpenAiCategory::SexualMinors, "sexual/minors"),
        (OpenAiCategory::Harassment, "harassment"),
        (
            OpenAiCategory::HarassmentThreatening,
            "harassment/threatening",
        ),
        (OpenAiCategory::Hate, "hate"),
        (OpenAiCategory::HateThreatening, "hate/threatening"),
        (OpenAiCategory::Illicit, "illicit"),
        (OpenAiCategory::IllicitViolent, "illicit/violent"),
        (OpenAiCategory::SelfHarm, "self-harm"),
        (OpenAiCategory::SelfHarmIntent, "self-harm/intent"),
        (
            OpenAiCategory::SelfHarmInstructions,
            "self-harm/instructions",
        ),
        (OpenAiCategory::Violence, "violence"),
        (OpenAiCategory::ViolenceGraphic, "violence/graphic"),
    ];
    assert_eq!(expected.len(), OpenAiCategory::ALL.len());
    for (category, api_name) in expected {
        assert_eq!(category.api_name(), api_name);
        assert_eq!(OpenAiCategory::from_api_name(api_name), Some(category));
    }
}

#[test]
fn test_name_is_the_json_field_name_and_the_serde_name() {
    // The editor's field, the database row and serde must all agree, or a
    // category silently turns into "off" somewhere between the three.
    let fields = serde_json::to_value(OpenAiCategoryTriggers::default()).unwrap();
    let fields = fields.as_object().unwrap();
    assert_eq!(fields.len(), OpenAiCategory::ALL.len());
    for category in OpenAiCategory::ALL {
        let name = category.name();
        assert!(fields.contains_key(name), "no JSON field named {name}");
        assert_eq!(serde_json::to_value(category).unwrap(), name);
        assert_eq!(OpenAiCategory::from_name(name), Some(category));
    }
}

#[test]
fn test_unknown_names_are_not_categories() {
    // OpenAI may add categories; they must be ignored rather than misread.
    assert_eq!(OpenAiCategory::from_api_name("new-category/sub"), None);
    assert_eq!(OpenAiCategory::from_api_name("hate_threatening"), None);
    assert_eq!(OpenAiCategory::from_name("hate/threatening"), None);
    assert_eq!(OpenAiCategory::from_name(""), None);
}

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

#[test]
fn test_all_sets_every_category() {
    let triggers = OpenAiCategoryTriggers::all(CategoryTrigger::OpenAiDecides);
    for category in OpenAiCategory::ALL {
        assert_eq!(triggers.get(category), CategoryTrigger::OpenAiDecides);
    }
}

#[test]
fn test_set_changes_only_its_own_category() {
    for target in OpenAiCategory::ALL {
        let triggers = only(target, CategoryTrigger::MinScorePercent(42));
        for category in OpenAiCategory::ALL {
            let expected = if category == target {
                CategoryTrigger::MinScorePercent(42)
            } else {
                CategoryTrigger::Off
            };
            assert_eq!(triggers.get(category), expected, "{category:?}");
        }
    }
}

#[test]
fn test_default_triggers_are_all_off() {
    assert_eq!(OpenAiCategoryTriggers::default().hate, CategoryTrigger::Off);
    assert_eq!(
        OpenAiCategoryTriggers::default().violence_graphic,
        CategoryTrigger::Off
    );
}

#[test]
fn test_trigger_wire_format() {
    let parse = |json: &str| serde_json::from_str::<CategoryTrigger>(json);
    assert_eq!(parse(r#""off""#).unwrap(), CategoryTrigger::Off);
    assert_eq!(
        parse(r#""openai""#).unwrap(),
        CategoryTrigger::OpenAiDecides
    );
    assert_eq!(parse("80").unwrap(), CategoryTrigger::MinScorePercent(80));

    assert_eq!(
        serde_json::to_string(&CategoryTrigger::Off).unwrap(),
        r#""off""#
    );
    assert_eq!(
        serde_json::to_string(&CategoryTrigger::OpenAiDecides).unwrap(),
        r#""openai""#
    );
    assert_eq!(
        serde_json::to_string(&CategoryTrigger::MinScorePercent(80)).unwrap(),
        "80"
    );
}

#[test]
fn test_trigger_wire_format_rejects_anything_else() {
    let parse = |json: &str| serde_json::from_str::<CategoryTrigger>(json);
    for bad in [
        r#""maybe""#,
        r#""OpenAI""#,
        r#""""#,
        "-1",
        "256",
        "42.5",
        "true",
        "null",
    ] {
        assert!(parse(bad).is_err(), "{bad} should not parse");
    }
}

#[test]
fn test_missing_category_field_is_off() {
    let triggers: OpenAiCategoryTriggers = serde_json::from_str(r#"{ "hate": "openai" }"#).unwrap();
    assert_eq!(triggers.hate, CategoryTrigger::OpenAiDecides);
    assert_eq!(triggers.violence, CategoryTrigger::Off);
}
