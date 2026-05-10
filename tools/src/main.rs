use glam::{Mat3, Vec2, Vec3};
use std::env;
use std::fs;
use std::io::{self, Read};

const SVG_TO_BEVY_MATRIX: Mat3 = Mat3::from_cols_array(&[
    0.2, 0.0, 0.0, //
    0.0, -0.2, 0.0, //
    -53.9, 85.6, 1.0,
]);

fn main() {
    let args = match Args::parse(env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            std::process::exit(2);
        }
    };

    let svg = match read_svg(args.input.as_deref()) {
        Ok(svg) => svg,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let points = match extract_polygon_points(&svg, args.polygon_id.as_deref(), args.polygon_index)
    {
        Ok(points) => points,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let transformed = points
        .into_iter()
        .map(|point| transform_point(SVG_TO_BEVY_MATRIX, point))
        .map(|point| [point.x.round() as i32, point.y.round() as i32])
        .collect::<Vec<_>>();

    println!("{}", format_points(&transformed));
}

struct Args {
    input: Option<String>,
    polygon_id: Option<String>,
    polygon_index: usize,
}

impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut input = None;
        let mut polygon_id = None;
        let mut polygon_index = 0;
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                "--id" => {
                    polygon_id = Some(take_value(&mut args, "--id")?);
                }
                "--index" => {
                    polygon_index = take_value(&mut args, "--index")?
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --index: {error}"))?;
                }
                _ if arg.starts_with("--id=") => {
                    polygon_id = Some(arg["--id=".len()..].to_string());
                }
                _ if arg.starts_with("--index=") => {
                    polygon_index = arg["--index=".len()..]
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --index: {error}"))?;
                }
                _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
                _ => {
                    if input.replace(arg).is_some() {
                        return Err("only one input svg path is supported".into());
                    }
                }
            }
        }

        Ok(Self {
            input,
            polygon_id,
            polygon_index,
        })
    }
}

fn take_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn read_svg(path: Option<&str>) -> Result<String, String> {
    match path {
        Some("-") | None => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| format!("failed to read stdin: {error}"))?;
            Ok(input)
        }
        Some(path) => fs::read_to_string(path)
            .map_err(|error| format!("failed to read svg file '{path}': {error}")),
    }
}

fn extract_polygon_points(
    svg: &str,
    polygon_id: Option<&str>,
    polygon_index: usize,
) -> Result<Vec<Vec2>, String> {
    let mut matched_index = 0;
    let mut rest = svg;

    while let Some(start) = rest.find("<polygon") {
        rest = &rest[start..];
        let Some(end) = rest.find('>') else {
            return Err("unterminated <polygon> tag".into());
        };
        let tag = &rest[..=end];
        rest = &rest[end + 1..];

        if let Some(id) = polygon_id {
            if attribute_value(tag, "id").as_deref() != Some(id) {
                continue;
            }
        } else if matched_index != polygon_index {
            matched_index += 1;
            continue;
        }

        let points = attribute_value(tag, "points")
            .ok_or_else(|| "matched <polygon> tag has no points attribute".to_string())?;
        return parse_points(&points);
    }

    Err(match polygon_id {
        Some(id) => format!("no <polygon id=\"{id}\"> found"),
        None => format!("no <polygon> found at index {polygon_index}"),
    })
}

fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let name_start = tag.find(name)?;
    let after_name = tag[name_start + name.len()..].trim_start();
    let after_equal = after_name.strip_prefix('=')?.trim_start();
    let quote = after_equal.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after_quote = &after_equal[quote.len_utf8()..];
    let end = after_quote.find(quote)?;
    Some(after_quote[..end].to_string())
}

fn parse_points(points: &str) -> Result<Vec<Vec2>, String> {
    let values = points
        .split([',', ' ', '\t', '\n', '\r'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>()
                .map_err(|error| format!("invalid point value '{part}': {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if values.len() % 2 != 0 {
        return Err("polygon points must contain an even number of values".into());
    }

    Ok(values
        .chunks_exact(2)
        .map(|pair| Vec2::new(pair[0], pair[1]))
        .collect())
}

fn transform_point(matrix: Mat3, point: Vec2) -> Vec2 {
    matrix.mul_vec3(Vec3::new(point.x, point.y, 1.0)).truncate()
}

fn format_points(points: &[[i32; 2]]) -> String {
    let body = points
        .iter()
        .map(|[x, y]| format!("[{x},{y}]"))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn print_usage() {
    eprintln!(
        "Usage: svg_polygon_points [SVG_PATH|-] [--id POLYGON_ID | --index N]\n\
         \n\
         Applies the built-in SVG-to-Bevy transform.\n\
         If SVG_PATH is omitted or '-', SVG is read from stdin."
    );
}
