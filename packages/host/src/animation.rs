//! Animations (PROTOCOL.md section 8): `transition` interpolates a node's colors and opacity
//! toward what its classes, the theme and hover ask for; `animate-pulse` and `animate-spin`
//! loop. Everything is a pure function of the time, so the window only has to re-render
//! (`request_animation_frame`) while something moves.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use gpui_kit::component::theme::Theme;
use gpui_kit::*;

use crate::styles::{self, Class, Ease};

/// The animatable part of a node's style. `None` is "not set by a class".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visual {
    pub bg: Option<Hsla>,
    pub text: Option<Hsla>,
    pub border: Option<Hsla>,
    pub opacity: Option<f32>,
}

impl Visual {
    /// What the classes ask for, in order; `hover:` colors win while `hovered`.
    pub fn resolve(classes: &[Class], theme: &Theme, hovered: bool) -> Self {
        let mut visual = Visual {
            bg: None,
            text: None,
            border: None,
            opacity: None,
        };
        for class in classes {
            match *class {
                Class::Bg(role) => visual.bg = Some(styles::color(theme, role)),
                Class::TextColor(role) => visual.text = Some(styles::color(theme, role)),
                Class::BorderColor(role) => visual.border = Some(styles::color(theme, role)),
                Class::Opacity(opacity) => visual.opacity = Some(opacity),
                _ => {}
            }
        }
        if hovered {
            for class in classes {
                match *class {
                    Class::HoverBg(role) => visual.bg = Some(styles::color(theme, role)),
                    Class::HoverTextColor(role) => visual.text = Some(styles::color(theme, role)),
                    Class::HoverBorderColor(role) => {
                        visual.border = Some(styles::color(theme, role))
                    }
                    _ => {}
                }
            }
        }
        visual
    }

    /// Sets the values on top of what `styles::apply` set.
    pub fn apply<E: Styled>(self, mut element: E) -> E {
        if let Some(bg) = self.bg {
            element = element.bg(bg);
        }
        if let Some(text) = self.text {
            element = element.text_color(text);
        }
        if let Some(border) = self.border {
            element = element.border_color(border);
        }
        if let Some(opacity) = self.opacity {
            element = element.opacity(opacity);
        }
        element
    }

    fn lerp(from: Self, to: Self, t: f32) -> Self {
        Visual {
            // No background is transparent, so it fades from or to the other color.
            bg: match (from.bg, to.bg) {
                (Some(a), Some(b)) => Some(lerp_color(a, b, t)),
                (None, Some(b)) => Some(lerp_color(b.opacity(0.), b, t)),
                (Some(a), None) => Some(lerp_color(a, a.opacity(0.), t)),
                (None, None) => None,
            },
            // An unset text or border color is inherited or GPUI's default: unknown here, so it switches at once.
            text: lerp_known(from.text, to.text, t),
            border: lerp_known(from.border, to.border, t),
            opacity: match (from.opacity, to.opacity) {
                (None, None) => None,
                (a, b) => Some(lerp(a.unwrap_or(1.), b.unwrap_or(1.), t)),
            },
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_known(from: Option<Hsla>, to: Option<Hsla>, t: f32) -> Option<Hsla> {
    match (from, to) {
        (Some(a), Some(b)) => Some(lerp_color(a, b, t)),
        _ => to,
    }
}

/// In RGBA: going around the hue would pass through unrelated colors (red to blue via green).
fn lerp_color(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (a.to_rgb(), b.to_rgb());
    Rgba {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
    .into()
}

/// `duration-*` and `ease-*` of a node with `transition` (Tailwind's defaults: 150 ms, in-out).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    duration: Duration,
    ease: Ease,
}

impl Timing {
    /// `None` without the `transition` class.
    pub fn of(classes: &[Class]) -> Option<Self> {
        classes.contains(&Class::Transition).then(|| {
            let mut timing = Timing {
                duration: Duration::from_millis(150),
                ease: Ease::InOut,
            };
            for class in classes {
                match *class {
                    Class::DurationMs(ms) => timing.duration = Duration::from_millis(ms.into()),
                    Class::Ease(ease) => timing.ease = ease,
                    _ => {}
                }
            }
            timing
        })
    }

    fn progress(self, elapsed: Duration) -> f32 {
        let t = (elapsed.as_secs_f32() / self.duration.as_secs_f32().max(f32::EPSILON)).min(1.);
        match self.ease {
            Ease::Linear => t,
            Ease::In => t * t,
            Ease::Out => 1. - (1. - t) * (1. - t),
            Ease::InOut => ease_in_out(t),
        }
    }
}

/// One node's transition: from the value shown when its target last changed, to that target.
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    from: Visual,
    to: Visual,
    start: Instant,
    timing: Timing,
}

impl Transition {
    /// A node seen for the first time shows its target without animating.
    pub fn at_rest(target: Visual, timing: Timing, now: Instant) -> Self {
        Transition {
            from: target,
            to: target,
            start: now.checked_sub(timing.duration).unwrap_or(now),
            timing,
        }
    }

    /// Heads for `target`, starting from what is on screen now (also mid-transition).
    pub fn retarget(&mut self, target: Visual, timing: Timing, now: Instant) {
        if target != self.to {
            *self = Transition {
                from: self.current(now),
                to: target,
                start: now,
                timing,
            };
        }
    }

    pub fn current(&self, now: Instant) -> Visual {
        Visual::lerp(
            self.from,
            self.to,
            self.timing
                .progress(now.saturating_duration_since(self.start)),
        )
    }

    pub fn running(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) < self.timing.duration
    }
}

/// `animate-pulse`: opacity 1 → 0.5 → 1 every 2 s, like Tailwind's.
pub fn pulse(elapsed: Duration) -> f32 {
    let phase = (elapsed.as_secs_f32() / 2.) % 1.;
    let half = 1. - (2. * phase - 1.).abs();
    1. - 0.5 * ease_in_out(half)
}

/// `animate-spin`: one turn per second.
pub fn spin(elapsed: Duration) -> Radians {
    radians((elapsed.as_secs_f32() % 1.) * TAU)
}

#[cfg(test)]
mod tests {
    use super::{Duration, Instant, Timing, Transition, Visual, pulse};
    use crate::styles::{Class, Ease};
    use gpui_kit::{Hsla, Rgba};

    fn rgb(r: f32, g: f32, b: f32) -> Hsla {
        Rgba { r, g, b, a: 1. }.into()
    }

    fn visual(bg: Hsla) -> Visual {
        Visual {
            bg: Some(bg),
            text: None,
            border: None,
            opacity: None,
        }
    }

    fn close(a: Hsla, b: Hsla) -> bool {
        let (a, b) = (a.to_rgb(), b.to_rgb());
        [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
            .iter()
            .all(|(x, y)| (x - y).abs() < 0.01)
    }

    const LINEAR_1S: Timing = Timing {
        duration: Duration::from_secs(1),
        ease: Ease::Linear,
    };

    #[test]
    fn timing_needs_transition_and_reads_duration_and_ease() {
        assert_eq!(Timing::of(&[Class::DurationMs(300)]), None);
        assert_eq!(
            Timing::of(&[Class::Transition]),
            Some(Timing {
                duration: Duration::from_millis(150),
                ease: Ease::InOut
            })
        );
        assert_eq!(
            Timing::of(&[
                Class::Transition,
                Class::DurationMs(1000),
                Class::Ease(Ease::Linear)
            ]),
            Some(LINEAR_1S)
        );
    }

    #[test]
    fn interpolates_in_rgba_and_finishes() {
        let (red, blue) = (rgb(1., 0., 0.), rgb(0., 0., 1.));
        let t0 = Instant::now();
        let mut transition = Transition::at_rest(visual(red), LINEAR_1S, t0);
        assert!(!transition.running(t0), "a new node does not animate");
        assert_eq!(transition.current(t0), visual(red));

        transition.retarget(visual(blue), LINEAR_1S, t0);
        let half = transition
            .current(t0 + Duration::from_millis(500))
            .bg
            .unwrap();
        assert!(
            close(half, rgb(0.5, 0., 0.5)),
            "halfway is purple, not green: {:?}",
            half.to_rgb()
        );
        assert!(transition.running(t0 + Duration::from_millis(500)));
        assert_eq!(
            transition.current(t0 + Duration::from_secs(2)),
            visual(blue)
        );
        assert!(!transition.running(t0 + Duration::from_secs(2)));
    }

    #[test]
    fn a_new_target_starts_from_what_is_shown() {
        let (red, blue) = (rgb(1., 0., 0.), rgb(0., 0., 1.));
        let t0 = Instant::now();
        let mut transition = Transition::at_rest(visual(red), LINEAR_1S, t0);
        transition.retarget(visual(blue), LINEAR_1S, t0);
        // Back to red halfway: it starts from purple, so right after it is still near purple.
        let mid = t0 + Duration::from_millis(500);
        transition.retarget(visual(red), LINEAR_1S, mid);
        assert!(close(
            transition.current(mid).bg.unwrap(),
            rgb(0.5, 0., 0.5)
        ));
        assert!(close(
            transition.current(mid + Duration::from_secs(1)).bg.unwrap(),
            red
        ));
        // The same target again does not restart it.
        let start = transition.start;
        transition.retarget(visual(red), LINEAR_1S, mid + Duration::from_millis(100));
        assert_eq!(transition.start, start);
    }

    #[test]
    fn unset_values_fade_or_switch() {
        let red = rgb(1., 0., 0.);
        let none = Visual {
            bg: None,
            text: None,
            border: None,
            opacity: None,
        };
        let shown = Visual {
            bg: Some(red),
            text: Some(red),
            border: None,
            opacity: Some(0.),
        };
        let half = Visual::lerp(none, shown, 0.5);
        assert!(
            close(half.bg.unwrap(), red.opacity(0.5)),
            "no background fades in"
        );
        assert_eq!(half.text, Some(red), "an unset text color switches at once");
        assert_eq!(half.opacity, Some(0.5), "no opacity class is 1");
    }

    #[test]
    fn pulse_goes_down_to_half_and_back() {
        assert!((pulse(Duration::ZERO) - 1.).abs() < 1e-4);
        assert!((pulse(Duration::from_secs(1)) - 0.5).abs() < 1e-4);
        assert!((pulse(Duration::from_secs(2)) - 1.).abs() < 1e-4);
    }
}
