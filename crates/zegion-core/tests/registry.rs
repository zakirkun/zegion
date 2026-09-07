use zegion_core::plugins::PluginRegistry;
use zegion_core::skills::SkillRegistry;

#[tokio::test]
async fn autoloads_all_skills_from_disk() {
    let skills = SkillRegistry::load_from_dir("../../skills").await.unwrap();
    assert!(
        skills.skills.len() >= 15,
        "expected >=15 skills, found {}",
        skills.skills.len()
    );
    // Each skill should have a name and non-empty body.
    for s in &skills.skills {
        assert!(!s.name.is_empty());
        assert!(!s.body.trim().is_empty());
    }
    // Catalog should list them for the system prompt.
    assert!(skills.catalog().contains("research"));
    assert!(skills.catalog().contains("code_review"));
}

#[tokio::test]
async fn autoloads_all_plugins_from_disk() {
    let plugins = PluginRegistry::load_from_dir("../../plugins")
        .await
        .unwrap();
    assert!(
        plugins.scripts.len() >= 10,
        "expected >=10 plugins, found {}",
        plugins.scripts.len()
    );
    let names = plugins.names();
    for expected in [
        "slugify",
        "reverse",
        "word_count",
        "sort_lines",
        "csv_to_table",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing plugin {expected}"
        );
    }
}

#[test]
fn rhai_plugins_execute() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let plugins = rt
        .block_on(PluginRegistry::load_from_dir("../../plugins"))
        .unwrap();

    // Run every plugin with sample input and assert it produces output without error.
    let cases = [
        ("reverse", "zegion"),
        ("word_count", "hello world foo"),
        ("title_case", "hello world"),
        ("is_palindrome", "level"),
        ("slugify", "Hello World! Rust"),
        ("sort_lines", "banana\napple\ncherry"),
        ("dedupe_lines", "a\nb\na\nc"),
        (
            "wrap",
            "one two three four five six seven eight nine ten eleven twelve thirteen",
        ),
        ("censor", "contact me at bob@example.com now"),
        ("csv_to_table", "name,age\nalice,30\nbob,25"),
        ("hello", "hi"),
    ];
    for (name, input) in cases {
        let out = plugins
            .run(name, input)
            .unwrap_or_else(|e| panic!("plugin `{name}` failed: {e}"));
        assert!(!out.is_empty(), "plugin `{name}` produced empty output");
    }

    // Spot-check correctness on a few.
    assert_eq!(plugins.run("reverse", "zegion").unwrap(), "noigez");
    assert_eq!(plugins.run("is_palindrome", "level").unwrap(), "palindrome");
}
