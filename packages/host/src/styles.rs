//! Tailwind-like class translation to GPUI styles (PROTOCOL.md section 8).
//!
//! A class string is parsed once and cached. Colors are kept as theme roles and
//! resolved at render time, so a theme switch needs no cache invalidation.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::component::theme::Theme;
use gpui_kit::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeRole {
    Background,
    Foreground,
    Primary,
    PrimaryForeground,
    Secondary,
    SecondaryForeground,
    Muted,
    MutedForeground,
    Accent,
    AccentForeground,
    Danger,
    DangerForeground,
    Success,
    SuccessForeground,
    Warning,
    WarningForeground,
    Info,
    InfoForeground,
    Popover,
    PopoverForeground,
    Border,
    Input,
    Ring,
}

/// Theme colors usable after `bg-`, `text-` and `border-`, by their gpui-component names.
const ROLES: [(&str, ThemeRole); 23] = [
    ("background", ThemeRole::Background),
    ("foreground", ThemeRole::Foreground),
    ("primary", ThemeRole::Primary),
    ("primary-foreground", ThemeRole::PrimaryForeground),
    ("secondary", ThemeRole::Secondary),
    ("secondary-foreground", ThemeRole::SecondaryForeground),
    ("muted", ThemeRole::Muted),
    ("muted-foreground", ThemeRole::MutedForeground),
    ("accent", ThemeRole::Accent),
    ("accent-foreground", ThemeRole::AccentForeground),
    ("danger", ThemeRole::Danger),
    ("danger-foreground", ThemeRole::DangerForeground),
    ("success", ThemeRole::Success),
    ("success-foreground", ThemeRole::SuccessForeground),
    ("warning", ThemeRole::Warning),
    ("warning-foreground", ThemeRole::WarningForeground),
    ("info", ThemeRole::Info),
    ("info-foreground", ThemeRole::InfoForeground),
    ("popover", ThemeRole::Popover),
    ("popover-foreground", ThemeRole::PopoverForeground),
    ("border", ThemeRole::Border),
    ("input", ThemeRole::Input),
    ("ring", ThemeRole::Ring),
];

/// Scroll axes of `overflow-*-scroll`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollAxis {
    X,
    Y,
    Both,
}

/// Easing of a `transition` (`ease-*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
}

/// Tailwind's `duration-*` values, in milliseconds.
const DURATIONS: [u32; 8] = [75, 100, 150, 200, 300, 500, 700, 1000];

/// One supported class. Spacing values are in Tailwind units (1 = 0.25rem).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Class {
    Flex,
    FlexRow,
    FlexCol,
    Flex1,
    FlexWrap,
    ItemsStart,
    ItemsCenter,
    ItemsEnd,
    JustifyStart,
    JustifyCenter,
    JustifyEnd,
    JustifyBetween,
    Gap(f32),
    P(f32),
    Px(f32),
    Py(f32),
    Pt(f32),
    Pr(f32),
    Pb(f32),
    Pl(f32),
    M(f32),
    Mx(f32),
    My(f32),
    WFull,
    HFull,
    SizeFull,
    W(f32),
    H(f32),
    Size(f32),
    MinW0,
    MinH0,
    Shrink0,
    FlexNone,
    OverflowHidden,
    /// Only on `div`, which then gets a scrollbar.
    OverflowScroll(ScrollAxis),
    Border,
    BorderT,
    BorderR,
    BorderB,
    BorderL,
    RoundedSm,
    RoundedMd,
    RoundedLg,
    RoundedFull,
    TextXs,
    TextSm,
    TextBase,
    TextLg,
    TextXl,
    FontMedium,
    FontBold,
    FontMono,
    Truncate,
    WhitespaceNormal,
    WhitespaceNowrap,
    CursorPointer,
    Bg(ThemeRole),
    TextColor(ThemeRole),
    BorderColor(ThemeRole),
    /// `hover:` colors; only on `div`.
    HoverBg(ThemeRole),
    HoverTextColor(ThemeRole),
    HoverBorderColor(ThemeRole),
    /// 0 to 1.
    Opacity(f32),
    /// Animate color and opacity changes (see `animation`).
    Transition,
    DurationMs(u32),
    Ease(Ease),
    /// Only on `Icon`.
    AnimateSpin,
    AnimatePulse,
}

pub fn parse_class(name: &str) -> Option<Class> {
    use Class::*;
    let class = match name {
        "flex" => Flex,
        "flex-row" => FlexRow,
        "flex-col" => FlexCol,
        "flex-1" => Flex1,
        "flex-wrap" => FlexWrap,
        "items-start" => ItemsStart,
        "items-center" => ItemsCenter,
        "items-end" => ItemsEnd,
        "justify-start" => JustifyStart,
        "justify-center" => JustifyCenter,
        "justify-end" => JustifyEnd,
        "justify-between" => JustifyBetween,
        "w-full" => WFull,
        "h-full" => HFull,
        "size-full" => SizeFull,
        "min-w-0" => MinW0,
        "min-h-0" => MinH0,
        "shrink-0" => Shrink0,
        "flex-none" => FlexNone,
        "overflow-hidden" => OverflowHidden,
        "overflow-scroll" => OverflowScroll(ScrollAxis::Both),
        "overflow-x-scroll" => OverflowScroll(ScrollAxis::X),
        "overflow-y-scroll" => OverflowScroll(ScrollAxis::Y),
        "border" => Border,
        "border-t" => BorderT,
        "border-r" => BorderR,
        "border-b" => BorderB,
        "border-l" => BorderL,
        "rounded-sm" => RoundedSm,
        "rounded-md" => RoundedMd,
        "rounded-lg" => RoundedLg,
        "rounded-full" => RoundedFull,
        "text-xs" => TextXs,
        "text-sm" => TextSm,
        "text-base" => TextBase,
        "text-lg" => TextLg,
        "text-xl" => TextXl,
        "font-medium" => FontMedium,
        "font-bold" => FontBold,
        "font-mono" => FontMono,
        "truncate" => Truncate,
        "whitespace-normal" => WhitespaceNormal,
        // GPUI keeps spaces and line breaks already, so `pre` only needs to stop wrapping.
        "whitespace-nowrap" | "whitespace-pre" => WhitespaceNowrap,
        "cursor-pointer" => CursorPointer,
        "transition" => Transition,
        // Before the `prefix-number` branch, which would split `ease-in-out` at its last dash.
        "ease-linear" => Ease(self::Ease::Linear),
        "ease-in" => Ease(self::Ease::In),
        "ease-out" => Ease(self::Ease::Out),
        "ease-in-out" => Ease(self::Ease::InOut),
        "animate-spin" => AnimateSpin,
        "animate-pulse" => AnimatePulse,
        _ => {
            if let Some(hover) = name.strip_prefix("hover:") {
                return match parse_color(hover)? {
                    Bg(role) => Some(HoverBg(role)),
                    TextColor(role) => Some(HoverTextColor(role)),
                    BorderColor(role) => Some(HoverBorderColor(role)),
                    _ => None,
                };
            }
            if let Some(class) = parse_color(name) {
                return Some(class);
            }
            let (prefix, value) = name.rsplit_once('-')?;
            let n = value
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0.0)?;
            match prefix {
                "gap" => Gap(n),
                "p" => P(n),
                "px" => Px(n),
                "py" => Py(n),
                "pt" => Pt(n),
                "pr" => Pr(n),
                "pb" => Pb(n),
                "pl" => Pl(n),
                "m" => M(n),
                "mx" => Mx(n),
                "my" => My(n),
                "w" => W(n),
                "h" => H(n),
                "size" => Size(n),
                "opacity" if n <= 100.0 && n % 5.0 == 0.0 => Opacity(n / 100.0),
                "duration" => DurationMs(DURATIONS.into_iter().find(|&ms| ms as f32 == n)?),
                _ => return None,
            }
        }
    };
    Some(class)
}

fn parse_color(name: &str) -> Option<Class> {
    let (make, role): (fn(ThemeRole) -> Class, _) = if let Some(role) = name.strip_prefix("bg-") {
        (Class::Bg, role)
    } else if let Some(role) = name.strip_prefix("text-") {
        (Class::TextColor, role)
    } else {
        (Class::BorderColor, name.strip_prefix("border-")?)
    };
    ROLES
        .iter()
        .find(|(name, _)| *name == role)
        .map(|&(_, role)| make(role))
}

/// Parsed class strings, plus the unknown classes already warned about.
#[derive(Default)]
pub struct StyleCache {
    parsed: HashMap<String, Rc<[Class]>>,
    warned: HashSet<String>,
}

impl StyleCache {
    /// Classes in source order; unknown ones are dropped with a one-time warning.
    pub fn classes(&mut self, class: &str) -> Rc<[Class]> {
        if let Some(classes) = self.parsed.get(class) {
            return classes.clone();
        }
        let classes: Rc<[Class]> = class
            .split_whitespace()
            .filter_map(|name| {
                let parsed = parse_class(name);
                if parsed.is_none() && self.warned.insert(name.to_owned()) {
                    log::warn!("unknown class '{name}' ignored");
                }
                parsed
            })
            .collect();
        self.parsed.insert(class.to_owned(), classes.clone());
        classes
    }
}

fn spacing(n: f32) -> Rems {
    rems(n * 0.25)
}

pub fn color(theme: &Theme, role: ThemeRole) -> Hsla {
    match role {
        ThemeRole::Background => theme.background,
        ThemeRole::Foreground => theme.foreground,
        ThemeRole::Primary => theme.primary,
        ThemeRole::PrimaryForeground => theme.primary_foreground,
        ThemeRole::Secondary => theme.secondary,
        ThemeRole::SecondaryForeground => theme.secondary_foreground,
        ThemeRole::Muted => theme.muted,
        ThemeRole::MutedForeground => theme.muted_foreground,
        ThemeRole::Accent => theme.accent,
        ThemeRole::AccentForeground => theme.accent_foreground,
        ThemeRole::Danger => theme.danger,
        ThemeRole::DangerForeground => theme.danger_foreground,
        ThemeRole::Success => theme.success,
        ThemeRole::SuccessForeground => theme.success_foreground,
        ThemeRole::Warning => theme.warning,
        ThemeRole::WarningForeground => theme.warning_foreground,
        ThemeRole::Info => theme.info,
        ThemeRole::InfoForeground => theme.info_foreground,
        ThemeRole::Popover => theme.popover,
        ThemeRole::PopoverForeground => theme.popover_foreground,
        ThemeRole::Border => theme.border,
        ThemeRole::Input => theme.input,
        ThemeRole::Ring => theme.ring,
    }
}

/// Applies classes in order, so a later class overrides an earlier one (`p-2 p-4` gives `p-4`).
/// `OverflowScroll` and `Hover*` need an interactive element; see `scroll_axis` and `apply_hover`.
pub fn apply<E: Styled>(element: E, classes: &[Class], theme: &Theme) -> E {
    classes.iter().fold(element, |el, class| match *class {
        Class::Flex => el.flex(),
        Class::FlexRow => el.flex_row(),
        Class::FlexCol => el.flex_col(),
        Class::Flex1 => el.flex_1(),
        Class::FlexWrap => el.flex_wrap(),
        Class::ItemsStart => el.items_start(),
        Class::ItemsCenter => el.items_center(),
        Class::ItemsEnd => el.items_end(),
        Class::JustifyStart => el.justify_start(),
        Class::JustifyCenter => el.justify_center(),
        Class::JustifyEnd => el.justify_end(),
        Class::JustifyBetween => el.justify_between(),
        Class::Gap(n) => el.gap(spacing(n)),
        Class::P(n) => el.p(spacing(n)),
        Class::Px(n) => el.px(spacing(n)),
        Class::Py(n) => el.py(spacing(n)),
        Class::Pt(n) => el.pt(spacing(n)),
        Class::Pr(n) => el.pr(spacing(n)),
        Class::Pb(n) => el.pb(spacing(n)),
        Class::Pl(n) => el.pl(spacing(n)),
        Class::M(n) => el.m(spacing(n)),
        Class::Mx(n) => el.mx(spacing(n)),
        Class::My(n) => el.my(spacing(n)),
        Class::WFull => el.w_full(),
        Class::HFull => el.h_full(),
        Class::SizeFull => el.size_full(),
        Class::W(n) => el.w(spacing(n)),
        Class::H(n) => el.h(spacing(n)),
        Class::Size(n) => el.size(spacing(n)),
        Class::MinW0 => el.min_w(px(0.)),
        Class::MinH0 => el.min_h(px(0.)),
        Class::Shrink0 => el.flex_shrink_0(),
        Class::FlexNone => el.flex_none(),
        Class::OverflowHidden => el.overflow_hidden(),
        Class::Border => el.border_1(),
        Class::BorderT => el.border_t_1(),
        Class::BorderR => el.border_r_1(),
        Class::BorderB => el.border_b_1(),
        Class::BorderL => el.border_l_1(),
        Class::RoundedSm => el.rounded_sm(),
        Class::RoundedMd => el.rounded_md(),
        Class::RoundedLg => el.rounded_lg(),
        Class::RoundedFull => el.rounded_full(),
        Class::TextXs => el.text_xs(),
        Class::TextSm => el.text_sm(),
        Class::TextBase => el.text_base(),
        Class::TextLg => el.text_lg(),
        Class::TextXl => el.text_xl(),
        Class::FontMedium => el.font_weight(FontWeight::MEDIUM),
        Class::FontBold => el.font_weight(FontWeight::BOLD),
        Class::FontMono => el.font_family(theme.mono_font_family.clone()),
        Class::Truncate => el.truncate(),
        Class::WhitespaceNormal => el.whitespace_normal(),
        Class::WhitespaceNowrap => el.whitespace_nowrap(),
        Class::CursorPointer => el.cursor_pointer(),
        Class::Bg(role) => el.bg(color(theme, role)),
        Class::TextColor(role) => el.text_color(color(theme, role)),
        Class::BorderColor(role) => el.border_color(color(theme, role)),
        Class::Opacity(opacity) => el.opacity(opacity),
        // Handled by `animation`, `scroll_axis` and `apply_hover`.
        Class::OverflowScroll(_)
        | Class::HoverBg(_)
        | Class::HoverTextColor(_)
        | Class::HoverBorderColor(_)
        | Class::Transition
        | Class::DurationMs(_)
        | Class::Ease(_)
        | Class::AnimateSpin
        | Class::AnimatePulse => el,
    })
}

/// The last `overflow-*-scroll` class, if any.
pub fn scroll_axis(classes: &[Class]) -> Option<ScrollAxis> {
    classes.iter().rev().find_map(|class| match *class {
        Class::OverflowScroll(axis) => Some(axis),
        _ => None,
    })
}

/// Whether any `hover:` class is present.
pub fn has_hover(classes: &[Class]) -> bool {
    classes.iter().any(|class| {
        matches!(
            class,
            Class::HoverBg(_) | Class::HoverTextColor(_) | Class::HoverBorderColor(_)
        )
    })
}

/// Applies the `hover:` classes, in order.
pub fn apply_hover<E: InteractiveElement>(element: E, classes: &[Class], theme: &Theme) -> E {
    let hover: Vec<Class> = classes
        .iter()
        .filter_map(|class| match *class {
            Class::HoverBg(role) => Some(Class::Bg(role)),
            Class::HoverTextColor(role) => Some(Class::TextColor(role)),
            Class::HoverBorderColor(role) => Some(Class::BorderColor(role)),
            _ => None,
        })
        .collect();
    if hover.is_empty() {
        return element;
    }
    element.hover(|style| apply(style, &hover, theme))
}

#[cfg(test)]
mod tests {
    // Explicit imports: `super::*` would bring GPUI's `test` macro, shadowing Rust's.
    use super::{Class, Ease, ScrollAxis, StyleCache, ThemeRole, parse_class};
    use std::collections::HashSet;
    use std::rc::Rc;

    #[test]
    fn parses_every_mvp_class() {
        let cases = [
            ("flex", Class::Flex),
            ("flex-row", Class::FlexRow),
            ("flex-col", Class::FlexCol),
            ("flex-1", Class::Flex1),
            ("flex-wrap", Class::FlexWrap),
            ("items-start", Class::ItemsStart),
            ("items-center", Class::ItemsCenter),
            ("items-end", Class::ItemsEnd),
            ("justify-start", Class::JustifyStart),
            ("justify-center", Class::JustifyCenter),
            ("justify-end", Class::JustifyEnd),
            ("justify-between", Class::JustifyBetween),
            ("gap-2", Class::Gap(2.0)),
            ("p-4", Class::P(4.0)),
            ("px-1", Class::Px(1.0)),
            ("py-0.5", Class::Py(0.5)),
            ("pt-3", Class::Pt(3.0)),
            ("pr-3", Class::Pr(3.0)),
            ("pb-3", Class::Pb(3.0)),
            ("pl-3", Class::Pl(3.0)),
            ("m-2", Class::M(2.0)),
            ("mx-2", Class::Mx(2.0)),
            ("my-2", Class::My(2.0)),
            ("w-full", Class::WFull),
            ("h-full", Class::HFull),
            ("size-full", Class::SizeFull),
            ("w-64", Class::W(64.0)),
            ("h-10", Class::H(10.0)),
            ("border", Class::Border),
            ("rounded-sm", Class::RoundedSm),
            ("rounded-md", Class::RoundedMd),
            ("rounded-lg", Class::RoundedLg),
            ("rounded-full", Class::RoundedFull),
            ("text-sm", Class::TextSm),
            ("text-base", Class::TextBase),
            ("text-lg", Class::TextLg),
            ("text-xl", Class::TextXl),
            ("font-medium", Class::FontMedium),
            ("font-bold", Class::FontBold),
            ("bg-background", Class::Bg(ThemeRole::Background)),
            ("bg-muted", Class::Bg(ThemeRole::Muted)),
            ("bg-primary", Class::Bg(ThemeRole::Primary)),
            ("text-foreground", Class::TextColor(ThemeRole::Foreground)),
            (
                "text-muted-foreground",
                Class::TextColor(ThemeRole::MutedForeground),
            ),
            ("text-primary", Class::TextColor(ThemeRole::Primary)),
            (
                "text-primary-foreground",
                Class::TextColor(ThemeRole::PrimaryForeground),
            ),
            ("border-border", Class::BorderColor(ThemeRole::Border)),
            ("bg-secondary", Class::Bg(ThemeRole::Secondary)),
            ("bg-danger", Class::Bg(ThemeRole::Danger)),
            (
                "text-success-foreground",
                Class::TextColor(ThemeRole::SuccessForeground),
            ),
            ("border-ring", Class::BorderColor(ThemeRole::Ring)),
            ("size-4", Class::Size(4.0)),
            ("min-w-0", Class::MinW0),
            ("min-h-0", Class::MinH0),
            ("shrink-0", Class::Shrink0),
            ("flex-none", Class::FlexNone),
            ("overflow-hidden", Class::OverflowHidden),
            ("overflow-scroll", Class::OverflowScroll(ScrollAxis::Both)),
            ("overflow-x-scroll", Class::OverflowScroll(ScrollAxis::X)),
            ("overflow-y-scroll", Class::OverflowScroll(ScrollAxis::Y)),
            ("border-t", Class::BorderT),
            ("border-r", Class::BorderR),
            ("border-b", Class::BorderB),
            ("border-l", Class::BorderL),
            ("text-xs", Class::TextXs),
            ("font-mono", Class::FontMono),
            ("truncate", Class::Truncate),
            ("whitespace-normal", Class::WhitespaceNormal),
            ("whitespace-nowrap", Class::WhitespaceNowrap),
            ("whitespace-pre", Class::WhitespaceNowrap),
            ("cursor-pointer", Class::CursorPointer),
            ("hover:bg-accent", Class::HoverBg(ThemeRole::Accent)),
            (
                "hover:text-primary",
                Class::HoverTextColor(ThemeRole::Primary),
            ),
            (
                "hover:border-ring",
                Class::HoverBorderColor(ThemeRole::Ring),
            ),
            ("opacity-0", Class::Opacity(0.0)),
            ("opacity-50", Class::Opacity(0.5)),
            ("opacity-100", Class::Opacity(1.0)),
            ("transition", Class::Transition),
            ("duration-75", Class::DurationMs(75)),
            ("duration-150", Class::DurationMs(150)),
            ("duration-1000", Class::DurationMs(1000)),
            ("ease-linear", Class::Ease(Ease::Linear)),
            ("ease-in", Class::Ease(Ease::In)),
            ("ease-out", Class::Ease(Ease::Out)),
            ("ease-in-out", Class::Ease(Ease::InOut)),
            ("animate-spin", Class::AnimateSpin),
            ("animate-pulse", Class::AnimatePulse),
        ];
        for (name, expected) in cases {
            assert_eq!(parse_class(name), Some(expected), "{name}");
        }
    }

    #[test]
    fn rejects_unknown_and_malformed_classes() {
        for name in [
            "",
            "grid",
            "bg-red-500",
            "p-",
            "p-x",
            "p--1",
            "gap-NaN",
            "gap-inf",
            "z-10",
            "text-2xl",
            "flex-",
            "bg-",
            "bg-destructive",
            "border-primary-",
            "text-card",
            "hover:p-2",
            "hover:",
            "hover:hover:bg-accent",
            "border-x",
            "min-w-1",
            "opacity-7",
            "opacity-101",
            "opacity-",
            "duration-123",
            "duration-0",
            "ease-fast",
            "ease-",
            "animate-ping",
            "animate-bounce",
            "animate-none",
            "transition-colors",
        ] {
            assert_eq!(parse_class(name), None, "{name}");
        }
    }

    #[test]
    fn cache_keeps_order_drops_unknown_and_warns_once() {
        let mut cache = StyleCache::default();
        let classes = cache.classes("p-2 grid p-4  flex grid");
        assert_eq!(&*classes, &[Class::P(2.0), Class::P(4.0), Class::Flex]);
        assert_eq!(cache.warned, HashSet::from(["grid".to_string()]));

        cache.classes("grid gap-1");
        assert_eq!(cache.warned.len(), 1);
    }

    #[test]
    fn cache_reuses_parsed_strings() {
        let mut cache = StyleCache::default();
        let first = cache.classes("flex gap-2");
        let second = cache.classes("flex gap-2");
        assert!(Rc::ptr_eq(&first, &second));
        assert_eq!(cache.parsed.len(), 1);
    }
}
