use crate::installable::{attribute_path_parser, parse_attribute_robust};
use chumsky::prelude::*;

#[test]
fn test_attribute_path_parser() {
    // Test single segment
    assert_eq!(attribute_path_parser().parse("foo").unwrap(), vec!["foo"]);
    
    // Test multiple segments
    assert_eq!(attribute_path_parser().parse("foo.bar").unwrap(), vec!["foo", "bar"]);
    
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
    assert!(attribute_path_parser().parse(".foo").is_err());
    assert!(attribute_path_parser().parse("foo.").is_err());
    assert!(attribute_path_parser().parse("foo..bar").is_err());
}

// Test the public parse_attribute_robust function with various inputs
#[test]
fn test_parse_attribute_robust_comprehensive() {
    // Test basic attribute paths
    assert_eq!(parse_attribute_robust("foo").unwrap(), vec!["foo"]);
    assert_eq!(parse_attribute_robust("foo.bar").unwrap(), vec!["foo", "bar"]);
    assert_eq!(parse_attribute_robust("foo.bar.baz").unwrap(), vec!["foo", "bar", "baz"]);
    
    // Test with quoted segments containing dots
    assert_eq!(parse_attribute_robust("foo.\"bar.baz\"").unwrap(), vec!["foo", "bar.baz"]);
    assert_eq!(parse_attribute_robust("\"foo.bar\".baz").unwrap(), vec!["foo.bar", "baz"]);
    assert_eq!(parse_attribute_robust("\"foo.bar.baz\"").unwrap(), vec!["foo.bar.baz"]);
    
    // Test with quoted segments containing spaces and special characters
    assert_eq!(parse_attribute_robust("foo.\"bar baz\"").unwrap(), vec!["foo", "bar baz"]);
    assert_eq!(parse_attribute_robust("\"foo bar\".baz").unwrap(), vec!["foo bar", "baz"]);
    assert_eq!(parse_attribute_robust("foo.\"bar\\\"baz\"").unwrap(), vec!["foo", "bar\"baz"]);
    
    // Test with valid identifiers that include special characters
    assert_eq!(parse_attribute_robust("_foo").unwrap(), vec!["_foo"]);
    assert_eq!(parse_attribute_robust("foo_bar").unwrap(), vec!["foo_bar"]);
    assert_eq!(parse_attribute_robust("foo-bar").unwrap(), vec!["foo-bar"]);
    assert_eq!(parse_attribute_robust("foo'bar").unwrap(), vec!["foo'bar"]);
    
    // Test empty string
    assert_eq!(parse_attribute_robust("").unwrap(), Vec::<String>::new());
    
    // Test error cases
    assert!(parse_attribute_robust(".foo").is_err());
    assert!(parse_attribute_robust("foo.").is_err());
    assert!(parse_attribute_robust("foo..bar").is_err());
    assert!(parse_attribute_robust("1foo").is_err());
    assert!(parse_attribute_robust("foo.1bar").is_err());
    assert!(parse_attribute_robust("\"unclosed").is_err());
}

// Test the public attribute_path_parser function directly
#[test]
fn test_attribute_path_parser_direct() {
    // Test single segment
    assert_eq!(attribute_path_parser().parse("foo").unwrap(), vec!["foo"]);
    
    // Test multiple segments
    assert_eq!(attribute_path_parser().parse("foo.bar").unwrap(), vec!["foo", "bar"]);
    
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
    assert!(attribute_path_parser().parse(".foo").is_err());
    assert!(attribute_path_parser().parse("foo.").is_err());
    assert!(attribute_path_parser().parse("foo..bar").is_err());
}

#[test]
fn test_parse_attribute_robust() {
    // Test valid attribute paths
    assert_eq!(parse_attribute_robust("foo.bar").unwrap(), vec!["foo", "bar"]);
    assert_eq!(parse_attribute_robust("foo.\"bar.baz\"").unwrap(), vec!["foo", "bar.baz"]);
    assert_eq!(parse_attribute_robust("\"foo.bar\".baz").unwrap(), vec!["foo.bar", "baz"]);
    assert_eq!(parse_attribute_robust("foo").unwrap(), vec!["foo"]);
    assert_eq!(parse_attribute_robust("\"foo\"").unwrap(), vec!["foo"]);
    assert_eq!(parse_attribute_robust("").unwrap(), Vec::<String>::new());
    
    // Test error reporting for invalid paths
    let err_result = parse_attribute_robust("foo.");
    assert!(err_result.is_err());
    
    let err_result = parse_attribute_robust(".foo");
    assert!(err_result.is_err());
    
    let err_result = parse_attribute_robust("foo..bar");
    assert!(err_result.is_err());
    
    let err_result = parse_attribute_robust("1foo");
    assert!(err_result.is_err());
}