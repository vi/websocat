use std::collections::HashMap;

use anyhow::Result;

use super::PropertyValueTypeTag;

use super::CliOptionDescription;

impl super::ClassRegistrar {
    /// Get all class-injected long CLI options with their types
    pub fn get_all_cli_options(&self) -> Result<HashMap<String, CliOptionDescription>> {
        let mut v: HashMap<String, CliOptionDescription> = HashMap::with_capacity(32);
        // for error reporintg
        let mut provenance = <HashMap<String, String>>::with_capacity(32);

        for k in self.classes.values() {
            for p in k.properties() {
                if let Some(clin) = p.inject_cli_long_option {
                    let prov = format!("{}::{}", k.official_name(), p.name);
                    let pi = CliOptionDescription {
                        typ: p.r#type,
                        for_array: false,
                    };
                    register_opt(&mut v, clin, pi, &mut provenance, prov)?;
                }
            }
            if let Some(clin) = k.array_inject_cli_long_opt() {
                let prov = format!("{}::<array>", k.official_name());
                let arrtyp = k.array_type();
                if arrtyp.is_none() {
                    anyhow::bail!(
                        "Internal error: attempt to create CLI option `{}` for a node class that does not accept array: `{}`.",
                        clin,
                        prov,
                    );
                }
                let arrtyp = arrtyp.unwrap();
                let pi = CliOptionDescription {
                    typ: arrtyp,
                    for_array: true,
                };
                register_opt(&mut v, clin, pi, &mut provenance, prov)?;
            }
        }

        for m in self.macros.values() {
            for (i, (clin, pi)) in m.injected_cli_opts().into_iter().enumerate() {
                let prov = format!("macro {}::#{}", m.official_name(), i);
                register_opt(&mut v, clin, pi, &mut provenance, prov)?;
            }
        }

        Ok(v)
    }
}

fn register_opt(
    v: &mut HashMap<String, CliOptionDescription>,
    clin: String,
    pi: CliOptionDescription,
    provenance: &mut HashMap<String, String>,
    prov: String,
) -> Result<()> {
    if pi.typ.tag() == PropertyValueTypeTag::ChildNode {
        anyhow::bail!(
            "Internal error: attempt to create CLI option `{}` for `{}` of a not allowed type \"submonode\".",
            clin,
            prov,
        );
    }
    match v.entry(clin.clone()) {
        std::collections::hash_map::Entry::Occupied(x) => {
            if pi.for_array != x.get().for_array {
                anyhow::bail!(
                    "Internal error: conflicting usages of long CLI option `{}`. `{}` and `{}` disagree whether it is a scalar or array property",
                    clin,
                    provenance[&*clin],
                    prov,
                );
            }
            if &x.get().typ == &pi.typ {
                tracing::debug!(
                    "CLI long option `{}` of type `{}` also maps to `{}`",
                    clin,
                    pi.typ,
                    prov,
                );
            } else {
                anyhow::bail!(
                    "Internal error: conflicting types of long CLI option `{}`. Accorting to `{}` it should be `{}`, but according to `{}` it should be `{}`.",
                    clin,
                    provenance[&*clin],
                    x.get().typ,
                    prov,
                    pi.typ,
                );
            }
        }
        std::collections::hash_map::Entry::Vacant(x) => {
            tracing::debug!(
                "Inserting global CLI long option: `{}` of type `{}`, mapping to `{}`",
                clin,
                pi.typ,
                prov,
            );
            provenance.insert(clin.clone(), prov);
            x.insert(pi);
        }
    }
    Ok(())
}
