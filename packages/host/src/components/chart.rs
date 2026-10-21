//! `Chart` (PROTOCOL.md section 19): line, bar and pie charts. The data is a list of
//! `{label, value}` in props, kept parsed in a slot per node: a series of a thousand points is
//! read once per change, not again on every frame.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::chart::{BarChart, LineChart, PieChart};
use gpui_kit::component::plot::shape::BarAlignment;
use gpui_kit::component::theme::Theme;
use gpui_kit::*;
use serde_json::Value;

use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// One datum: its label, its value and its position (the pie cycles the theme's chart colors).
#[derive(Clone, Debug, PartialEq)]
pub struct Datum {
    pub index: usize,
    pub label: SharedString,
    pub value: f64,
}

/// What a `Chart` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Line,
    Bar,
    Pie,
}

impl Kind {
    /// The `chart` prop; anything else is a line chart.
    pub fn of(node: &Node) -> Self {
        match node.props.get("chart").and_then(Value::as_str) {
            Some("bar") => Self::Bar,
            Some("pie") => Self::Pie,
            _ => Self::Line,
        }
    }
}

pub struct ChartSlot {
    data: Vec<Datum>,
}

/// Each Chart among `members`: its parsed data.
pub struct ChartSnapshot(Vec<(NodeId, Vec<Datum>)>);

/// Theme colors a series can take, by the name in the `color` prop.
pub const COLORS: &[&str] = &[
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "primary",
    "secondary",
    "muted",
    "accent",
    "danger",
    "success",
    "warning",
    "info",
];

fn color(theme: &Theme, name: &str) -> Hsla {
    match name {
        "chart-2" => theme.chart_2,
        "chart-3" => theme.chart_3,
        "chart-4" => theme.chart_4,
        "chart-5" => theme.chart_5,
        "primary" => theme.primary,
        "secondary" => theme.secondary,
        "muted" => theme.muted,
        "accent" => theme.accent,
        "danger" => theme.danger,
        "success" => theme.success,
        "warning" => theme.warning,
        "info" => theme.info,
        _ => theme.chart_1,
    }
}

/// The five chart colors of the theme, for the slices of a pie.
fn palette(theme: &Theme) -> [Hsla; 5] {
    [
        theme.chart_1,
        theme.chart_2,
        theme.chart_3,
        theme.chart_4,
        theme.chart_5,
    ]
}

/// Checks `data`: a list of `{label, value}`, the value a finite number.
pub fn check_data(value: &Value) -> Result<(), String> {
    let data = value.as_array().ok_or("a list of data points")?;
    for (ix, datum) in data.iter().enumerate() {
        let label = datum.get("label").is_some_and(Value::is_string);
        let value = datum
            .get("value")
            .and_then(Value::as_f64)
            .is_some_and(f64::is_finite);
        if !label || !value {
            return Err(format!(
                "point {ix} needs a 'label' string and a finite 'value'"
            ));
        }
    }
    Ok(())
}

/// The data a validated `data` prop holds.
fn parse(value: &Value) -> Vec<Datum> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, datum)| Datum {
            index,
            label: datum
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
                .into(),
            value: datum
                .get("value")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
        })
        .collect()
}

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> ChartSnapshot {
    ChartSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Chart && members.contains(id))
            .map(|(id, node)| {
                let data = node.props.get("data").unwrap_or(&Value::Null);
                (id, parse(data))
            })
            .collect(),
    )
}

/// Creates a slot per Chart and reads its data again only when `data` changed.
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, ChartSlot>,
    snapshot: ChartSnapshot,
    _session: &Entity<crate::Session>,
    _cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(chart, _)| chart == id));
    for (id, data) in snapshot.0 {
        match slots.get_mut(&id) {
            Some(slot) => {
                if slot.data != data {
                    slot.data = data;
                }
            }
            None => {
                slots.insert(id, ChartSlot { data });
            }
        }
    }
}

/// Category labels an axis draws unless `xTicks` says otherwise, spread evenly: gpui-component
/// labels every point by default, and a thousand of them overlap into a solid bar.
const X_TICKS: usize = 12;

/// Ticks on the value axis unless `yTicks` says otherwise: gpui-component's own default.
const Y_TICKS: usize = 5;

fn flag(node: &Node, key: &str, default: bool) -> bool {
    node.props
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn number(node: &Node, key: &str) -> Option<f64> {
    node.props
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

/// `xTicks` (0 draws none, one at or above the number of points labels them all) or `yTicks`
/// (both ends included; gpui-component raises anything below 2 to 2).
fn ticks(node: &Node, key: &str, default: usize) -> usize {
    node.props
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |count| count as usize)
}

fn name(node: &Node) -> Option<&str> {
    node.props.get("name").and_then(Value::as_str)
}

/// A line: `curve` (`linear`, `natural` or `step`), `dot`, `xAxis`, `xTicks`, `yAxis`, `yTicks`,
/// `grid`, `yMin`/`yMax` and `interactive`.
fn line_chart(id: NodeId, node: &Node, data: Vec<Datum>, stroke: Hsla) -> AnyElement {
    let mut chart = LineChart::new(data)
        .id(("chart", id))
        .x_tick_count(ticks(node, "xTicks", X_TICKS))
        .y_tick_count(ticks(node, "yTicks", Y_TICKS))
        .x(|datum: &Datum| datum.label.clone())
        .y(|datum: &Datum| datum.value)
        .stroke(stroke)
        .interactive(flag(node, "interactive", true))
        .x_axis(flag(node, "xAxis", true))
        .y_axis(flag(node, "yAxis", false))
        .grid(flag(node, "grid", true));
    chart = match node.props.get("curve").and_then(Value::as_str) {
        Some("natural") => chart.natural(),
        Some("step") => chart.step_after(),
        _ => chart.linear(),
    };
    if flag(node, "dot", false) {
        chart = chart.dot();
    }
    if let (Some(min), Some(max)) = (number(node, "yMin"), number(node, "yMax"))
        && min < max
    {
        chart = chart.y_domain(min, max);
    }
    if let Some(name) = name(node) {
        chart = chart.name(name.to_owned());
    }
    chart.into_any_element()
}

/// Bars: `horizontal`, `xAxis` and `xTicks` (the label axis), `yAxis` and `yTicks` (the value
/// axis), `grid` and `interactive`.
fn bar_chart(id: NodeId, node: &Node, data: Vec<Datum>, fill: Hsla) -> AnyElement {
    let mut chart = BarChart::new(data)
        .id(("chart", id))
        .band_tick_count(ticks(node, "xTicks", X_TICKS))
        .value_tick_count(ticks(node, "yTicks", Y_TICKS))
        .band(|datum: &Datum| datum.label.clone())
        .value(|datum: &Datum| datum.value)
        .fill(move |_: &Datum, _: Bounds<f32>, _: Bounds<f32>, _: BarAlignment| fill)
        .interactive(flag(node, "interactive", true))
        .label_axis(flag(node, "xAxis", true))
        .value_axis(flag(node, "yAxis", false))
        .grid(flag(node, "grid", true));
    if flag(node, "horizontal", false) {
        chart = chart.alignment(BarAlignment::Left);
    }
    if let Some(name) = name(node) {
        chart = chart.name(name.to_owned());
    }
    chart.into_any_element()
}

/// A pie: one slice per datum, colored in turn with the theme's chart colors, with `innerRadius`
/// (a donut from that many pixels in) and `padAngle` (the gap between slices, in radians).
fn pie_chart(id: NodeId, node: &Node, data: Vec<Datum>, theme: &Theme) -> AnyElement {
    let palette = palette(theme);
    let mut chart = PieChart::new(data)
        .id(("chart", id))
        .value(|datum: &Datum| datum.value as f32)
        .label(|datum: &Datum| datum.label.clone())
        .tooltip_name(|datum: &Datum| datum.label.clone())
        .color(move |datum: &Datum| palette[datum.index % palette.len()])
        .inner_radius(number(node, "innerRadius").unwrap_or(0.) as f32)
        .pad_angle(number(node, "padAngle").unwrap_or(0.) as f32)
        .interactive(flag(node, "interactive", true));
    if let Some(name) = name(node) {
        chart = chart.name(name.to_owned());
    }
    chart.into_any_element()
}

/// Props: `chart`, `data` (applied by `sync`, which keeps the parsed points), `color`, `name`
/// and the options each plot takes (see the functions above). A chart has no events: its tooltip
/// is the component's own.
pub fn build(id: NodeId, node: &Node, slot: &ChartSlot, theme: &Theme) -> AnyElement {
    let data = slot.data.clone();
    let stroke = color(
        theme,
        node.props
            .get("color")
            .and_then(Value::as_str)
            .unwrap_or("chart-1"),
    );
    match Kind::of(node) {
        Kind::Line => line_chart(id, node, data, stroke),
        Kind::Bar => bar_chart(id, node, data, stroke),
        Kind::Pie => pie_chart(id, node, data, theme),
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{Kind, X_TICKS, Y_TICKS, check_data, parse, ticks};
    use crate::schema::NodeType;
    use crate::tree::Node;
    use serde_json::{Value, json};

    fn node(props: Value) -> Node {
        Node {
            node_type: NodeType::Chart,
            props: props.as_object().cloned().unwrap_or_default(),
            events: Default::default(),
            children: vec![],
            parent: None,
        }
    }

    #[test]
    fn data_is_checked_point_by_point() {
        assert!(check_data(&json!([{"label": "Mon", "value": 12}])).is_ok());
        assert!(check_data(&json!([])).is_ok());
        assert!(check_data(&json!([{"label": "Mon"}])).is_err());
        assert!(check_data(&json!([{"value": 12}])).is_err());
        assert!(check_data(&json!([{"label": "Mon", "value": "12"}])).is_err());
        assert!(check_data(&json!({"label": "Mon", "value": 12})).is_err());
    }

    #[test]
    fn a_series_of_a_thousand_points_keeps_its_order() {
        let series = json!(
            (0..1000)
                .map(|ix| json!({"label": format!("p{ix}"), "value": ix as f64}))
                .collect::<Vec<_>>()
        );
        assert!(check_data(&series).is_ok());
        let data = parse(&series);
        assert_eq!(data.len(), 1000);
        assert_eq!(data[999].label.as_ref(), "p999");
        assert_eq!(data[999].value, 999.);
        assert_eq!(data[999].index, 999);
    }

    #[test]
    fn ticks_default_per_axis() {
        assert_eq!(ticks(&node(json!({})), "xTicks", X_TICKS), 12);
        assert_eq!(ticks(&node(json!({})), "yTicks", Y_TICKS), 5);
        assert_eq!(ticks(&node(json!({"xTicks": 0})), "xTicks", X_TICKS), 0);
        assert_eq!(ticks(&node(json!({"yTicks": 3})), "yTicks", Y_TICKS), 3);
    }

    #[test]
    fn the_chart_prop_selects_the_plot() {
        assert_eq!(Kind::of(&node(json!({}))), Kind::Line);
        assert_eq!(Kind::of(&node(json!({"chart": "bar"}))), Kind::Bar);
        assert_eq!(Kind::of(&node(json!({"chart": "pie"}))), Kind::Pie);
        assert_eq!(Kind::of(&node(json!({"chart": "radar"}))), Kind::Line);
    }
}
