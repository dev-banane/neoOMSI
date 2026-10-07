use crate::humans::BusId;
use glam::{Vec2, Vec3};

const SEATED: f32 = 0.25;
const RISE: f32 = 1.2;
const PACE: f32 = 0.9;
const SIT: f32 = 1.4;
const SEAT_FRONT: f32 = 0.34;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CabMove {
    bus: BusId,
    hip: Vec3,
    heading: f32,
    height: f32,
    front: Vec3,
    stand: Vec3,
    pub(crate) sitting_down: bool,
    t: f32,
}

/// Bus frame.
pub(crate) struct CabStep {
    pub wheel: Option<(BusId, Vec3, f32, f32)>,
    pub at: Vec3,
    pub heading: f32,
    pub vel: Vec2,
    pub done: bool,
}

impl CabMove {
    pub(crate) fn new(
        bus: BusId,
        (hip, heading, height): (Vec3, f32, f32),
        stand: Vec3,
        sitting_down: bool,
    ) -> CabMove {
        let r = heading.to_radians();
        let front = Vec3::new(
            hip.x + r.sin() * SEAT_FRONT,
            hip.y + r.cos() * SEAT_FRONT,
            hip.z - height,
        );
        CabMove {
            bus,
            hip,
            heading,
            height,
            front,
            stand,
            sitting_down,
            t: 0.0,
        }
    }

    fn walk(&self) -> f32 {
        ((self.stand - self.front).truncate().length() / PACE).max(0.4)
    }

    pub(crate) fn step(&mut self, dt: f32) -> CabStep {
        self.t += dt;
        let walk = self.walk();
        let seated = Some((self.bus, self.hip, self.heading, self.height));
        let out = (self.stand - self.front).truncate();
        let course = |d: Vec2| d.x.atan2(d.y).to_degrees();
        let pace = |d: Vec2| d.normalize_or_zero() * (d.length() / walk).min(PACE * 1.5);
        let still = |at: Vec3, wheel, done| CabStep {
            wheel,
            at,
            heading: self.heading,
            vel: Vec2::ZERO,
            done,
        };
        let stepping = |from: Vec3, to: Vec3, e: f32| CabStep {
            wheel: None,
            at: from.lerp(to, smooth(e)),
            heading: course((to - from).truncate()),
            vel: pace((to - from).truncate()),
            done: false,
        };
        let t = self.t;
        if self.sitting_down {
            if t < walk {
                stepping(self.stand, self.front, t / walk)
            } else {
                still(self.front, seated, t >= walk + SIT)
            }
        } else if t < SEATED {
            still(self.front, seated, false)
        } else if t < SEATED + RISE {
            still(self.front, None, false)
        } else if t < SEATED + RISE + walk {
            stepping(self.front, self.stand, (t - SEATED - RISE) / walk)
        } else {
            CabStep {
                wheel: None,
                at: self.stand,
                heading: course(out),
                vel: Vec2::ZERO,
                done: true,
            }
        }
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_driver_stands_up_in_front_of_the_seat_before_stepping_out() {
        let seat = (Vec3::new(-0.7, 4.6, 1.1), 0.0, 0.4);
        let stand = Vec3::new(0.1, 4.4, 0.7);
        let mut m = CabMove::new(BusId::Player, seat, stand, false);
        let s = m.step(0.1);
        assert!(s.wheel.is_some(), "still sitting at first");
        let s = m.step(0.5);
        assert!(s.wheel.is_none());
        assert!(
            (s.at - Vec3::new(-0.7, 4.94, 0.7)).length() < 1e-3,
            "rising where the feet were"
        );
        let mut last = s.at;
        let mut done = false;
        for _ in 0..200 {
            let s = m.step(1.0 / 30.0);
            assert!((s.at - last).length() < 0.05, "no jump");
            last = s.at;
            if s.done {
                done = true;
                break;
            }
        }
        assert!(done && (last - stand).length() < 1e-3);
    }

    #[test]
    fn sitting_down_steps_to_the_seat_first() {
        let seat = (Vec3::new(-0.7, 4.6, 1.1), 0.0, 0.4);
        let stand = Vec3::new(0.1, 4.4, 0.7);
        let mut m = CabMove::new(BusId::Player, seat, stand, true);
        let s = m.step(0.1);
        assert!(
            s.wheel.is_none() && s.vel.length() > 0.1,
            "walking to the seat"
        );
        let mut sat = false;
        for _ in 0..200 {
            let s = m.step(1.0 / 30.0);
            sat |= s.wheel.is_some();
            if s.done {
                break;
            }
        }
        assert!(sat);
    }
}
