//! `Canvas` (PROTOCOL.md section 20): the client draws into it with a list of commands
//! (`rect`, `line`, `path`) in its `commands` prop; `pointerDown` reports local coordinates.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::theme::Theme;
use gpui_kit::*;
use serde_json::{Map, Value, json};

use super::chart::{COLORS, color};
use super::emit;
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree};

/// A stroked path segment, in coordinates local to the canvas.
#[derive(Debug, PartialEq)]
enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Close,
}

#[derive(Debug, PartialEq)]
enum Cmd {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: String,
    },
    /// `line` and `path`: both are strokes.
    Stroke {
        segs: Vec<Seg>,
        width: f32,
        color: String,
    },
}

/// Validates the `commands` prop (the error is wrapped by the schema).
pub fn check_commands(value: &Value) -> Result<(), String> {
    parse(value).map(drop)
}

fn parse(value: &Value) -> Result<Vec<Cmd>, String> {
    let list = value.as_array().ok_or("not a list")?;
    list.iter()
        .enumerate()
        .map(|(ix, cmd)| parse_cmd(cmd).map_err(|err| format!("command {ix}: {err}")))
        .collect()
}

fn parse_cmd(cmd: &Value) -> Result<Cmd, String> {
    let obj = cmd.as_object().ok_or("not an object")?;
    let color = match obj.get("color") {
        None => "chart-1".to_owned(),
        Some(Value::String(name)) if COLORS.contains(&name.as_str()) => name.clone(),
        Some(_) => return Err(format!("'color' must be one of {}", COLORS.join(", "))),
    };
    let width = || not_negative(num(obj, "width")?.unwrap_or(1.), "width");
    match obj.get("op").and_then(Value::as_str) {
        Some("rect") => Ok(Cmd::Rect {
            x: req(obj, "x")?,
            y: req(obj, "y")?,
            w: not_negative(req(obj, "w")?, "w")?,
            h: not_negative(req(obj, "h")?, "h")?,
            radius: not_negative(num(obj, "corner_radius")?.unwrap_or(0.), "corner_radius")?,
            color,
        }),
        Some("line") => Ok(Cmd::Stroke {
            segs: vec![
                Seg::Move(req(obj, "x1")?, req(obj, "y1")?),
                Seg::Line(req(obj, "x2")?, req(obj, "y2")?),
            ],
            width: width()?,
            color,
        }),
        Some("path") => {
            let d = obj.get("d").and_then(Value::as_str).ok_or("missing 'd'")?;
            Ok(Cmd::Stroke {
                segs: parse_d(d)?,
                width: width()?,
                color,
            })
        }
        Some(op) => Err(format!("unknown op '{op}' (rect, line, path)")),
        None => Err("missing 'op'".into()),
    }
}

/// An optional number field; present but not a finite `f32` (`1e300` overflows) is an error.
fn num(obj: &Map<String, Value>, key: &str) -> Result<Option<f32>, String> {
    obj.get(key)
        .map(|v| {
            v.as_f64()
                .map(|n| n as f32)
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("'{key}' must be a number"))
        })
        .transpose()
}

/// Sizes and widths: a negative one would reach GPUI's tessellator as nonsense.
fn not_negative(n: f32, key: &str) -> Result<f32, String> {
    if n >= 0. {
        Ok(n)
    } else {
        Err(format!("'{key}' cannot be negative"))
    }
}

fn req(obj: &Map<String, Value>, key: &str) -> Result<f32, String> {
    num(obj, key)?.ok_or_else(|| format!("missing '{key}'"))
}

/// The subset of SVG path data a canvas takes: absolute `M x y`, `L x y` and `Z`.
fn parse_d(d: &str) -> Result<Vec<Seg>, String> {
    // Split the commands off their numbers, so "M10,10L90 10" tokenizes like "M 10 10 L 90 10".
    let spaced = d
        .replace(',', " ")
        .replace('M', " M ")
        .replace('L', " L ")
        .replace('Z', " Z ");
    let mut tokens = spaced.split_whitespace();
    let mut segs = Vec::new();
    while let Some(token) = tokens.next() {
        let mut coord = || {
            tokens
                .next()
                .and_then(|t| t.parse::<f32>().ok())
                .filter(|n| n.is_finite())
        };
        match token {
            "M" | "L" => {
                let (Some(x), Some(y)) = (coord(), coord()) else {
                    return Err(format!("'{token}' needs two numbers in 'd'"));
                };
                if token == "L" && segs.is_empty() {
                    return Err("'d' must start with 'M'".into());
                }
                segs.push(if token == "M" {
                    Seg::Move(x, y)
                } else {
                    Seg::Line(x, y)
                });
            }
            "Z" if !segs.is_empty() => segs.push(Seg::Close),
            _ => return Err(format!("unexpected '{token}' in 'd' (only M, L and Z)")),
        }
    }
    Ok(segs)
}

/// The user pressed the left button on the canvas, at `x`/`y` local to it.
pub fn pointer_down(tree: &Tree, id: NodeId, x: f64, y: f64) {
    emit(tree, id, "pointerDown", Some(json!({"x": x, "y": y})), None);
}

fn paint(cmds: &[(Cmd, Hsla)], origin: Point<Pixels>, window: &mut Window) {
    let at = |x: f32, y: f32| origin + point(px(x), px(y));
    for (cmd, color) in cmds {
        match cmd {
            Cmd::Rect {
                x, y, w, h, radius, ..
            } => {
                let bounds = Bounds::new(at(*x, *y), size(px(*w), px(*h)));
                window.paint_quad(fill(bounds, *color).corner_radii(px(*radius)));
            }
            Cmd::Stroke { segs, width, .. } => {
                let mut builder = PathBuilder::stroke(px(*width));
                for seg in segs {
                    match *seg {
                        Seg::Move(x, y) => builder.move_to(at(x, y)),
                        Seg::Line(x, y) => builder.line_to(at(x, y)),
                        Seg::Close => builder.close(),
                    }
                }
                // An empty path has nothing to paint.
                if let Ok(path) = builder.build() {
                    window.paint_path(path, *color);
                }
            }
        }
    }
}

/// Props: `commands`. Events: `pointerDown` (`value` is `{x, y}`, local to the canvas).
pub fn build(id: NodeId, node: &Node, theme: &Theme, session: &Entity<Session>) -> AnyElement {
    // Already checked by the schema when it was set.
    let cmds: Vec<(Cmd, Hsla)> = node
        .props
        .get("commands")
        .and_then(|value| parse(value).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|cmd| {
            let name = match &cmd {
                Cmd::Rect { color, .. } | Cmd::Stroke { color, .. } => color,
            };
            let hsla = color(theme, name);
            (cmd, hsla)
        })
        .collect();
    // Where the canvas was laid out, to turn the window positions of the mouse into local ones.
    let origin = Rc::new(Cell::new(Point::default()));
    let laid_out = origin.clone();
    let canvas = canvas(
        move |bounds, _, _| laid_out.set(bounds.origin),
        // Commands outside the canvas must not paint over its siblings.
        move |bounds, (), window, _| {
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                paint(&cmds, bounds.origin, window)
            })
        },
    )
    .size_full();
    let mut element = div().size_full().child(canvas);
    if node.events.contains("pointerDown") {
        let session = session.clone();
        element = element.on_mouse_down(MouseButton::Left, move |event, _, cx| {
            let local = event.position - origin.get();
            let (x, y) = (f32::from(local.x), f32::from(local.y));
            pointer_down(&session.read(cx).tree, id, x.into(), y.into());
        });
    }
    element.into_any_element()
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{Cmd, Seg, check_commands, parse, parse_d};
    use serde_json::json;

    #[test]
    fn commands_are_checked_one_by_one() {
        assert!(check_commands(&json!([])).is_ok());
        assert!(
            check_commands(&json!([
                {"op": "rect", "x": 0, "y": 0, "w": 10, "h": 5, "color": "primary", "corner_radius": 2},
                {"op": "line", "x1": 0, "y1": 0, "x2": 9, "y2": 9, "width": 2},
                {"op": "path", "d": "M10 10 L90 10 L90 90 Z", "color": "chart-2"}
            ]))
            .is_ok()
        );
        for bad in [
            json!({}),
            json!([1]),
            json!([{"x": 0}]),
            json!([{"op": "text", "x": 0, "y": 0}]),
            json!([{"op": "rect", "x": 0, "y": 0, "w": 10}]),
            json!([{"op": "rect", "x": "0", "y": 0, "w": 10, "h": 5}]),
            json!([{"op": "line", "x1": 0, "y1": 0, "x2": 9, "y2": 9, "color": "pink"}]),
            json!([{"op": "line", "x1": 0, "y1": 0, "x2": 9, "y2": 9, "width": "2"}]),
            json!([{"op": "path"}]),
            json!([{"op": "rect", "x": 1e300, "y": 0, "w": 10, "h": 5}]),
            json!([{"op": "rect", "x": 0, "y": 0, "w": -10, "h": 5}]),
            json!([{"op": "rect", "x": 0, "y": 0, "w": 10, "h": -5}]),
            json!([{"op": "rect", "x": 0, "y": 0, "w": 10, "h": 5, "corner_radius": -1}]),
            json!([{"op": "line", "x1": 0, "y1": 0, "x2": 9, "y2": 9, "width": -2}]),
            json!([{"op": "path", "d": "M0 0 L1e300 0"}]),
        ] {
            assert!(check_commands(&bad).is_err(), "{bad}");
        }
        let err = check_commands(&json!([{"op": "rect"}, {"op": "oval"}])).unwrap_err();
        assert_eq!(err, "command 0: missing 'x'");
    }

    #[test]
    fn defaults_fill_the_optional_fields() {
        let cmds = parse(&json!([{"op": "rect", "x": 1, "y": 2, "w": 3, "h": 4}])).unwrap();
        assert_eq!(
            cmds,
            [Cmd::Rect {
                x: 1.,
                y: 2.,
                w: 3.,
                h: 4.,
                radius: 0.,
                color: "chart-1".into()
            }]
        );
    }

    #[test]
    fn path_data_takes_m_l_and_z() {
        let segs = [Seg::Move(10., 10.), Seg::Line(90., 10.5), Seg::Close];
        assert_eq!(parse_d("M10 10 L90 10.5 Z").unwrap(), segs);
        assert_eq!(parse_d("M10,10L90,10.5Z").unwrap(), segs);
        assert_eq!(parse_d("").unwrap(), []);
        for bad in ["L1 1", "M1", "M1 x", "m1 1", "M1 1 C2 2", "Z", "M1 1 2 2"] {
            assert!(parse_d(bad).is_err(), "{bad}");
        }
    }
}
