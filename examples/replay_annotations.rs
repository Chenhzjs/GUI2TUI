//! Read-only research projection: no GUI connection, runtime or operation ticket.
use clap::Parser;
use gui2tui::transcompile::document::annotations::{self, AnnotationMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{self, Read},
};

#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    mode: AnnotationMode,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    let document: Value = serde_json::from_slice(&input)?;
    let annotations =
        annotations::from_public_projection(&document, args.mode).map_err(io::Error::other)?;
    let digest: String = Sha256::digest(&input)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!(
        "{}",
        json!({"version":"gui2tui.annotation_replay.v1",
        "input_sha256":digest,"execution_authority":false,"annotations":annotations})
    );
    Ok(())
}
