use ::simulation::VehicleInstance;
use glam::{Mat4, Vec3};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Which {
    Throttle,
    Brake,
}

pub(super) struct Pedal {
    mesh: usize,
    var: String,
    pad: Vec3,
}

impl Pedal {
    pub(super) fn pad(&self, v: &VehicleInstance) -> Vec3 {
        let turn = v
            .mesh_transforms
            .get(self.mesh)
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        turn.transform_point3(self.pad)
    }

    pub(super) fn value(&self, v: &VehicleInstance) -> f32 {
        v.var(&self.var).unwrap_or(0.0)
    }
}

pub(super) struct Pedals {
    pub(super) throttle: Option<Pedal>,
    pub(super) brake: Option<Pedal>,
    pub(super) clutch: Option<Pedal>,
    on: Which,
    from: Vec3,
    t: f32,
    covering: f32,
}

const THROTTLE: &[&str] = &["throttle", "gaspedal", "fahrpedal"];
const BRAKE: &[&str] = &["brake", "bremspedal"];
const CLUTCH: &[&str] = &["clutch", "kupplung"];
const MOVE: f32 = 0.32;
const MOVE_LIFT: f32 = 0.035;
const HOVER: f32 = 0.018;

impl Pedals {
    pub(super) fn find(v: &VehicleInstance, hip: Vec3, heading: f32) -> Option<Pedals> {
        let h = heading.to_radians();
        let fwd = Vec3::new(h.sin(), h.cos(), 0.0);
        let mut found: [Option<(f32, Pedal)>; 3] = [None, None, None];
        for (i, vm) in v.ty.meshes.iter().enumerate() {
            let def = &v.ty.model.meshes[vm.def_index];
            for a in &def.animations {
                let n = a.variable.to_ascii_lowercase();
                let kind = if THROTTLE.contains(&n.as_str()) {
                    0
                } else if BRAKE.contains(&n.as_str()) {
                    1
                } else if CLUTCH.contains(&n.as_str()) {
                    2
                } else {
                    continue;
                };
                let pivot = ::simulation::anim::origin_matrix(&a.origins, vm.pivot)
                    .transform_point3(Vec3::ZERO);
                let positions = super::positions_of(v, i);
                let Some(pad) = pad_of(&positions, pivot) else {
                    continue;
                };
                let ahead = (pad - hip).dot(fwd);
                if pad.z > hip.z - 0.1 || !(0.2..1.2).contains(&ahead) {
                    continue;
                }
                let dist = (pad - hip).length();
                if found[kind].as_ref().is_none_or(|(d, _)| dist < *d) {
                    found[kind] = Some((
                        dist,
                        Pedal {
                            mesh: i,
                            var: a.variable.clone(),
                            pad,
                        },
                    ));
                }
            }
        }
        let [throttle, brake, clutch] = found.map(|f| f.map(|(_, p)| p));
        let name = |p: &Option<Pedal>| {
            p.as_ref()
                .map(|p| format!("{} ({:?})", p.var, p.pad))
                .unwrap_or_else(|| "none".into())
        };
        log::debug!(
            "driver: pedals: accelerator {}, brake {}, clutch {}",
            name(&throttle),
            name(&brake),
            name(&clutch)
        );
        let throttle = throttle?;
        let pad = throttle.pad;
        Some(Pedals {
            throttle: Some(throttle),
            brake,
            clutch,
            on: Which::Brake,
            from: pad,
            t: 1.0,
            covering: 0.0,
        })
    }

    pub(super) fn feet(&mut self, v: &VehicleInstance, speed: f32, dt: f32) -> [Option<Vec3>; 2] {
        let value = |p: &Option<Pedal>| p.as_ref().map(|p| p.value(v)).unwrap_or(0.0);
        let (gas, brake) = (value(&self.throttle), value(&self.brake));
        let want = if self.brake.is_none() {
            Which::Throttle
        } else if brake > 0.03 {
            Which::Brake
        } else if gas > 0.02 {
            Which::Throttle
        } else if speed < 0.4 {
            Which::Brake
        } else {
            self.on
        };
        if want != self.on {
            self.from = self.current(v).unwrap_or_default();
            self.on = want;
            self.t = 0.0;
        }
        self.t = (self.t + dt / MOVE).min(1.0);
        let cover = self.on == Which::Throttle && gas < 0.02;
        self.covering += ((if cover { 1.0 } else { 0.0 }) - self.covering) * (dt / 0.2).min(1.0);
        let right = self.current(v);
        let left = match &self.clutch {
            Some(c) if c.value(v) > 0.05 => Some(c.pad(v)),
            _ => None,
        };
        [left, right]
    }

    fn current(&self, v: &VehicleInstance) -> Option<Vec3> {
        let to = match self.on {
            Which::Throttle => self.throttle.as_ref(),
            Which::Brake => self.brake.as_ref().or(self.throttle.as_ref()),
        }?
        .pad(v)
            + Vec3::Z * (HOVER * self.covering);
        if self.t >= 1.0 {
            return Some(to);
        }
        let e = min_jerk(self.t);
        Some(self.from.lerp(to, e) + Vec3::Z * (MOVE_LIFT * (std::f32::consts::PI * self.t).sin()))
    }
}

fn min_jerk(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (10.0 - 15.0 * t + 6.0 * t * t)
}

/// A bus pedal's plate a little beyond its middle (its tip is out of the legs' reach), a
/// hanging car pedal's pad.
fn pad_of(positions: &[Vec3], pivot: Vec3) -> Option<Vec3> {
    if positions.len() < 4 {
        return None;
    }
    let max = positions
        .iter()
        .map(|p| (*p - pivot).length())
        .fold(0.0f32, f32::max);
    if max < 0.05 {
        return None;
    }
    let tip_pts: Vec<&Vec3> = positions
        .iter()
        .filter(|p| (**p - pivot).length() >= max * 0.8)
        .collect();
    let tip = tip_pts.iter().fold(Vec3::ZERO, |s, p| s + **p) / tip_pts.len().max(1) as f32;
    let c = if tip.z < pivot.z - 0.05 {
        tip
    } else {
        let dir = (tip - pivot).normalize_or(Vec3::Y);
        let along = |p: &Vec3| (*p - pivot).dot(dir);
        let lo = positions.iter().map(along).fold(f32::MAX, f32::min);
        let hi = positions.iter().map(along).fold(f32::MIN, f32::max);
        let at = lo + (hi - lo) * 0.6;
        let near: Vec<&Vec3> = positions
            .iter()
            .filter(|p| (along(p) - at).abs() <= (hi - lo) * 0.2)
            .collect();
        if near.is_empty() {
            tip
        } else {
            near.iter().fold(Vec3::ZERO, |s, p| s + **p) / near.len() as f32
        }
    };
    c.is_finite().then_some(c + Vec3::Z * 0.012)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_foot_presses_a_bus_pedal_above_its_middle_and_a_car_pedal_at_its_pad() {
        let up = Vec3::new(0.0, 0.6, 0.8);
        let plate: Vec<Vec3> = (0..=10)
            .flat_map(|i| {
                let a = up * (0.025 * i as f32);
                [a - Vec3::X * 0.04, a + Vec3::X * 0.04]
            })
            .collect();
        let pad = pad_of(&plate, Vec3::ZERO).unwrap();
        let along = pad.dot(up);
        assert!((0.13..0.17).contains(&along), "{along}");
        let arm: Vec<Vec3> = (0..=10)
            .map(|i| Vec3::new(0.0, 0.01 * i as f32, 0.3 - 0.03 * i as f32))
            .collect();
        let pad = pad_of(&arm, Vec3::new(0.0, 0.0, 0.3)).unwrap();
        assert!(pad.z < 0.06, "{pad:?}");
    }
}
