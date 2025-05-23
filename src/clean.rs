use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::{Path, PathBuf},
    time::SystemTime,
};

use cli_table::{Cell, Table, Style}; // print_stdout was unused
use color_eyre::eyre::{bail, eyre, Context, ContextCompat};
use nix::errno::Errno;
use nix::{
    fcntl::AtFlags,
    unistd::{faccessat, AccessFlags},
};
use regex::Regex;
use tracing::{debug, info, instrument, span, warn, Level};
use uzers::os::unix::UserExt;

use crate::{commands::Command, *};

// Nix impl:
// https://github.com/NixOS/nix/blob/master/src/nix-collect-garbage/nix-collect-garbage.cc

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Generation {
    number: u32,
    last_modified: SystemTime,
    path: PathBuf,
}

type ToBeRemoved = bool;
// BTreeMap to automatically sort generations by id
type GenerationsTagged = BTreeMap<Generation, ToBeRemoved>;
type ProfilesTagged = HashMap<PathBuf, GenerationsTagged>;

impl interface::CleanMode {
    pub fn run(&self, _verbose_count: u8) -> Result<()> {
        let mut profiles = Vec::new();
        let mut gcroots_tagged: HashMap<PathBuf, ToBeRemoved> = HashMap::new();
        let now = SystemTime::now();
        let mut is_profile_clean = false;

        // What profiles to clean depending on the call mode
        let uid = nix::unistd::Uid::effective();
        let args = match self {
            interface::CleanMode::Profile(args) => {
                profiles.push(args.profile.clone());
                is_profile_clean = true;
                &args.common
            }
            interface::CleanMode::All(args) => {
                if !uid.is_root() {
                    crate::self_elevate();
                }
                profiles.extend(profiles_in_dir("/nix/var/nix/profiles"));
                for read_dir in PathBuf::from("/nix/var/nix/profiles/per-user").read_dir()? {
                    let path = read_dir?.path();
                    profiles.extend(profiles_in_dir(path));
                }

                // Most unix systems start regular users at uid 1000+, but macos is special at 501+
                // https://en.wikipedia.org/wiki/User_identifier
                let uid_min = if cfg!(target_os = "macos") { 501 } else { 1000 };
                let uid_max = uid_min + 100;
                debug!("Scanning XDG profiles for users 0, ${uid_min}-${uid_max}");
                for user in unsafe { uzers::all_users() } {
                    if user.uid() >= uid_min && user.uid() < uid_max || user.uid() == 0 {
                        debug!(?user, "Adding XDG profiles for user");
                        profiles.extend(profiles_in_dir(
                            user.home_dir().join(".local/state/nix/profiles"),
                        ));
                    }
                }
                args
            }
            interface::CleanMode::User(args) => {
                if uid.is_root() {
                    bail!("ng clean user: don't run me as root!");
                }
                let user = nix::unistd::User::from_uid(uid)?.unwrap();
                profiles.extend(profiles_in_dir(
                    PathBuf::from(std::env::var("HOME")?).join(".local/state/nix/profiles"),
                ));
                profiles.extend(profiles_in_dir(
                    PathBuf::from("/nix/var/nix/profiles/per-user").join(user.name),
                ));
                args
            }
        };

        // Use mutation to raise errors as they come
        let mut profiles_tagged = ProfilesTagged::new();
        for p in profiles {
            profiles_tagged.insert(
                p.clone(),
                cleanable_generations(&p, args.keep, args.keep_since)?,
            );
        }

        // Query gcroots
        let filename_tests = [r".*/.direnv/.*", r".*result.*"];
        let regexes = filename_tests
            .into_iter()
            .map(Regex::new)
            .collect::<Result<Vec<_>, regex::Error>>()?;

        if !is_profile_clean && !args.nogcroots {
            for elem in PathBuf::from("/nix/var/nix/gcroots/auto")
                .read_dir()
                .wrap_err("Reading auto gcroots dir")?
            {
                let src = elem.wrap_err("Reading auto gcroots element")?.path();
                let dst = src.read_link().wrap_err("Reading symlink destination")?;
                let span = span!(Level::TRACE, "gcroot detection", ?dst);
                let _entered = span.enter();
                debug!(?src);

                if !regexes
                    .iter()
                    .any(|next| next.is_match(&dst.to_string_lossy()))
                {
                    debug!("dst doesn't match any gcroot regex, skipping");
                    continue;
                };

                // Use .exists to not travel symlinks
                if match faccessat(
                    None,
                    &dst,
                    AccessFlags::F_OK | AccessFlags::W_OK,
                    AtFlags::AT_SYMLINK_NOFOLLOW,
                ) {
                    Ok(_) => true,
                    Err(errno) => match errno {
                        Errno::EACCES | Errno::ENOENT => false,
                        _ => {
                            bail!(eyre!("Checking access for gcroot {:?}, unknown error", dst)
                                .wrap_err(errno))
                        }
                    },
                } {
                    let dur = now.duration_since(
                        dst.symlink_metadata()
                            .wrap_err("Reading gcroot metadata")?
                            .modified()?,
                    );
                    debug!(?dur);
                    match dur {
                        Err(err) => {
                            warn!(?err, ?now, "Failed to compare time!");
                        }
                        Ok(val) if val <= args.keep_since.into() => {
                            gcroots_tagged.insert(dst, false);
                        }
                        Ok(_) => {
                            gcroots_tagged.insert(dst, true);
                        }
                    }
                } else {
                    debug!("dst doesn't exist or is not writable, skipping");
                }
            }
        }

        // Present the user the information about the paths to clean
        use owo_colors::OwoColorize;
        println!();
        println!("{}", "Welcome to ng clean".bold());
        println!("Keeping {} generation(s)", args.keep.green());
        println!("Keeping paths newer than {}", args.keep_since.green());
        
        // Show which cleanup steps will be performed in a table format
        println!();
        println!("{}", "Cleanup steps:".bold());
        
        // Create a table to display cleanup steps
        let mut table = vec![];
        
        // Add rows for each cleanup step
        table.push(vec![
            "🗑️ Remove old generations".cell(),
            "Yes".cell().foreground_color(Some(cli_table::Color::Green)),
        ]);
        
        table.push(vec![
            "🧹 Run garbage collection".cell(),
            if args.nogc {
                "No".cell().foreground_color(Some(cli_table::Color::Red))
            } else {
                "Yes".cell().foreground_color(Some(cli_table::Color::Green))
            },
        ]);
        
        table.push(vec![
            "⚡ Optimize nix store".cell(),
            if args.nooptimise {
                "No".cell().foreground_color(Some(cli_table::Color::Red))
            } else {
                "Yes".cell().foreground_color(Some(cli_table::Color::Green))
            },
        ]);
        
        // Display the table
        let table = table.table()
            .title(vec![
                "Step".cell().bold(true),
                "Enabled".cell().bold(true),
            ])
            .bold(true);
        
        if let Err(e) = cli_table::print_stdout(table) {
            debug!("Failed to display cleanup steps as table: {}", e);
            // Fallback to simple text if table display fails
            println!("- 🗑️ Remove old generations: {}", "Yes".green());
            println!("- 🧹 Run garbage collection: {}", if args.nogc { "No".red().to_string() } else { "Yes".green().to_string() });
            println!("- ⚡ Optimize nix store: {}", if args.nooptimise { "No".red().to_string() } else { "Yes".green().to_string() });
        }
        
        println!();
        println!("legend:");
        println!("{}: path to be kept", "OK".green());
        println!("{}: path to be removed", "DEL".red());
        println!();
        if !gcroots_tagged.is_empty() {
            println!(
                "{}",
                "gcroots (matching the following regex patterns)"
                    .blue()
                    .bold()
            );
            for re in regexes {
                println!("- {}  {}", "RE".purple(), re);
            }
            for (path, tbr) in &gcroots_tagged {
                if *tbr {
                    println!("- {} {}", "DEL".red(), path.to_string_lossy());
                } else {
                    println!("- {} {}", "OK ".green(), path.to_string_lossy());
                }
            }
            println!();
        }
        for (profile, generations_tagged) in profiles_tagged.iter() {
            println!("{}", profile.to_string_lossy().blue().bold());
            for (gen, tbr) in generations_tagged.iter().rev() {
                if *tbr {
                    println!("- {} {}", "DEL".red(), gen.path.to_string_lossy());
                } else {
                    println!("- {} {}", "OK ".green(), gen.path.to_string_lossy());
                };
            }
            println!();
        }

        // Clean the paths
        if args.ask {
            info!("Confirm the cleanup plan?");
            if !dialoguer::Confirm::new().default(false).interact()? {
                bail!("User rejected the cleanup plan");
            }
        }

        if !args.dry {
            for (path, tbr) in &gcroots_tagged {
                if *tbr {
                    remove_path_nofail(path);
                }
            }

            for (_, generations_tagged) in profiles_tagged.iter() {
                for (gen, tbr) in generations_tagged.iter().rev() {
                    if *tbr {
                        remove_path_nofail(&gen.path);
                    }
                }
            }
        }

        // Check if we're running as root
        let uid = nix::unistd::Uid::effective();
        let is_root = uid.is_root();

        // Multi-step cleanup process with loading indicators
        if !args.nogc {
            // Start a spinner for the GC step with emoji icon
            let gc_spinner = crate::progress::start_spinner("[🧹 GC] Performing garbage collection on the nix store");
            
            // Run the GC command with sudo if not root
            if is_root {
                Command::new("nix-store")
                    .args(["--gc"])
                    .dry(args.dry)
                    .run()?;
            } else {
                Command::new("sudo")
                    .args(["nix-store", "--gc"])
                    .dry(args.dry)
                    .run()?;
            }
            
            // Finish the spinner with success
            crate::progress::finish_spinner_success(&gc_spinner, "[✅ GC] Garbage collection completed successfully");
        }

        if !args.nooptimise {
            // Start a spinner for the optimize step with emoji icon
            let optimize_spinner = crate::progress::start_spinner("[⚡ Optimize] Optimizing Nix store");
            
            // Run the optimize command with sudo if not root
            if is_root {
                Command::new("nix-store")
                    .args(["--optimise"])
                    .dry(args.dry)
                    .run()?;
            } else {
                Command::new("sudo")
                    .args(["nix-store", "--optimise"])
                    .dry(args.dry)
                    .run()?;
            }
            
            // Finish the spinner with success
            crate::progress::finish_spinner_success(&optimize_spinner, "[✅ Optimize] Store optimization completed successfully");
        }

        Ok(())
    }
}

#[instrument(ret, level = "debug")]
fn profiles_in_dir<P: AsRef<Path> + fmt::Debug>(dir: P) -> Vec<PathBuf> {
    let mut res = Vec::new();
    let dir = dir.as_ref();

    match dir.read_dir() {
        Ok(read_dir) => {
            for entry in read_dir {
                match entry {
                    Ok(e) => {
                        let path = e.path();

                        if let Ok(dst) = path.read_link() {
                            let name = dst
                                .file_name()
                                .expect("Failed to get filename")
                                .to_string_lossy();

                            let generation_regex = Regex::new(r"^(.*)-(\d+)-link$").unwrap();

                            if generation_regex.captures(&name).is_some() {
                                res.push(path);
                            }
                        }
                    }
                    Err(error) => {
                        warn!(?dir, ?error, "Failed to read folder element");
                    }
                }
            }
        }
        Err(error) => {
            warn!(?dir, ?error, "Failed to read profiles directory");
        }
    }

    res
}

#[instrument(err, level = "debug")]
fn cleanable_generations(
    profile: &Path,
    keep: u32,
    keep_since: humantime::Duration,
) -> Result<GenerationsTagged> {
    let name = profile
        .file_name()
        .context("Checking profile's name")?
        .to_str()
        .unwrap();

    let generation_regex = Regex::new(&format!(r"^{name}-(\d+)-link"))?;

    let mut result = GenerationsTagged::new();

    for entry in profile
        .parent()
        .context("Reading profile's parent dir")?
        .read_dir()
        .context("Reading profile's generations")?
    {
        let path = entry?.path();
        let captures = generation_regex.captures(path.file_name().unwrap().to_str().unwrap());

        if let Some(caps) = captures {
            if let Some(number) = caps.get(1) {
                let last_modified = path
                    .symlink_metadata()
                    .context("Checking symlink metadata")?
                    .modified()
                    .context("Reading modified time")?;

                result.insert(
                    Generation {
                        number: number.as_str().parse().unwrap(),
                        last_modified,
                        path: path.clone(),
                    },
                    true,
                );
            }
        }
    }

    let now = SystemTime::now();
    for (gen, tbr) in result.iter_mut() {
        match now.duration_since(gen.last_modified) {
            Err(err) => {
                warn!(?err, ?now, ?gen, "Failed to compare time!");
            }
            Ok(val) if val <= keep_since.into() => {
                *tbr = false;
            }
            Ok(_) => {}
        }
    }

    for (_, tbr) in result.iter_mut().rev().take(keep as _) {
        *tbr = false;
    }

    debug!("{:#?}", result);
    Ok(result)
}

fn remove_path_nofail(path: &Path) {
    info!("Removing {}", path.to_string_lossy());
    if let Err(err) = std::fs::remove_file(path) {
        warn!(?path, ?err, "Failed to remove path");
    }
}
