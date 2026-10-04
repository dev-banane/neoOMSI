//! Time-to-collision avoidance (Karamouzas, Skinner and Guy, 2014): people give way a few
//! metres early instead of bumping and sliding round each other.

use glam::DVec2;
use hashbrown::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Walker {
    pub pos: DVec2,
    pub vel: DVec2,
    pub radius: f64,
    pub want: DVec2,
    /// How far the person yields to others and to contact (0 not at all, 1 fully).
    pub give: f64,
    /// Seen by the others but not moved.
    pub fixed: bool,
    /// Passes through others for a moment: the way out of a deadlock in a doorway.
    pub ghost: bool,
    /// Keeps within `.2` of the segment `.0`-`.1`.
    pub corridor: Option<(DVec2, DVec2, f64)>,
}

#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub center: DVec2,
    /// Half extents: x across, y along the heading.
    pub half: DVec2,
    /// Radians, clockwise from north (like vehicle headings).
    pub heading: f64,
    pub vel: DVec2,
}

impl Block {
    fn axes(&self) -> (DVec2, DVec2) {
        let (s, c) = (self.heading.sin(), self.heading.cos());
        (DVec2::new(c, -s), DVec2::new(s, c))
    }

    pub fn closest(&self, p: DVec2) -> (DVec2, bool) {
        let (r, f) = self.axes();
        let d = p - self.center;
        let (x, y) = (d.dot(r), d.dot(f));
        let inside = x.abs() < self.half.x && y.abs() < self.half.y;
        if inside {
            // out through the nearest side
            let (dx, dy) = (self.half.x - x.abs(), self.half.y - y.abs());
            let q = if dx < dy {
                DVec2::new(self.half.x * sign(x), y)
            } else {
                DVec2::new(x, self.half.y * sign(y))
            };
            return (self.center + r * q.x + f * q.y, true);
        }
        let q = DVec2::new(
            x.clamp(-self.half.x, self.half.x),
            y.clamp(-self.half.y, self.half.y),
        );
        (self.center + r * q.x + f * q.y, false)
    }

    pub fn near(&self, p: DVec2, margin: f64) -> bool {
        let (r, f) = self.axes();
        let d = p - self.center;
        d.dot(r).abs() < self.half.x + margin && d.dot(f).abs() < self.half.y + margin
    }
}

fn sign(v: f64) -> f64 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

#[derive(Debug, Clone, Copy)]
pub struct CrowdParams {
    pub k: f64,
    pub tau0: f64,
    pub relax: f64,
    pub max_accel: f64,
    pub sense: f64,
}

impl Default for CrowdParams {
    fn default() -> Self {
        CrowdParams {
            k: 1.5,
            tau0: 3.0,
            relax: 0.45,
            max_accel: 2.2,
            sense: 4.0,
        }
    }
}

const CELL: f64 = 2.0;

fn cell_of(p: DVec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

fn ttc_force(p: &CrowdParams, x: DVec2, v: DVec2, xo: DVec2, vo: DVec2, r: f64) -> DVec2 {
    let w = xo - x;
    let dist = w.length();
    // already touching: count the contact at the current distance, so that the force
    // stays finite and still pushes apart
    let r = if dist < r { dist.max(1e-3) * 0.99 } else { r };
    let rv = v - vo;
    let a = rv.dot(rv);
    let b = w.dot(rv);
    let c = w.dot(w) - r * r;
    let discr = b * b - a * c;
    if discr <= 0.0 || a.abs() < 1e-6 {
        return DVec2::ZERO;
    }
    let sq = discr.sqrt();
    let t = (b - sq) / a;
    if !(t > 0.0) || t > p.tau0 * 3.0 {
        return DVec2::ZERO;
    }
    let m = 2.0;
    let f = -p.k * (-t / p.tau0).exp() * (rv - (rv * b - w * a) / sq) / (a * t.powf(m))
        * (m / t + 1.0 / p.tau0);
    let len = f.length();
    if len > 12.0 { f * (12.0 / len) } else { f }
}

pub fn step(walkers: &mut [Walker], blocks: &[Block], params: &CrowdParams, dt: f64) {
    if dt <= 0.0 || walkers.is_empty() {
        return;
    }
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, w) in walkers.iter().enumerate() {
        grid.entry(cell_of(w.pos)).or_default().push(i);
    }
    let reach = (params.sense / CELL).ceil() as i32;
    let mut new_vel = vec![DVec2::ZERO; walkers.len()];
    for (i, me) in walkers.iter().enumerate() {
        if me.fixed {
            continue;
        }
        let mut force = (me.want - me.vel) / params.relax;
        let mut avoid = DVec2::ZERO;
        let (cx, cy) = cell_of(me.pos);
        if !me.ghost {
            for gy in cy - reach..=cy + reach {
                for gx in cx - reach..=cx + reach {
                    let Some(list) = grid.get(&(gx, gy)) else {
                        continue;
                    };
                    for &j in list {
                        if j == i {
                            continue;
                        }
                        let o = &walkers[j];
                        let d = o.pos - me.pos;
                        if d.length_squared() > params.sense * params.sense {
                            continue;
                        }
                        // nobody minds the people behind them
                        if d.dot(me.want) < 0.0 && d.length() > me.radius + o.radius + 0.1 {
                            continue;
                        }
                        avoid +=
                            ttc_force(params, me.pos, me.vel, o.pos, o.vel, me.radius + o.radius);
                    }
                }
            }
        }
        for b in blocks {
            if !b.near(me.pos, params.sense) {
                continue;
            }
            let (q, inside) = b.closest(me.pos);
            if inside {
                continue;
            }
            avoid += ttc_force(params, me.pos, me.vel, q, b.vel, me.radius + 0.05);
        }
        // Straight at somebody the force only brakes, and two people stop nose to nose:
        // a walker then steps to the right, as people do.
        let wl = me.want.length();
        let al = avoid.length();
        if wl > 0.1 && al > 1e-3 && avoid.dot(me.want) < -0.9 * wl * al {
            avoid += DVec2::new(me.want.y, -me.want.x) / wl * al * 0.5;
        }
        // somebody who stands still only makes a little room
        force += avoid * me.give.clamp(0.0, 1.0);
        let len = force.length();
        if len > params.max_accel {
            force *= params.max_accel / len;
        }
        let mut v = me.vel + force * dt;
        // a walker never goes much faster than it wants to; a standing person only shuffles
        let cap = (me.want.length() * 1.2).max(0.3);
        let vl = v.length();
        if vl > cap {
            v *= cap / vl;
        }
        new_vel[i] = v;
    }
    for (i, w) in walkers.iter_mut().enumerate() {
        if w.fixed {
            continue;
        }
        w.vel = new_vel[i];
        w.pos += w.vel * dt;
        if let Some((a, b, dev)) = w.corridor {
            w.pos = ease_into_corridor(w.pos, a, b, dev, dt);
        }
    }
    for _ in 0..2 {
        for i in 0..walkers.len() {
            let (cx, cy) = cell_of(walkers[i].pos);
            for gy in cy - 1..=cy + 1 {
                for gx in cx - 1..=cx + 1 {
                    let Some(list) = grid.get(&(gx, gy)) else {
                        continue;
                    };
                    for &j in list {
                        if j <= i {
                            continue;
                        }
                        let (a, b) = (walkers[i], walkers[j]);
                        if a.ghost || b.ghost {
                            continue;
                        }
                        let d = b.pos - a.pos;
                        let dist = d.length();
                        let min = a.radius + b.radius;
                        if dist >= min {
                            continue;
                        }
                        let ga = if a.fixed { 0.0 } else { a.give.max(0.05) };
                        let gb = if b.fixed { 0.0 } else { b.give.max(0.05) };
                        if ga + gb <= 0.0 {
                            continue;
                        }
                        let n = if dist > 1e-6 {
                            d / dist
                        } else {
                            DVec2::new(((i * 7 + j * 3) % 5) as f64 - 2.0, 1.0).normalize()
                        };
                        let push = min - dist;
                        walkers[i].pos -= n * push * ga / (ga + gb);
                        walkers[j].pos += n * push * gb / (ga + gb);
                    }
                }
            }
        }
        for w in walkers.iter_mut() {
            if w.fixed {
                continue;
            }
            for b in blocks {
                if !b.near(w.pos, w.radius + 0.1) {
                    continue;
                }
                let (q, inside) = b.closest(w.pos);
                let d = w.pos - q;
                let dist = d.length();
                if inside {
                    let n = if dist > 1e-6 { -d / dist } else { DVec2::X };
                    w.pos = q + n * w.radius;
                } else if dist < w.radius {
                    let n = if dist > 1e-6 { d / dist } else { DVec2::X };
                    w.pos = q + n * w.radius;
                }
            }
        }
    }
    for w in walkers.iter_mut() {
        if let (false, Some((a, b, dev))) = (w.fixed, w.corridor) {
            w.pos = ease_into_corridor(w.pos, a, b, dev, dt);
        }
    }
}

/// Clamped outright, a stroller jumped 5-6 cm wherever one pavement path took over from the
/// next.
fn ease_into_corridor(p: DVec2, a: DVec2, b: DVec2, max_dev: f64, dt: f64) -> DVec2 {
    let q = clamp_to_corridor(p, a, b, max_dev);
    let d = q - p;
    let step = 0.6 * dt.max(0.0) + 0.002;
    if d.length() <= step {
        q
    } else {
        p + d * (step / d.length())
    }
}

pub fn turn_towards(from: f64, to: f64, rate: f64, dt: f64) -> f64 {
    let diff = angle_diff(from, to);
    // proportional near the target (time constant 0.25 s), capped at `rate`
    let step = (diff * (dt / 0.25).min(1.0)).clamp(-rate * dt, rate * dt);
    let out = from + step;
    out.rem_euclid(360.0)
}

/// Signed difference `to - from` in degrees, the short way round (-180..180).
pub fn angle_diff(from: f64, to: f64) -> f64 {
    let mut diff = (to - from) % 360.0;
    if diff > 180.0 {
        diff -= 360.0;
    } else if diff < -180.0 {
        diff += 360.0;
    }
    diff
}

/// Heading (degrees clockwise from north) of a ground direction.
pub fn heading_of(d: DVec2) -> f64 {
    d.x.atan2(d.y).to_degrees()
}

pub fn project_on_segment(p: DVec2, a: DVec2, b: DVec2) -> (DVec2, f64) {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (a + ab * t, t)
}

fn clamp_to_corridor(p: DVec2, a: DVec2, b: DVec2, max_dev: f64) -> DVec2 {
    let (q, _) = project_on_segment(p, a, b);
    let d = p - q;
    let len = d.length();
    if len > max_dev {
        q + d * (max_dev / len)
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walker(x: f64, y: f64, want: DVec2) -> Walker {
        Walker {
            pos: DVec2::new(x, y),
            vel: want,
            radius: 0.25,
            want,
            give: 1.0,
            fixed: false,
            ghost: false,
            corridor: None,
        }
    }

    #[test]
    fn head_on_walkers_pass_without_touching() {
        let mut w = vec![
            walker(0.0, -5.0, DVec2::new(0.0, 1.3)),
            walker(0.05, 5.0, DVec2::new(0.0, -1.3)),
        ];
        let p = CrowdParams::default();
        let mut closest = f64::MAX;
        for _ in 0..300 {
            w[0].want = DVec2::new(0.0, 1.3);
            w[1].want = DVec2::new(0.0, -1.3);
            step(&mut w, &[], &p, 1.0 / 30.0);
            closest = closest.min((w[0].pos - w[1].pos).length());
        }
        assert!(closest >= 0.5 - 1e-6, "closest approach {closest}");
        // both got past each other and kept going
        assert!(
            w[0].pos.y > 3.0 && w[1].pos.y < -3.0,
            "{:?} {:?}",
            w[0].pos,
            w[1].pos
        );
        // and they did not swerve wildly
        assert!(w[0].pos.x.abs() < 1.5 && w[1].pos.x.abs() < 1.5);
    }

    #[test]
    fn walker_goes_round_a_standing_person() {
        let mut w = vec![
            walker(0.0, -4.0, DVec2::new(0.0, 1.2)),
            Walker {
                give: 0.3,
                ..walker(0.0, 0.0, DVec2::ZERO)
            },
        ];
        let p = CrowdParams::default();
        let mut closest = f64::MAX;
        for _ in 0..300 {
            w[0].want = DVec2::new(0.0, 1.2);
            w[1].want = -w[1].pos * 1.5; // keeps to its spot
            step(&mut w, &[], &p, 1.0 / 30.0);
            closest = closest.min((w[0].pos - w[1].pos).length());
        }
        assert!(closest >= 0.5 - 1e-6);
        assert!(w[0].pos.y > 3.0);
        assert!(
            w[1].pos.length() < 0.4,
            "the standing person drifted to {:?}",
            w[1].pos
        );
    }

    #[test]
    fn crowd_never_interpenetrates() {
        // twelve people walking to the same point from a ring
        let mut w: Vec<Walker> = (0..12)
            .map(|k| {
                let a = k as f64 / 12.0 * std::f64::consts::TAU;
                walker(a.cos() * 5.0, a.sin() * 5.0, DVec2::ZERO)
            })
            .collect();
        let p = CrowdParams::default();
        for _ in 0..600 {
            for x in w.iter_mut() {
                let d = -x.pos;
                x.want = if d.length() > 0.1 {
                    d.normalize() * 1.2
                } else {
                    DVec2::ZERO
                };
            }
            step(&mut w, &[], &p, 1.0 / 30.0);
        }
        for i in 0..w.len() {
            for j in i + 1..w.len() {
                let d = (w[i].pos - w[j].pos).length();
                assert!(d > 0.45, "{i} and {j} overlap: {d}");
            }
        }
    }

    #[test]
    fn velocity_changes_smoothly() {
        // a walker told to reverse at once does not flip its velocity in one frame
        let mut w = vec![walker(0.0, 0.0, DVec2::new(0.0, 1.3))];
        let p = CrowdParams::default();
        w[0].want = DVec2::new(0.0, -1.3);
        let dt = 1.0 / 30.0;
        let before = w[0].vel;
        step(&mut w, &[], &p, dt);
        assert!(
            (w[0].vel - before).length() <= p.max_accel * dt + 1e-9,
            "{:?}",
            w[0].vel
        );
    }

    #[test]
    fn nobody_walks_through_a_bus() {
        let bus = Block {
            center: DVec2::ZERO,
            half: DVec2::new(1.25, 6.0),
            heading: 0.0,
            vel: DVec2::ZERO,
        };
        let mut w = vec![walker(-4.0, 0.3, DVec2::new(1.3, 0.0))];
        let p = CrowdParams::default();
        for _ in 0..400 {
            let d = DVec2::new(4.0, 0.0) - w[0].pos;
            w[0].want = if d.length() > 0.1 {
                d.normalize() * 1.3
            } else {
                DVec2::ZERO
            };
            step(&mut w, &[bus], &p, 1.0 / 30.0);
            let (_, inside) = bus.closest(w[0].pos);
            assert!(!inside, "inside the bus at {:?}", w[0].pos);
        }
    }

    #[test]
    fn ghosts_pass_through_each_other() {
        // two people pressed against each other in a doorway: as ghosts they get past
        let mut w = vec![
            Walker {
                ghost: true,
                ..walker(0.0, -0.2, DVec2::new(0.0, 1.0))
            },
            Walker {
                ghost: true,
                ..walker(0.0, 0.2, DVec2::new(0.0, -1.0))
            },
        ];
        let p = CrowdParams::default();
        for _ in 0..60 {
            w[0].want = DVec2::new(0.0, 1.0);
            w[1].want = DVec2::new(0.0, -1.0);
            step(&mut w, &[], &p, 1.0 / 30.0);
        }
        assert!(
            w[0].pos.y > 1.0 && w[1].pos.y < -1.0,
            "{:?} {:?}",
            w[0].pos,
            w[1].pos
        );
    }

    #[test]
    fn corridor_clamp_keeps_people_in_the_aisle() {
        let p = clamp_to_corridor(
            DVec2::new(1.0, 2.0),
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 5.0),
            0.3,
        );
        assert!((p - DVec2::new(0.3, 2.0)).length() < 1e-9, "{p:?}");
        let q = clamp_to_corridor(
            DVec2::new(0.1, 2.0),
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 5.0),
            0.3,
        );
        assert!((q - DVec2::new(0.1, 2.0)).length() < 1e-9);
    }

    #[test]
    fn heading_turns_smoothly_the_short_way() {
        let mut h = 350.0;
        for _ in 0..30 {
            h = turn_towards(h, 20.0, 180.0, 1.0 / 30.0);
        }
        assert!((h - 20.0).abs() < 3.0 || (h - 20.0).abs() > 357.0, "{h}");
        let h1 = turn_towards(0.0, 180.0, 90.0, 0.1);
        assert!((h1 - 9.0).abs() < 1e-6 || (h1 - 351.0).abs() < 1e-6, "{h1}");
        assert!((angle_diff(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((heading_of(DVec2::new(1.0, 0.0)) - 90.0).abs() < 1e-9);
    }
}
