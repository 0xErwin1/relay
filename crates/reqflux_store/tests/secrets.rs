//! S10: secret values never come from or go to a workspace file.

mod common;

use common::*;
use pretty_assertions::assert_eq;
use reqflux_core::model::Variable;

#[test]
fn loading_drops_secret_values_and_warns_per_key() {
    let dir = workspace();
    let root = dir.path();
    write(
        root,
        "reqflux.toml",
        &format!(
            "{}\n[[globals]]\nkey = \"g\"\nvalue = \"1\"\nsecret = true\n",
            manifest(1)
        ),
    );
    write(
        root,
        "environments/dev.env.toml",
        &collection_toml(
            1,
            "Dev",
            "\n[[variables]]\nkey = \"token\"\nvalue = \"s3cr3t\"\nsecret = true\n\n[[variables]]\nkey = \"host\"\nvalue = \"h\"\n\n[[variables]]\nkey = \"pw\"\nvalue = \"x\"\nsecret = true\n",
        ),
    );
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(
            2,
            "API",
            "\n[[variables]]\nkey = \"c\"\nvalue = \"v\"\nsecret = true\n",
        ),
    );

    let loaded = load(root);

    let env = path(root, "environments/dev.env.toml");
    assert_eq!(
        warnings(&loaded),
        vec![
            format!(
                "{}: secret variable c has a value in the workspace file; it was ignored",
                path(root, "collections/api/collection.toml").display()
            ),
            format!(
                "{}: secret variable token has a value in the workspace file; it was ignored",
                env.display()
            ),
            format!(
                "{}: secret variable pw has a value in the workspace file; it was ignored",
                env.display()
            ),
            format!(
                "{}: secret variable g has a value in the workspace file; it was ignored",
                path(root, "reqflux.toml").display()
            ),
        ]
    );
    let variables = &loaded.workspace.environments[0].environment.variables;
    assert_eq!(variables[0].value, None);
    assert_eq!(variables[1].value, Some("h".to_owned()));
    assert_eq!(loaded.workspace.manifest.globals[0].value, None);
    assert_eq!(
        loaded.workspace.collections[0].collection.variables[0].value,
        None
    );
}

#[test]
fn saving_never_writes_a_secret_value() {
    let dir = minimal_workspace();
    let root = dir.path();
    let mut secret = Variable::new("token", "s3cr3t");
    secret.secret = true;
    let environment = reqflux_core::model::Environment {
        id: id(1),
        name: "Dev".into(),
        variables: vec![secret],
        extensions: toml::Table::new(),
    };

    let result = reqflux_store::save_environment(root, "dev", &environment, None);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        read(root, "environments/dev.env.toml"),
        format!(
            "id = \"{}\"\nname = \"Dev\"\n\n[[variables]]\nkey = \"token\"\nsecret = true\n",
            id_text(1)
        )
    );
}
