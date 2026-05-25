use std::fs;
use std::path::Path;

use brain_brew_formats::{canonical_yaml, crowdanki};

use crate::args::parse_required_out;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("crowdanki") {
        return Err(
            "usage: brainbrew import crowdanki <deck-folder> --accept-suggested-ids --out deck.yaml"
                .to_owned(),
        );
    }
    if !args.iter().any(|arg| arg == "--accept-suggested-ids") {
        return Err(
            "non-interactive CrowdAnki import requires --accept-suggested-ids for now".to_owned(),
        );
    }
    if args.len() < 5 {
        return Err(
            "usage: brainbrew import crowdanki <deck-folder> --accept-suggested-ids --out deck.yaml"
                .to_owned(),
        );
    }

    let deck_dir = Path::new(&args[1]);
    let out_path = parse_required_out(&args[2..])?;
    let deck_json_path = deck_dir.join("deck.json");
    let deck_json = fs::read_to_string(&deck_json_path)
        .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;
    let deck = crowdanki::import_deck_accept_suggested_ids(&deck_json)
        .map_err(|error| error.to_string())?;
    let yaml = canonical_yaml::to_string(&deck).map_err(|error| error.to_string())?;
    fs::write(&out_path, yaml).map_err(|error| format!("{}: {error}", out_path.display()))?;
    println!("imported crowdanki deck");
    Ok(())
}
