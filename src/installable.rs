use std::path::PathBuf;
use std::{env, fs};

use clap::error::ErrorKind;
use clap::{Arg, ArgAction, Args, FromArgMatches};
use color_eyre::owo_colors::OwoColorize;
use chumsky::prelude::*;
use tracing::warn;

// Reference: https://nix.dev/manual/nix/2.18/command-ref/new-cli/nix

#[derive(Debug, Clone)]
pub enum Installable {
    Flake {
        reference: String,
        attribute: Vec<String>,
    },
    File {
        path: PathBuf,
        attribute: Vec<String>,
    },
    Store {
        path: PathBuf,
    },
    Expression {
        expression: String,
        attribute: Vec<String>,
    },
}

impl FromArgMatches for Installable {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        let mut matches = matches.clone();
        Self::from_arg_matches_mut(&mut matches)
    }

    fn from_arg_matches_mut(matches: &mut clap::ArgMatches) -> Result<Self, clap::Error> {
        let installable = matches.get_one::<String>("installable");
        let file = matches.get_one::<String>("file");
        let expr = matches.get_one::<String>("expr");
        
        // Robust attribute parsing helper
        fn parse_attr_for_installable<S: AsRef<str>>(attr_str: S) -> Vec<String> {
            let attr_s = attr_str.as_ref();
            parse_attribute_robust(attr_s).unwrap_or_else(|parse_err| { // parse_err is Simple<char>
                // Convert chumsky Simple error to a string
                let err_msg = parse_err.to_string();
                warn!("Failed to robustly parse attribute '{}': {}. Falling back to simple parsing.", attr_s, err_msg);
                // Fallback to the old simple parse_attribute
                parse_attribute(attr_s)
            })
        }

        if let Some(i) = installable {
            let canonincal = fs::canonicalize(i);

            if let Ok(p) = canonincal {
                if p.starts_with("/nix/store") {
                    return Ok(Self::Store { path: p });
                }
            }
        }

        if let Some(f) = file {
            return Ok(Self::File {
                path: PathBuf::from(f),
                attribute: parse_attr_for_installable(installable.cloned().unwrap_or_default()),
            });
        }

        if let Some(e) = expr {
            return Ok(Self::Expression {
                expression: e.to_string(),
                attribute: parse_attr_for_installable(installable.cloned().unwrap_or_default()),
            });
        }

        if let Some(i) = installable {
            let mut elems = i.splitn(2, '#');
            let reference = elems.next().unwrap().to_owned();
            return Ok(Self::Flake {
                reference,
                attribute: parse_attr_for_installable(elems.next().map(|s| s.to_string()).unwrap_or_default()),
            });
        }

        // env var fallbacks

        // Check for command-specific flake env vars first
        if let Ok(subcommand) = env::var("NH_CURRENT_COMMAND") {
            if subcommand == "os" {
                if let Ok(f) = env::var("NH_OS_FLAKE") {
                    let mut elems = f.splitn(2, '#');
                    return Ok(Self::Flake {
                        reference: elems.next().unwrap().to_owned(),
                        attribute: parse_attr_for_installable(
                            elems.next().map(|s| s.to_string()).unwrap_or_default(),
                        ),
                    });
                }
            } else if subcommand == "home" {
                if let Ok(f) = env::var("NH_HOME_FLAKE") {
                    let mut elems = f.splitn(2, '#');
                    return Ok(Self::Flake {
                        reference: elems.next().unwrap().to_owned(),
                        attribute: parse_attr_for_installable(
                            elems.next().map(|s| s.to_string()).unwrap_or_default(),
                        ),
                    });
                }
            } else if subcommand == "darwin" {
                if let Ok(f) = env::var("NH_DARWIN_FLAKE") {
                    let mut elems = f.splitn(2, '#');
                    return Ok(Self::Flake {
                        reference: elems.next().unwrap().to_owned(),
                        attribute: parse_attr_for_installable(
                            elems.next().map(|s| s.to_string()).unwrap_or_default(),
                        ),
                    });
                }
            }
        }

        if let Ok(f) = env::var("NH_FLAKE") {
            let mut elems = f.splitn(2, '#');
            return Ok(Self::Flake {
                reference: elems.next().unwrap().to_owned(),
                attribute: parse_attr_for_installable(elems.next().map(|s| s.to_string()).unwrap_or_default()),
            });
        }

        if let Ok(f) = env::var("NH_OS_FLAKE") {
            let mut elems = f.splitn(2, '#');
            return Ok(Self::Flake {
                reference: elems.next().unwrap().to_owned(),
                attribute: parse_attr_for_installable(elems.next().map(|s| s.to_string()).unwrap_or_default()),
            });
        }

        if let Ok(f) = env::var("NH_HOME_FLAKE") {
            let mut elems = f.splitn(2, '#');
            return Ok(Self::Flake {
                reference: elems.next().unwrap().to_owned(),
                attribute: parse_attr_for_installable(elems.next().map(|s| s.to_string()).unwrap_or_default()),
            });
        }

        if let Ok(f) = env::var("NH_DARWIN_FLAKE") {
            let mut elems = f.splitn(2, '#');
            return Ok(Self::Flake {
                reference: elems.next().unwrap().to_owned(),
                attribute: parse_attr_for_installable(elems.next().map(|s| s.to_string()).unwrap_or_default()),
            });
        }

        if let Ok(f) = env::var("NH_FILE") {
            return Ok(Self::File {
                path: PathBuf::from(f),
                attribute: parse_attr_for_installable(env::var("NH_ATTRP").unwrap_or_default()),
            });
        }

        Err(clap::Error::new(ErrorKind::TooFewValues))
    }

    fn update_from_arg_matches(&mut self, _matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        todo!()
    }
}

impl Args for Installable {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .action(ArgAction::Set)
                .hide(true),
        )
        .arg(
            Arg::new("expr")
                .short('E')
                .long("expr")
                .conflicts_with("file")
                .hide(true)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("installable")
                .action(ArgAction::Set)
                .value_name("INSTALLABLE")
                .help("Which installable to use")
                .long_help(format!(
                    r#"Which installable to use.
Nix accepts various kinds of installables:

[FLAKEREF[#ATTRPATH]]
    Flake reference with an optional attribute path.
    [env: NH_FLAKE={}]
    [env: NH_OS_FLAKE={}]
    [env: NH_HOME_FLAKE={}]
    [env: NH_DARWIN_FLAKE={}]

{}, {} <FILE> [ATTRPATH]
    Path to file with an optional attribute path.
    [env: NH_FILE={}]
    [env: NH_ATTRP={}]

{}, {} <EXPR> [ATTRPATH]
    Nix expression with an optional attribute path.

[PATH]
    Path or symlink to a /nix/store path
"#,
                    env::var("NH_FLAKE").unwrap_or_default(),
                    env::var("NH_OS_FLAKE").unwrap_or_default(),
                    env::var("NH_HOME_FLAKE").unwrap_or_default(),
                    env::var("NH_DARWIN_FLAKE").unwrap_or_default(),
                    "-f".yellow(),
                    "--file".yellow(),
                    env::var("NH_FILE").unwrap_or_default(),
                    env::var("NH_ATTR").unwrap_or_default(),
                    "-e".yellow(),
                    "--expr".yellow(),
                )),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

// --- Chumsky Parsers for Attribute Paths ---
fn nix_identifier_char_first() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
}

fn nix_identifier_char_rest() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '\'' || *c == '-')
}

fn nix_identifier() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier_char_first()
        .chain(nix_identifier_char_rest().repeated())
        .collect()
}

fn quoted_string_content() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| *c != '"' && *c != '\\').or(just('\\').ignore_then(any()))
}

fn quoted_string() -> impl Parser<char, String, Error = Simple<char>> {
    just('"')
        .ignore_then(quoted_string_content().repeated().collect())
        .then_ignore(just('"'))
}

fn attribute_segment() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier().or(quoted_string())
}

pub fn attribute_path_parser() -> impl Parser<char, Vec<String>, Error = Simple<char>> {
    // Non-empty path: must start with a segment, then dot-separated segments
    let non_empty_path = attribute_segment()
        .separated_by(just('.'))
        .at_least(1) // Must have at least one segment
        .collect::<Vec<String>>();

    // Allow non_empty_path OR an empty parser that yields an empty Vec for genuinely empty input
    non_empty_path.or(empty().map(|_| Vec::new()))
}

// New robust parse_attribute function using chumsky
pub fn parse_attribute_robust<S>(s: S) -> Result<Vec<String>, Simple<char>> // Error type changed
where
    S: AsRef<str>,
{
    let s_ref = s.as_ref();
    // The new attribute_path_parser combined with then_ignore(end()) handles empty string correctly.
    attribute_path_parser()
        .then_ignore(end()) // Ensures the whole string is consumed
        .parse(s_ref)
        .map_err(|e_vec| { // Chumsky parse returns Vec<Error>, map to a single Simple error
            // For simplicity, take the first error message if available, or a generic one.
            let message = e_vec.first().map(|e| e.to_string()).unwrap_or_else(|| format!("Invalid attribute path: '{}'", s_ref));
            Simple::custom(Default::default(), message)
        })
}

// Keep the old parse_attribute for backward compatibility and fallback
pub fn parse_attribute<S>(s: S) -> Vec<String>
where
    S: AsRef<str>,
{
    let s = s.as_ref();
    let mut res = Vec::new();

    if s.is_empty() {
        return res;
    }

    let mut in_quote = false;

    let mut elem = String::new();
    for char in s.chars() {
        match char {
            '.' => {
                if !in_quote {
                    res.push(elem.clone());
                    elem = String::new();
                } else {
                    elem.push(char);
                }
            }
            '"' => {
                in_quote = !in_quote;
            }
            _ => elem.push(char),
        }
    }

    res.push(elem);

    if in_quote {
        panic!("Failed to parse attribute: {}", s);
    }

    res
}

#[test]
fn test_parse_attribute() {
    assert_eq!(parse_attribute(r#"foo.bar"#), vec!["foo", "bar"]);
    assert_eq!(parse_attribute(r#"foo."bar.baz""#), vec!["foo", "bar.baz"]);
    let v: Vec<String> = vec![];
    assert_eq!(parse_attribute(""), v)
}

#[test]
fn test_parse_attribute_robust() {
    assert_eq!(parse_attribute_robust("foo.bar").unwrap(), vec!["foo", "bar"]);
    assert_eq!(parse_attribute_robust("foo.\"bar.baz\"").unwrap(), vec!["foo", "bar.baz"]);
    assert_eq!(parse_attribute_robust("\"foo.bar\".baz").unwrap(), vec!["foo.bar", "baz"]);
    assert_eq!(parse_attribute_robust("foo").unwrap(), vec!["foo"]);
    assert_eq!(parse_attribute_robust("\"foo\"").unwrap(), vec!["foo"]);
    assert_eq!(parse_attribute_robust("").unwrap(), Vec::<String>::new());
}

#[test]
fn test_nix_identifier() {
    // Test valid identifiers
    assert_eq!(nix_identifier().parse("a").unwrap(), "a");
    assert_eq!(nix_identifier().parse("a_b").unwrap(), "a_b");
    assert_eq!(nix_identifier().parse("_a").unwrap(), "_a");
    assert_eq!(nix_identifier().parse("a-b").unwrap(), "a-b");
    assert_eq!(nix_identifier().parse("a'b").unwrap(), "a'b");
    assert_eq!(nix_identifier().parse("abc123").unwrap(), "abc123");
    
    // Test invalid identifiers
    assert!(nix_identifier().parse("1a").is_err());
    assert!(nix_identifier().parse(".a").is_err());
    assert!(nix_identifier().then_ignore(end()).parse("a.b").is_err());
    assert!(nix_identifier().parse("").is_err());
}

#[test]
fn test_quoted_string() {
    // Test simple strings
    assert_eq!(quoted_string().parse("\"foo\"").unwrap(), "foo");
    
    // Test strings with spaces
    assert_eq!(quoted_string().parse("\"foo bar\"").unwrap(), "foo bar");
    
    // Test strings with dots
    assert_eq!(quoted_string().parse("\"foo.bar\"").unwrap(), "foo.bar");
    
    // Test strings with escaped quotes
    assert_eq!(quoted_string().parse("\"foo\\\"bar\"").unwrap(), "foo\"bar");
    
    // Test strings with other escaped characters
    assert_eq!(quoted_string().parse("\"foo\\\\bar\"").unwrap(), "foo\\bar");
    
    // Test invalid quoted strings
    assert!(quoted_string().parse("foo").is_err());
    assert!(quoted_string().parse("\"foo").is_err());
    assert!(quoted_string().parse("foo\"").is_err());
    assert!(quoted_string().parse("").is_err());
}

#[test]
fn test_attribute_segment() {
    // Test with identifiers
    assert_eq!(attribute_segment().parse("foo").unwrap(), "foo");
    assert_eq!(attribute_segment().parse("_foo").unwrap(), "_foo");
    assert_eq!(attribute_segment().parse("foo_bar").unwrap(), "foo_bar");
    assert_eq!(attribute_segment().parse("foo-bar").unwrap(), "foo-bar");
    
    // Test with quoted strings
    assert_eq!(attribute_segment().parse("\"foo\"").unwrap(), "foo");
    assert_eq!(attribute_segment().parse("\"foo.bar\"").unwrap(), "foo.bar");
    assert_eq!(attribute_segment().parse("\"foo bar\"").unwrap(), "foo bar");
    
    // Test invalid segments
    assert!(attribute_segment().parse("").is_err());
    assert!(attribute_segment().parse("1foo").is_err());
    assert!(attribute_segment().then_ignore(end()).parse("foo.bar").is_err());
}

#[test]
fn test_attribute_path_parser_comprehensive() {
    // Test single segment
    assert_eq!(attribute_path_parser().parse("foo").unwrap(), vec!["foo"]);
    
    // Test multiple segments
    assert_eq!(attribute_path_parser().parse("foo.bar").unwrap(), vec!["foo", "bar"]);
    assert_eq!(attribute_path_parser().parse("foo.bar.baz").unwrap(), vec!["foo", "bar", "baz"]);
    
    // Test with quoted segments containing dots
    assert_eq!(
        attribute_path_parser().parse("foo.\"bar.baz\"").unwrap(),
        vec!["foo", "bar.baz"]
    );
    
    // Test with quoted segments at the beginning
    assert_eq!(
        attribute_path_parser().parse("\"foo.bar\".baz").unwrap(),
        vec!["foo.bar", "baz"]
    );
    
    // Test with quoted segments
    assert_eq!(attribute_path_parser().parse("\"foo\"").unwrap(), vec!["foo"]);
    
    // Test empty string
    assert_eq!(attribute_path_parser().parse("").unwrap(), Vec::<String>::new());
    
    // Test invalid paths
    assert!(attribute_path_parser().then_ignore(end()).parse(".foo").is_err());
    assert!(attribute_path_parser().then_ignore(end()).parse("foo.").is_err());
    assert!(attribute_path_parser().then_ignore(end()).parse("foo..bar").is_err());
}

impl Installable {
    pub fn to_args(&self) -> Vec<String> {
        let mut res = Vec::new();
        match self {
            Installable::Flake {
                reference,
                attribute,
            } => {
                res.push(format!("{reference}#{}", join_attribute(attribute)));
            }
            Installable::File { path, attribute } => {
                res.push(String::from("--file"));
                res.push(path.to_str().unwrap().to_string());
                res.push(join_attribute(attribute));
            }
            Installable::Expression {
                expression,
                attribute,
            } => {
                res.push(String::from("--expr"));
                res.push(expression.to_string());
                res.push(join_attribute(attribute));
            }
            Installable::Store { path } => res.push(path.to_str().unwrap().to_string()),
        }

        res
    }
}

#[test]
fn test_installable_to_args() {
    assert_eq!(
        (Installable::Flake {
            reference: String::from("w"),
            attribute: ["x", "y.z"].into_iter().map(str::to_string).collect()
        })
        .to_args(),
        vec![r#"w#x."y.z""#]
    );

    assert_eq!(
        (Installable::File {
            path: PathBuf::from("w"),
            attribute: ["x", "y.z"].into_iter().map(str::to_string).collect()
        })
        .to_args(),
        vec!["--file", "w", r#"x."y.z""#]
    );
}

fn join_attribute<I>(attribute: I) -> String
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut res = String::new();
    let mut first = true;
    for elem in attribute {
        if first {
            first = false;
        } else {
            res.push('.');
        }

        let s = elem.as_ref();

        // Quote the segment if it contains a dot or any character that would make it
        // an invalid Nix identifier
        if s.contains('.') || s.is_empty() || s.chars().next().map_or(false, |c|
            !(c.is_ascii_alphabetic() || c == '_')) ||
            s.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '\'' || c == '-')) {
            res.push_str(&format!(r#""{}""#, s));
        } else {
            res.push_str(s);
        }
    }

    res
}

#[test]
fn test_join_attribute() {
    assert_eq!(join_attribute(vec!["foo", "bar"]), "foo.bar");
    assert_eq!(join_attribute(vec!["foo", "bar.baz"]), r#"foo."bar.baz""#);
}

impl Installable {
    pub fn str_kind(&self) -> &str {
        match self {
            Installable::Flake { .. } => "flake",
            Installable::File { .. } => "file",
            Installable::Store { .. } => "store path",
            Installable::Expression { .. } => "expression",
        }
    }
}
