// Modified for rhy-mac-ios on 2026-10-20; see NOTICE and docs/PROVENANCE.md.
// Mush8 - touch.rs
// Constantin Clerc (c22dev) - 0.1
use crate::input::{self, A, B, DOWN, L, LEFT, MINUS, PLUS, R, RIGHT, UP, X, Y};
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSSet, NSString};
use objc2_ui_kit::{NSTextAlignment, UIColor, UIEvent, UIFont, UILabel, UIResponder, UITouch, UIView, UIViewAutoresizing};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering::Relaxed;

const BUTTONS: [(&str, u64); 12] = [
    ("A", A), ("B", B), ("X", X), ("Y", Y), ("L", L), ("R", R), ("+", PLUS), ("−", MINUS),
    ("▲", UP), ("▼", DOWN), ("◀", LEFT), ("▶", RIGHT),
];
const FILL: f64 = 0.35;
const RING: f64 = 0.8;
const PRESSED: f64 = 0.7;

struct Control {
    bit: u64,
    center: CGPoint,
    radius: f64,
    half: f64,
    view: Retained<UIView>,
}

struct Pad {
    view: Retained<TouchView>,
    controls: Vec<Control>,
    held: HashMap<usize, u64>,
    shown: bool,
    screen: HashSet<usize>,
}

thread_local! {
    static PAD: RefCell<Option<Pad>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NXTouchView"]
    struct TouchView;

    impl TouchView {
        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };
            layout();
        }

        #[unsafe(method(touchesBegan:withEvent:))]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            handle(self, touches, Phase::Began);
        }

        #[unsafe(method(touchesMoved:withEvent:))]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            handle(self, touches, Phase::Moved);
        }

        #[unsafe(method(touchesEnded:withEvent:))]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            handle(self, touches, Phase::Ended);
        }

        #[unsafe(method(touchesCancelled:withEvent:))]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            handle(self, touches, Phase::Ended);
        }
    }
);

#[derive(PartialEq)]
enum Phase {
    Began,
    Moved,
    Ended,
}

fn circle(mtm: MainThreadMarker, parent: &UIView, title: &str, fill: (f64, f64), ring: f64) -> Retained<UIView> {
    let v = UIView::new(mtm);
    v.setUserInteractionEnabled(false);
    v.setBackgroundColor(Some(&UIColor::colorWithWhite_alpha(fill.0, fill.1)));
    let layer = v.layer();
    layer.setBorderWidth(2.0);
    let border = unsafe { UIColor::colorWithWhite_alpha(1.0, ring).CGColor() };
    layer.setBorderColor(Some(&border));
    if !title.is_empty() {
        let l = UILabel::new(mtm);
        l.setText(Some(&NSString::from_str(title)));
        unsafe { l.setTextColor(Some(&UIColor::whiteColor())) };
        l.setShadowColor(Some(&UIColor::colorWithWhite_alpha(0.0, 0.8)));
        l.setShadowOffset(CGSize::new(0.0, 1.0));
        l.setTextAlignment(NSTextAlignment::Center);
        l.setAdjustsFontSizeToFitWidth(true);
        v.addSubview(&l);
    }
    parent.addSubview(&v);
    v
}

fn place_capsule(v: &UIView, c: CGPoint, r: f64, half: f64) {
    let w = 2.0 * (r + half);
    v.setFrame(CGRect::new(CGPoint::new(c.x - r - half, c.y - r), CGSize::new(w, 2.0 * r)));
    v.layer().setCornerRadius(r);
    if let Some(l) = v.subviews().firstObject().and_then(|l| l.downcast::<UILabel>().ok()) {
        l.setFrame(CGRect::new(CGPoint::new(2.0, 0.0), CGSize::new(w - 4.0, 2.0 * r)));
        unsafe { l.setFont(Some(&UIFont::boldSystemFontOfSize((r * 0.55).clamp(12.0, 22.0)))) };
    }
}

pub fn install(parent: &UIView, mtm: MainThreadMarker) {
    let view: Retained<TouchView> = unsafe { msg_send![TouchView::alloc(mtm), initWithFrame: parent.bounds()] };
    view.setMultipleTouchEnabled(true);
    view.setAutoresizingMask(UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight);
    parent.addSubview(&view);
    let controls = BUTTONS.iter().map(|&(t, bit)| Control { bit, center: CGPoint::new(0.0, 0.0), radius: 0.0, half: 0.0, view: circle(mtm, &view, t, (0.0, FILL), RING) }).collect();
    let p = Pad { view, controls, held: HashMap::new(), shown: true, screen: HashSet::new() };
    show_controls(&p, false);
    PAD.with(|pad| *pad.borrow_mut() = Some(Pad { shown: false, ..p }));
}

fn show_controls(p: &Pad, visible: bool) {
    for v in p.controls.iter().map(|c| &c.view) {
        v.setHidden(!visible);
    }
}

pub fn set_visible(visible: bool) {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        let Some(p) = p.as_mut() else { return };
        show_controls(p, visible);
        p.shown = visible;
        if !visible {
            p.held.clear();
            publish(p);
        }
    });
}

fn layout() {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        let Some(p) = p.as_mut() else { return };
        let b = p.view.bounds().size;
        let inset = p.view.safeAreaInsets();
        let (left, right) = (inset.left, b.width - inset.right);
        let u = b.height.min(b.width) / 10.0;
        let face = CGPoint::new(right - 3.0 * u, b.height - 3.2 * u);
        let at = |dx: f64, dy: f64| CGPoint::new(face.x + dx, face.y + dy);
        let dpad = CGPoint::new(left + 3.2 * u, b.height - 3.2 * u);
        let direction = |dx: f64, dy: f64| (CGPoint::new(dpad.x + dx * u, dpad.y + dy * u), 0.7 * u, 0.0);
        let spots = [
            (at(1.35 * u, 0.0), 0.75 * u, 0.0),
            (at(0.0, 1.35 * u), 0.75 * u, 0.0),
            (at(0.0, -1.35 * u), 0.75 * u, 0.0),
            (at(-1.35 * u, 0.0), 0.75 * u, 0.0),
            (CGPoint::new(left + 2.6 * u, 1.2 * u), 0.75 * u, 1.0 * u),
            (CGPoint::new(right - 2.6 * u, 1.2 * u), 0.75 * u, 1.0 * u),
            (CGPoint::new(b.width / 2.0 + 1.2 * u, 0.9 * u), 0.5 * u, 0.0),
            (CGPoint::new(b.width / 2.0 - 1.2 * u, 0.9 * u), 0.5 * u, 0.0),
            direction(0.0, -1.25),
            direction(0.0, 1.25),
            direction(-1.25, 0.0),
            direction(1.25, 0.0),
        ];
        for (c, (center, radius, half)) in p.controls.iter_mut().zip(spots) {
            c.center = center;
            c.radius = radius;
            c.half = half;
            place_capsule(&c.view, center, radius, half);
        }
    });
}

fn handle(view: &TouchView, touches: &NSSet<UITouch>, phase: Phase) {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        let Some(p) = p.as_mut() else { return };
        let b = view.bounds().size;
        for t in touches.allObjects().iter() {
            let id = &*t as *const UITouch as usize;
            let pt = t.locationInView(Some(view));
            match phase {
                Phase::Began => {
                    if let Some(bit) = hit_button(p, pt).filter(|_| p.shown) {
                        p.held.insert(id, bit);
                    } else if !super::ios::CONTROLLER_MODE.load(Relaxed) {
                        let at = super::screen_point(pt.x, pt.y, b.width, b.height);
                        if super::on_screen(at) {
                            p.screen.insert(id);
                            input::screen_touch(id, Some(at));
                        }
                    }
                }
                Phase::Moved => {
                    if p.held.contains_key(&id) {
                        // Keep ownership until release, including outside the buttons,
                        // so sliding off a direction releases it without a screen tap.
                        let bit = if p.shown { hit_button(p, pt).unwrap_or(0) } else { 0 };
                        p.held.insert(id, bit);
                    } else if p.screen.contains(&id) {
                        input::screen_touch(id, Some(super::screen_point(pt.x, pt.y, b.width, b.height)));
                    }
                }
                Phase::Ended => {
                    p.held.remove(&id);
                    if p.screen.remove(&id) {
                        input::screen_touch(id, None);
                    }
                }
            }
        }
        publish(p);
    });
}

fn hit_button(p: &Pad, pt: CGPoint) -> Option<u64> {
    p.controls.iter()
        .filter(|c| c.radius > 0.0)
        .map(|c| (c.bit, ((pt.x - c.center.x).abs() - c.half).max(0.0).hypot(pt.y - c.center.y) / c.radius))
        .filter(|(_, d)| *d < 1.3)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(bit, _)| bit)
}

fn publish(p: &Pad) {
    let held = p.held.values().fold(0, |m, b| m | b);
    input::TOUCH_BUTTONS.store(held, Relaxed);
    input::set_touch_stick(0.0, 0.0);
    for c in &p.controls {
        let color = if held & c.bit != 0 { UIColor::colorWithWhite_alpha(1.0, PRESSED) } else { UIColor::colorWithWhite_alpha(0.0, FILL) };
        c.view.setBackgroundColor(Some(&color));
    }
}
