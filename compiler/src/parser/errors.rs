use ::std::any::type_name;
use ::std::cmp::max;
use ::std::fmt;
use ::std::fmt::Write;

use ::lalrpop_util::ParseError;

use ::steel_api::log::info;

/// Returns error message and error line (or 0 if unknown)
pub fn build_error<T, E: fmt::Display>(
    error: ParseError<usize, T, E>,
    src_file: &str,
    code: &str,
) -> (String, usize) {
    let msg = match error {
        ParseError::InvalidToken { location } => {
            let (line, col) = source_line_col(code, location);
            (
                format!(
                    "Invalid code in {}:{}:{}\n{}",
                    src_file,
                    line + 1,
                    col + 1,
                    source_loc_repr(code, line, col, 1)
                ),
                line,
            )
        }
        ParseError::UnrecognizedEof { location, expected } => {
            let (line, col) = source_line_col(code, location);
            (
                format!(
                    "Unexpected end in {}:{}:{}\n{}{}",
                    src_file,
                    line + 1,
                    col + 1,
                    source_loc_repr(code, line, col, 1),
                    fmt_expected_tokens(&expected)
                ),
                line,
            )
        }
        ParseError::UnrecognizedToken {
            token: (start, encountered_type, end),
            expected,
        } => {
            let (line, col) = source_line_col(code, start);
            let found = &code[start..end];
            (
                format!(
                    "Did not expect '{found}' in {src_file}:{}:{}\n{}{}",
                    line + 1,
                    col + 1,
                    source_loc_repr(code, line, col, max(1, end - start)),
                    fmt_expected_tokens(&expected)
                ),
                line,
            )
        }
        ParseError::ExtraToken {
            token: (start, _, end),
        } => {
            let (line, col) = source_line_col(code, start);
            let found = &code[start..end];
            (
                format!(
                    "Invalid token '{found}' in {src_file}:{}:{}\n{}",
                    line + 1,
                    col + 1,
                    source_loc_repr(code, line, col, max(1, end - start))
                ),
                line,
            )
        }
        ParseError::User { error } => (format!("Error in {}: {}", src_file, error), 0),
    };
    info!("{}", &msg.0);
    msg
}

fn fmt_expected_tokens(tokens: &[String]) -> String {
    if tokens.is_empty() {
        "Do not know what to expect at this position".to_owned()
    } else if tokens.len() == 1 {
        format!("Expected: {}", tokens[0])
    } else {
        format!("Expected one of: {}", tokens.join(", "))
    }
}

fn source_line_col(code: &str, start: usize) -> (usize, usize) {
    let mut err_line_nr = 0;
    let mut err_char_in_line = 0;
    let mut char_nr = 0;
    for line in code.lines() {
        if char_nr + line.len() >= start {
            err_char_in_line = start - char_nr;
            break;
        }
        char_nr += line.len() + 1;
        err_line_nr += 1;
    }
    (err_line_nr, err_char_in_line)
}

fn source_loc_repr(code: &str, err_line: usize, err_col: usize, len: usize) -> String {
    let err_line_ix = if err_line > 0 { err_line - 1 } else { err_line };
    assert!(len >= 1);
    let mut locator = String::with_capacity(160);
    for (line_nr, line) in code.lines().enumerate() {
        if line_nr + 2 > err_line_ix {
            writeln!(locator, "{:3} | {}", line_nr + 1, line).unwrap();
        }
        if line_nr == err_line_ix + 1 {
            let end_loc = if len > 1 {
                format!("-{}", err_col + len)
            } else {
                "".to_owned()
            };
            writeln!(
                locator,
                "      {}{} {}{}",
                " ".repeat(err_col),
                "^".repeat(len),
                err_col + 1,
                end_loc
            )
            .unwrap();
        }
        if line_nr > err_line_ix + 2 {
            break;
        }
    }
    locator
}
