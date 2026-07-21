// SPDX-License-Identifier: Apache-2.0

mod pages;

use pliego_ssg::{Asset, Site};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/site"));
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generated");
    let public = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");
    let authored_pages = pages::all();
    let route_count = authored_pages.len();
    let mut site = Site::new();
    for page in authored_pages {
        site = site.page(page);
    }
    site = add_tree(site, &public)?;
    let generated_chunks = generated.join("chunks");
    if generated_chunks.exists() {
        site = add_tree_at(site, &generated_chunks, Path::new("assets/chunks"))?;
    }
    site = site
        .asset(Asset::new(
            "assets/site.css",
            std::fs::read(generated.join("site.css"))?,
        ))
        .asset(Asset::new(
            "assets/pliegocss.css",
            std::fs::read(generated.join("pliegocss.css"))?,
        ))
        .asset(Asset::new(
            "assets/laboratory.css",
            std::fs::read(generated.join("laboratory.css"))?,
        ))
        .asset(Asset::new(
            "assets/site.js",
            std::fs::read(generated.join("site.js"))?,
        ))
        .asset(Asset::new(
            "assets/laboratory.json",
            std::fs::read(generated.join("laboratory.json"))?,
        ))
        .asset(Asset::new(
            "assets/catalog.json",
            std::fs::read(generated.join("catalog.json"))?,
        ))
        .asset(Asset::new("sitemap.xml", pages::sitemap()));

    let report = site.build(&output)?;
    println!(
        "PliegoCSS site: {route_count} routes and {} files -> {}",
        report.receipt.outputs.files.len(),
        output.display()
    );
    Ok(())
}

fn add_tree(site: Site, root: &Path) -> Result<Site, Box<dyn std::error::Error>> {
    add_tree_at(site, root, Path::new(""))
}

fn add_tree_at(
    mut site: Site,
    root: &Path,
    output_prefix: &Path,
) -> Result<Site, Box<dyn std::error::Error>> {
    fn walk(
        site: &mut Option<Site>,
        root: &Path,
        current: &Path,
        output_prefix: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(site, root, &path, output_prefix)?;
            } else {
                let relative = output_prefix
                    .join(path.strip_prefix(root)?)
                    .to_string_lossy()
                    .replace('\\', "/");
                *site = Some(
                    site.take()
                        .expect("site")
                        .asset(Asset::new(relative, std::fs::read(&path)?)),
                );
            }
        }
        Ok(())
    }
    let mut owned = Some(site);
    walk(&mut owned, root, root, output_prefix)?;
    site = owned.expect("site");
    Ok(site)
}
