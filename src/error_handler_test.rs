use nh::error_handler::{enhance_syntax_error_output, generate_syntax_error_recommendations};

#[test]
fn test_generate_syntax_error_recommendations() {
    // Test case 1: Missing closing brace
    let error_details = r#"Error in ./test/syntax_error.nix: 
evaluating file '<nix/derivation-internal.nix>'
error: syntax error, unexpected end of file, expecting INHERIT
       at /home/user/test/syntax_error.nix:4:17:
                3|   foo = {
                4|     bar = "baz";
                 |                 ^"#;
    
    let recommendations = generate_syntax_error_recommendations(error_details);
    assert!(recommendations.contains(&"Add missing closing brace '}' to complete the attribute set".to_string()));
    
    // Test case 2: Missing closing bracket
    let error_details = r#"Error in ./test/syntax_error.nix: 
evaluating file '<nix/derivation-internal.nix>'
error: syntax error, unexpected end of file, expecting ]
       at /home/user/test/syntax_error.nix:4:17:
                3|   foo = [
                4|     "bar"
                 |          ^"#;
    
    let recommendations = generate_syntax_error_recommendations(error_details);
    assert!(recommendations.contains(&"Add missing closing bracket ']' to complete the list".to_string()));
    
    // Test case 3: Unexpected semicolon
    let error_details = r#"Error in ./test/syntax_error.nix: 
evaluating file '<nix/derivation-internal.nix>'
error: syntax error, unexpected ;
       at /home/user/test/syntax_error.nix:4:17:
                3|   foo = {
                4|     bar = "baz";;
                 |                 ^"#;
    
    let recommendations = generate_syntax_error_recommendations(error_details);
    assert!(recommendations.contains(&"Remove extra semicolon ';' or add an expression after it".to_string()));
    
    // Test case 4: Generic error
    let error_details = r#"Error in ./test/syntax_error.nix: 
evaluating file '<nix/derivation-internal.nix>'
error: syntax error, unexpected token
       at /home/user/test/syntax_error.nix:4:17:
                3|   foo = {
                4|     bar = @
                 |           ^"#;
    
    let recommendations = generate_syntax_error_recommendations(error_details);
    assert!(recommendations.contains(&"Fix the syntax error according to the error message".to_string()));
}

#[test]
fn test_enhance_syntax_error_output() {
    let error_details = r#"Found 1 file(s) with syntax errors:

Error in ./test/syntax_error.nix: 
evaluating file '<nix/derivation-internal.nix>'
error: syntax error, unexpected end of file, expecting INHERIT
       at /home/user/test/syntax_error.nix:4:17:
                3|   foo = {
                4|     bar = "baz";
                 |                 ^"#;
    
    let enhanced = enhance_syntax_error_output(error_details);
    
    // Basic checks - we can't test colors directly, but we can check that the content is preserved
    assert!(enhanced.contains("./test/syntax_error.nix"));
    assert!(enhanced.contains("error: syntax error"));
    assert!(enhanced.contains("at /home/user/test/syntax_error.nix:4:17:"));
    assert!(enhanced.contains("foo = {"));
    assert!(enhanced.contains("bar = \"baz\";"));
}