//! Tour a headless project settings screen with five behaviors.
//! Run with `cargo run -p rust-use --example agent_tour`.
//! Read the snapshot, update all five controls in one batch, then search for countries.

use rust_use::prelude::*;
use std::{error::Error, io::Write};

#[derive(Primitive)]
#[primitive(no_actions)]
struct Control<B: Behavior> {
    #[behavior]
    input: B,
}

fn main() -> Result<(), Box<dyn Error>> {
    tour(std::io::stdout().lock())
}

fn tour(mut out: impl Write) -> Result<(), Box<dyn Error>> {
    let ui = Ui::new();
    ui.mount(Control {
        input: Tabs::new(["General", "Members", "Notifications"]),
    })
    .label("Settings");
    ui.mount(Control {
        input: Range::new(0.0, 100.0),
    })
    .label("Volume");
    ui.mount(Control {
        input: Combobox::new(COUNTRIES.iter().map(|&name| name.to_owned()).collect()),
    })
    .label("Country");
    ui.mount(Control {
        input: Tree::<()>::from_paths(["docs/guide.md", "src/main.rs", "README.md"]),
    })
    .label("Files");
    ui.mount(Control {
        input: Transfer::new()
            .with_list("todo", vec!["Write docs".to_owned()])
            .with_list("doing", vec!["Review changes".to_owned()])
            .with_list("done", vec!["Create project".to_owned()]),
    })
    .label("Kanban");

    let agent = ui.agent();
    writeln!(out, "{}", agent.snapshot())?;
    let reply = agent.act(
        r#"Settings select Notifications
Volume set 40
Country pick sweden
Files select docs/guide.md
Kanban move "Write docs" doing"#,
    )?;
    writeln!(out, "\n{reply}")?;
    writeln!(out, "\n{}", agent.query("Country", Some("sw"), 0)?)?;
    Ok(())
}

#[test]
fn tour_updates_all_five_controls_and_searches_countries() -> Result<(), Box<dyn Error>> {
    let mut output = Vec::new();
    tour(&mut output)?;
    let output = String::from_utf8(output)?;
    let sections: Vec<_> = output.trim_end().split("\n\n").collect();
    let [snapshot, reply, countries] = sections.as_slice() else {
        return Err("expected a snapshot, action reply, and query reply".into());
    };
    assert!(snapshot.contains(&format!(
        r#""Country" n={} !pick(option:any) ?query"#,
        COUNTRIES.len()
    )));
    assert!(reply.starts_with("ok\n"));
    for change in [
        r#""Settings" =Notifications"#,
        r#""Volume" =40"#,
        r#""Country" =Sweden"#,
        r#""Files" sel=docs/guide.md"#,
        r#""Kanban" todo=0 doing=2 done=1"#,
    ] {
        assert!(reply.contains(change), "{reply}");
    }
    assert_eq!(
        *countries,
        "Botswana\nEswatini\nSweden\nSwitzerland\ntotal 4"
    );
    Ok(())
}

// Country and territory names used as sample options.
#[rustfmt::skip]
const COUNTRIES: &[&str] = &[
    "Afghanistan", "Albania", "Algeria", "American Samoa", "Andorra", "Angola", "Anguilla",
    "Antarctica", "Antigua and Barbuda", "Argentina", "Armenia", "Aruba", "Ascension Island",
    "Australia", "Austria", "Azerbaijan", "Bahamas", "Bahrain", "Bangladesh", "Barbados",
    "Belarus", "Belgium", "Belize", "Benin", "Bermuda", "Bhutan", "Bolivia",
    "Bonaire, Sint Eustatius and Saba", "Bosnia and Herzegovina", "Botswana", "Bouvet Island",
    "Brazil", "Britain (UK)", "British Indian Ocean Territory", "Brunei", "Bulgaria",
    "Burkina Faso", "Burundi", "Cabo Verde", "Cambodia", "Cameroon", "Canada", "Canary Islands",
    "Cayman Islands", "Central African Republic", "Chad", "Chile", "China", "Christmas Island",
    "Cocos (Keeling) Islands", "Colombia", "Comoros", "Cook Islands", "Costa Rica", "Croatia",
    "Cuba", "Curaçao", "Cyprus", "Czechia", "Côte d’Ivoire", "Democratic Republic of the Congo",
    "Denmark", "Djibouti", "Dominica", "Dominican Republic", "East Timor", "Ecuador", "Egypt",
    "El Salvador", "Equatorial Guinea", "Eritrea", "Estonia", "Eswatini", "Ethiopia",
    "Falkland Islands", "Faroe Islands", "Fiji", "Finland", "France", "French Guiana",
    "French Polynesia", "French S. Terr.", "Gabon", "Gambia", "Georgia", "Germany", "Ghana",
    "Gibraltar", "Greece", "Greenland", "Grenada", "Guadeloupe", "Guam", "Guatemala", "Guernsey",
    "Guinea", "Guinea-Bissau", "Guyana", "Haiti", "Heard Island & McDonald Islands", "Honduras",
    "Hong Kong", "Hungary", "Iceland", "India", "Indonesia", "Iran", "Iraq", "Ireland",
    "Isle of Man", "Israel", "Italy", "Jamaica", "Japan", "Jersey", "Jordan", "Kazakhstan",
    "Kenya", "Kiribati", "Korea (North)", "Korea (South)", "Kosovo", "Kuwait", "Kyrgyzstan",
    "Laos", "Latvia", "Lebanon", "Lesotho", "Liberia", "Libya", "Liechtenstein", "Lithuania",
    "Luxembourg", "Macau", "Madagascar", "Malawi", "Malaysia", "Maldives", "Mali", "Malta",
    "Marshall Islands", "Martinique", "Mauritania", "Mauritius", "Mayotte", "Mexico",
    "Micronesia", "Moldova", "Monaco", "Mongolia", "Montenegro", "Montserrat", "Morocco",
    "Mozambique", "Myanmar (Burma)", "Namibia", "Nauru", "Nepal", "Netherlands", "New Caledonia",
    "New Zealand", "Nicaragua", "Niger", "Nigeria", "Niue", "Norfolk Island", "North Macedonia",
    "Northern Mariana Islands", "Norway", "Oman", "Pakistan", "Palau", "Palestine", "Panama",
    "Papua New Guinea", "Paraguay", "Peru", "Philippines", "Pitcairn", "Poland", "Portugal",
    "Puerto Rico", "Qatar", "Republic of the Congo", "Romania", "Russia", "Rwanda", "Réunion",
    "Saint Barthélemy", "Samoa (western)", "San Marino", "Sao Tome & Principe", "Saudi Arabia",
    "Senegal", "Serbia", "Seychelles", "Sierra Leone", "Singapore", "Slovakia", "Slovenia",
    "Solomon Islands", "Somalia", "South Africa", "South Georgia & the South Sandwich Islands",
    "South Sudan", "Spain", "Sri Lanka", "St Helena", "St Kitts & Nevis", "St Lucia",
    "St Maarten (Dutch)", "St Martin (French)", "St Pierre & Miquelon", "St Vincent", "Sudan",
    "Suriname", "Svalbard & Jan Mayen", "Sweden", "Switzerland", "Syria", "Taiwan", "Tajikistan",
    "Tanzania", "Thailand", "Togo", "Tokelau", "Tonga", "Trinidad & Tobago", "Tristan da Cunha",
    "Tunisia", "Turkmenistan", "Turks & Caicos Is", "Tuvalu", "Türkiye", "Uganda", "Ukraine",
    "United Arab Emirates", "United States", "Uruguay", "US minor outlying islands", "Uzbekistan",
    "Vanuatu", "Vatican City", "Venezuela", "Vietnam", "Virgin Islands (UK)",
    "Virgin Islands (US)", "Wallis & Futuna", "Western Sahara", "Yemen", "Zambia", "Zimbabwe",
    "Åland Islands",
];
