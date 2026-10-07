use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Look {
    Road,
    Mirror(usize),
    Door,
    Desk,
    Around(f32, f32),
}

#[derive(Default, Clone, Copy)]
pub(super) struct Scene {
    /// Road wheels' angle in radians, right positive.
    pub(super) speed: f32,
    pub(super) steer: f32,
    pub(super) doors_open: bool,
    pub(super) customer: Option<Vec3>,
    pub(super) desk_hand: Option<Vec3>,
    pub(super) door: Option<Vec3>,
}

pub(super) struct Gaze {
    pub(super) mirrors: Vec<Vec3>,
    kerb_mirror: Option<usize>,
    road_mirror: Option<usize>,
    look: Look,
    hold: f32,
    next: f32,
    seed: u64,
    road_angle: f32,
    stood: f32,
    doors_were: bool,
}

impl Gaze {
    pub(super) fn new(mirrors: Vec<Vec3>, eye: Vec3, seed: u64) -> Gaze {
        let side = |s: f32| {
            mirrors
                .iter()
                .enumerate()
                .filter(|(_, m)| (m.x - eye.x) * s > 0.3)
                .min_by(|a, b| a.1.z.total_cmp(&b.1.z))
                .map(|(i, _)| i)
        };
        Gaze {
            kerb_mirror: side(1.0),
            road_mirror: side(-1.0),
            mirrors,
            look: Look::Road,
            hold: 0.0,
            next: 2.0,
            seed: seed | 1,
            road_angle: 0.0,
            stood: 0.0,
            doors_were: false,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(super) fn update(
        &mut self,
        s: &Scene,
        eye: Vec3,
        fwd: Vec3,
        wheelbase: f32,
        dt: f32,
    ) -> Vec3 {
        // along the arc the wheels are turned for: into the bend before the bus turns
        let reach = (s.speed * 2.5).clamp(14.0, 35.0);
        let curve = s.steer.clamp(-0.8, 0.8).tan() / wheelbase.max(2.0);
        let want = (reach * curve / 1.5).clamp(-1.1, 1.1);
        self.road_angle += (want - self.road_angle) * (dt / 0.35).min(1.0);
        let right = Vec3::new(fwd.y, -fwd.x, 0.0);
        let (sa, ca) = self.road_angle.sin_cos();
        let road = eye + (fwd * ca + right * sa) * reach - Vec3::Z * (eye.z - 1.0);

        self.hold -= dt;
        self.next -= dt;
        let busy_with_desk = s.customer.is_some();
        let closed = self.doors_were && !s.doors_open;
        let set_off = s.speed > 0.3 && self.stood > 2.0;
        self.doors_were = s.doors_open;
        self.stood = if s.speed < 0.3 { self.stood + dt } else { 0.0 };
        if (closed || set_off)
            && !busy_with_desk
            && let Some(m) = self.road_mirror
        {
            self.look = Look::Mirror(m);
            self.hold = 0.7 + 0.4 * self.rand();
            self.next = 3.0;
            self.stood = 0.0;
        }
        if busy_with_desk && !matches!(self.look, Look::Desk) && self.hold <= 0.0 {
            self.look = Look::Desk;
            self.hold = 1.2 + self.rand();
        }
        if self.hold <= 0.0 && !matches!(self.look, Look::Road) {
            self.look = Look::Road;
        }
        if self.next <= 0.0 && matches!(self.look, Look::Road) {
            let r = self.rand();
            let turning = s.steer.abs() > 0.15;
            let (look, hold, next) = if busy_with_desk {
                (Look::Desk, 1.5 + self.rand(), 0.5)
            } else if s.speed < 0.5 && s.doors_open {
                if r < 0.55 && s.door.is_some() {
                    (Look::Door, 1.4 + 1.6 * self.rand(), 0.6 + self.rand())
                } else if let (true, Some(m)) = (r < 0.85, self.kerb_mirror) {
                    (Look::Mirror(m), 0.9 + 0.8 * self.rand(), 0.8 + self.rand())
                } else {
                    (Look::Road, 0.0, 1.5 + 2.0 * self.rand())
                }
            } else if s.speed < 0.5 {
                if r < 0.4 {
                    let yaw = (self.rand() - 0.4) * 70.0;
                    (
                        Look::Around(yaw, -5.0 + 10.0 * self.rand()),
                        0.8 + self.rand(),
                        2.0 + 3.0 * self.rand(),
                    )
                } else {
                    (Look::Road, 0.0, 2.0 + 3.0 * self.rand())
                }
            } else if turning {
                let m = if s.steer > 0.0 {
                    self.kerb_mirror
                } else {
                    self.road_mirror
                };
                match m {
                    Some(m) if r < 0.5 => (
                        Look::Mirror(m),
                        0.5 + 0.3 * self.rand(),
                        2.5 + 2.0 * self.rand(),
                    ),
                    _ => (Look::Road, 0.0, 1.5 + self.rand()),
                }
            } else {
                let m = if r < 0.45 {
                    self.road_mirror
                } else if r < 0.7 {
                    self.kerb_mirror
                } else if r < 0.82 {
                    self.mirrors
                        .iter()
                        .enumerate()
                        .find(|(_, m)| (m.x - eye.x).abs() < 0.9 && m.z > eye.z)
                        .map(|(i, _)| i)
                } else {
                    None
                };
                match m {
                    Some(m) => (
                        Look::Mirror(m),
                        0.55 + 0.4 * self.rand(),
                        4.0 + 7.0 * self.rand(),
                    ),
                    None => {
                        let yaw = (self.rand() - 0.5) * 50.0;
                        (
                            Look::Around(yaw, -3.0),
                            0.6 + 0.4 * self.rand(),
                            4.0 + 6.0 * self.rand(),
                        )
                    }
                }
            };
            self.look = look;
            self.hold = hold;
            self.next = next;
        }
        match self.look {
            Look::Road => road,
            Look::Mirror(m) => self.mirrors.get(m).copied().unwrap_or(road),
            Look::Door => s.door.unwrap_or(road),
            Look::Desk => s.desk_hand.or(s.customer).unwrap_or(road),
            Look::Around(yaw, pitch) => {
                let (sy, cy) = yaw.to_radians().sin_cos();
                let d = fwd * cy + right * sy;
                eye + (d + Vec3::Z * pitch.to_radians().tan()) * 10.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gaze() -> (Gaze, Vec3) {
        let eye = Vec3::new(-0.7, 4.7, 1.75);
        let mirrors = vec![Vec3::new(-1.33, 5.47, 1.58), Vec3::new(1.31, 5.91, 2.13)];
        (Gaze::new(mirrors, eye, 7), eye)
    }

    #[test]
    fn the_driver_looks_into_the_bend_before_the_bus_turns() {
        let (mut g, eye) = gaze();
        let s = Scene {
            speed: 4.0,
            steer: 0.35,
            ..Default::default()
        };
        let mut at = Vec3::ZERO;
        for _ in 0..30 {
            at = g.update(&s, eye, Vec3::Y, 6.0, 1.0 / 30.0);
        }
        assert_eq!(g.look, Look::Road);
        let d = (at - eye).truncate();
        let angle = d.x.atan2(d.y).to_degrees();
        assert!(angle > 15.0 && angle < 70.0, "{angle}");
    }

    #[test]
    fn moving_off_from_a_stop_the_driver_checks_the_mirror_on_the_road_side() {
        let (mut g, eye) = gaze();
        let stand = Scene::default();
        for _ in 0..90 {
            g.update(&stand, eye, Vec3::Y, 6.0, 1.0 / 30.0);
        }
        let go = Scene {
            speed: 0.6,
            ..Default::default()
        };
        let at = g.update(&go, eye, Vec3::Y, 6.0, 1.0 / 30.0);
        assert_eq!(at, g.mirrors[0]);
    }
}
