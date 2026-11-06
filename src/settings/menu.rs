//! Écrans du menu `/settings`, générés depuis [`SCHEMA`] (aucune liste dupliquée).
//!
//! Données de rappel (toujours < 64 octets) :
//! `s:b` racine · `s:o:<section>` rubrique · `s:k:<clé>` réglage · `s:v:<clé>:<valeur>` modifier ·
//! `s:p:<clé>` saisie au clavier (IdList, ou entier libre).
//! Ces données viennent du client : toute valeur repasse par `SettingsStore::set`.

use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use super::schema::{spec, Kind, Section, Spec, Value, Values, SCHEMA};

/// Action décodée d'un bouton du menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Revenir à la racine.
    Back,
    OpenSection(Section),
    OpenKey(&'static str),
    /// Appeler `store.set(key, &value, user_id)` puis réafficher `item(key, …)`.
    Set {
        key: &'static str,
        value: String,
    },
    /// Envoyer `prompt_text(key)` puis passer le prochain message texte de l'admin à `store.set`.
    Prompt(&'static str),
}

/// Décode une donnée de rappel ; `None` si elle n'est pas à nous ou est invalide.
pub fn parse_callback(data: &str) -> Option<Action> {
    let rest = data.strip_prefix("s:")?;
    if rest == "b" {
        return Some(Action::Back);
    }
    let (op, arg) = rest.split_once(':')?;
    let key = |k: &str| spec(k).map(|s| s.key);
    match op {
        "o" => Section::from_id(arg).map(Action::OpenSection),
        "k" => key(arg).map(Action::OpenKey),
        "p" => key(arg).map(Action::Prompt),
        "v" => {
            let (k, v) = arg.split_once(':')?;
            Some(Action::Set {
                key: key(k)?,
                value: v.to_string(),
            })
        }
        _ => None,
    }
}

fn btn(text: impl Into<String>, data: String) -> InlineKeyboardButton {
    debug_assert!(data.len() < 64, "callback trop long : {data}");
    InlineKeyboardButton::callback(text, data)
}

fn shown(v: Option<&Value>) -> String {
    match v {
        Some(Value::Bool(b)) => (if *b { "activé" } else { "désactivé" }).into(),
        Some(Value::Ids(ids)) if ids.is_empty() => "(vide)".into(),
        Some(v) => v.to_string(),
        None => "?".into(),
    }
}

/// Menu racine : une ligne par rubrique.
pub fn root() -> (String, InlineKeyboardMarkup) {
    let rows = Section::ALL
        .iter()
        .map(|s| vec![btn(s.label(), format!("s:o:{}", s.id()))]);
    (
        "⚙️ Réglages\nChoisis une rubrique.".into(),
        InlineKeyboardMarkup::new(rows),
    )
}

/// Écran d'une rubrique : ses réglages avec leur valeur courante.
pub fn section(s: Section, values: &Values) -> (String, InlineKeyboardMarkup) {
    let specs = SCHEMA.iter().filter(|x| x.section == s);
    let mut text = format!("⚙️ {}", s.label());
    let mut rows = Vec::new();
    for x in specs {
        let v = shown(values.get(x.key));
        text.push_str(&format!("\n• {} : {v}", x.label));
        rows.push(vec![btn(
            format!("{} : {v}", x.label),
            format!("s:k:{}", x.key),
        )]);
    }
    rows.push(vec![btn("⬅️ Retour", "s:b".into())]);
    (text, InlineKeyboardMarkup::new(rows))
}

/// Écran d'un réglage ; `None` si la clé est inconnue.
pub fn item(key: &str, values: &Values) -> Option<(String, InlineKeyboardMarkup)> {
    let x = spec(key)?;
    let cur = values.get(x.key);
    let mut text = format!(
        "⚙️ {} › {}\n{}\n\nValeur : {}",
        x.section.label(),
        x.label,
        x.description,
        shown(cur)
    );
    if let Kind::U64 { min, max, .. } = x.kind {
        text.push_str(&format!(" (de {min} à {max})"));
    }
    if let Some(w) = x.warning {
        text.push_str(&format!("\n⚠️ {w}"));
    }
    if x.kind != Kind::ReadOnly {
        text.push_str(if x.hot {
            "\n\nAppliqué à chaud."
        } else {
            "\n\nEffet au prochain redémarrage."
        });
    }
    let mut rows = buttons(x, cur);
    rows.push(vec![btn("⬅️ Retour", format!("s:o:{}", x.section.id()))]);
    Some((text, InlineKeyboardMarkup::new(rows)))
}

/// Texte à envoyer pour une [`Action::Prompt`].
pub fn prompt_text(key: &str) -> Option<String> {
    let x = spec(key)?;
    Some(match x.kind {
        Kind::IdList => format!(
            "Envoie la nouvelle liste pour « {} » : IDs Telegram numériques séparés par des virgules (pas de pseudo @…). Elle remplace la liste actuelle.",
            x.label
        ),
        Kind::U64 { min, max, .. } => {
            format!("Envoie la nouvelle valeur pour « {} » (de {min} à {max}).", x.label)
        }
        _ => format!("Envoie la nouvelle valeur pour « {} ».", x.label),
    })
}

fn set(x: &Spec, v: impl std::fmt::Display) -> String {
    format!("s:v:{}:{v}", x.key)
}

fn buttons(x: &Spec, cur: Option<&Value>) -> Vec<Vec<InlineKeyboardButton>> {
    match (x.kind, cur) {
        (Kind::Bool, Some(Value::Bool(b))) => vec![vec![btn(
            if *b { "Désactiver" } else { "Activer" },
            set(x, if *b { "off" } else { "on" }),
        )]],
        (Kind::U64 { min, max, step }, Some(Value::U64(n))) => {
            let mut row = Vec::new();
            if *n > min {
                row.push(btn(
                    format!("−{step}"),
                    set(x, n.saturating_sub(step).max(min)),
                ));
            }
            if *n < max {
                row.push(btn(
                    format!("+{step}"),
                    set(x, n.saturating_add(step).min(max)),
                ));
            }
            vec![row, vec![btn("✏️ Saisir", format!("s:p:{}", x.key))]]
        }
        (Kind::Choice(opts), cur) => vec![opts
            .iter()
            .map(|o| {
                let mark = if cur == Some(&Value::Text((*o).into())) {
                    "✅ "
                } else {
                    ""
                };
                btn(format!("{mark}{o}"), set(x, o))
            })
            .collect()],
        (Kind::Order(_), Some(Value::List(l))) => (1..l.len())
            .map(|i| {
                let mut moved = l.clone();
                moved.swap(i - 1, i);
                vec![btn(format!("⬆️ {}", l[i]), set(x, moved.join(",")))]
            })
            .collect(),
        (Kind::IdList, _) => vec![vec![btn("✏️ Modifier la liste", format!("s:p:{}", x.key))]],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::{defaults_from, parse, tests::cfg};
    use teloxide::types::InlineKeyboardButtonKind;

    fn datas(m: &InlineKeyboardMarkup) -> Vec<String> {
        m.inline_keyboard
            .iter()
            .flatten()
            .filter_map(|b| match &b.kind {
                InlineKeyboardButtonKind::CallbackData(d) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chaque_spec_a_un_ecran_et_des_callbacks_courts() {
        let v = defaults_from(&cfg());
        let mut all = datas(&root().1);
        for s in Section::ALL {
            let (text, kb) = section(s, &v);
            assert!(text.contains(s.label()));
            all.extend(datas(&kb));
        }
        for x in SCHEMA {
            let (text, kb) = item(x.key, &v).unwrap_or_else(|| panic!("{}", x.key));
            assert!(text.contains(x.label), "{}", x.key);
            if x.kind != Kind::ReadOnly {
                assert!(text.contains("à chaud") || text.contains("redémarrage"));
            }
            assert!(all.contains(&format!("s:k:{}", x.key)), "{} absent", x.key);
            all.extend(datas(&kb));
        }
        assert!(all.len() > SCHEMA.len());
        for d in &all {
            assert!(d.len() < 64, "{d}");
            let a = parse_callback(d).unwrap_or_else(|| panic!("indécodable : {d}"));
            // toute valeur proposée par un bouton passe la validation stricte
            if let Action::Set { key, value } = a {
                assert!(parse(spec(key).unwrap(), &value).is_ok(), "{d}");
            }
        }
    }

    #[test]
    fn callbacks_decodes_et_refuses() {
        assert_eq!(parse_callback("s:b"), Some(Action::Back));
        assert_eq!(
            parse_callback("s:o:src"),
            Some(Action::OpenSection(Section::Sources))
        );
        assert_eq!(
            parse_callback("s:v:providers.order:tiktok,x,bluesky"),
            Some(Action::Set {
                key: "providers.order",
                value: "tiktok,x,bluesky".into()
            })
        );
        assert_eq!(
            parse_callback("s:p:acc.admins"),
            Some(Action::Prompt("acc.admins"))
        );
        for bad in [
            "",
            "x:b",
            "s:",
            "s:o:zz",
            "s:k:inconnu",
            "s:v:dl.quality",
            "s:z:a",
        ] {
            assert_eq!(parse_callback(bad), None, "{bad}");
        }
    }

    #[test]
    fn ecran_tiktok_avertit_et_boutons_bornes() {
        let mut v = defaults_from(&cfg());
        let (t, _) = item("providers.tiktok.enabled", &v).unwrap();
        assert!(t.contains("API publique tierce"));
        v.insert("dl.max_filesize_mb", Value::U64(4000));
        let d = datas(&item("dl.max_filesize_mb", &v).unwrap().1);
        assert!(d.contains(&"s:v:dl.max_filesize_mb:3950".to_string()));
        assert!(!d.iter().any(|x| x.ends_with(":4050")));
        assert!(prompt_text("acc.allowed_users")
            .unwrap()
            .contains("numériques"));
        assert!(item("inconnu", &v).is_none());
    }
}
