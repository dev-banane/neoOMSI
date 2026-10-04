use super::*;

const CHAT_PAUSE: f64 = 12.0;

pub struct VoiceLine {
    pub position: DVec3,
    pub path: PathBuf,
}

/// Omsi.exe's watch on the driving (0x7d65d4 - 0x7d6b7f).
#[derive(Debug, Clone, Default)]
pub(super) struct RideComfort {
    fast_long: f32,
    slow_lat: f32,
    up: bool,
    swing_ms: f64,
    swings: u32,
    hard_ms: f64,
    crashes: u32,
}

impl RideComfort {
    pub(super) fn step(
        &mut self,
        dt: f32,
        now_ms: f64,
        speed: f32,
        lat: f32,
        long: f32,
        crashes: u32,
    ) -> f32 {
        let w = speed.abs().min(1.0);
        let kf = (10.0 * dt).min(0.5);
        let ks = dt.min(0.5);
        self.fast_long = w * long * kf + (1.0 - kf) * self.fast_long;
        self.slow_lat = w * lat * ks + (1.0 - ks) * self.slow_lat;
        let mut k: f32 = 0.0;
        let swing = if self.fast_long > 0.2 && !self.up {
            Some(true)
        } else if self.fast_long < -0.2 && self.up {
            Some(false)
        } else {
            None
        };
        if let Some(up) = swing {
            // back within half a second is no swing
            if now_ms < self.swing_ms + 4000.0 && (up || now_ms > self.swing_ms + 500.0) {
                self.swings += 1;
                if self.swings > 4 {
                    k = 0.05;
                }
            } else {
                self.swings = 0;
            }
            self.swing_ms = now_ms;
            self.up = up;
        }
        if self.slow_lat.abs() > 3.0 || self.fast_long.abs() > 5.0 {
            if self.hard_ms + 1000.0 < now_ms {
                k = 0.1;
            }
            self.hard_ms = now_ms;
        }
        if crashes > self.crashes {
            k = k.max(0.15);
        }
        self.crashes = crashes;
        k
    }
}

fn bad_ride_thresholds(r: [f32; 3]) -> [f32; 3] {
    let a = 0.1 * r[0];
    [a, 0.1 + a.max(0.1) + 0.2 * r[1], 0.5 + 0.3 * r[2]]
}

fn bad_ride_complaint(x: f32, said: u8, at: [f32; 3]) -> Option<u8> {
    (1..=3).rev().find(|&c| at[c as usize - 1] <= x && said < c)
}

impl Humans {
    pub fn take_voice_lines(&mut self) -> Vec<VoiceLine> {
        std::mem::take(&mut self.voice_lines)
    }

    /// `limited` (greetings, complaints): one at a time, and not the same file twice within 10 s.
    pub(super) fn say(&mut self, i: usize, name: &str, limited: bool) -> bool {
        match self.voices {
            2 => return false,
            1 if !name.starts_with("Ticket_") => return false,
            _ => {}
        }
        if limited && self.time - self.last_chat < CHAT_PAUSE && self.time >= self.last_chat {
            return false;
        }
        // without a `[voicepath]` the voices are in the pack's own folder (Berlin_1, Berlin_86)
        let Some(base) = self.tickets.as_ref().and_then(|t| match &t.voice_path {
            Some(vp) if !vp.trim().is_empty() => {
                Some(omsi_cfg::resolve_path(&self.root, vp.trim()))
            }
            _ => t.path.parent().map(|p| p.to_path_buf()),
        }) else {
            return false;
        };
        let voice = self.people[i].ty.def.voice.trim().to_string();
        if voice.is_empty() {
            return false;
        }
        let file = format!("{name}.wav");
        let mut path = omsi_cfg::resolve_path(&omsi_cfg::resolve_path(&base, &voice), &file);
        if !omsi_cfg::vfs::is_file(&path) {
            // packs without driving complaints (Bowdenham V5) borrow Berlin's
            let standard = omsi_cfg::resolve_path(&self.root, "TicketPacks\\Berlin_86");
            path = omsi_cfg::resolve_path(&omsi_cfg::resolve_path(&standard, &voice), &file);
            if !name.starts_with("TooBad_") || !omsi_cfg::vfs::is_file(&path) {
                return false;
            }
        }
        if limited
            && self
                .voice_said
                .get(&path)
                .is_some_and(|&t| self.time - t < 10.0 && self.time >= t)
        {
            return false;
        }
        self.voice_said.insert(path.clone(), self.time);
        if limited {
            self.last_chat = self.time;
        }
        log::debug!("{} says {name}", self.people[i].label());
        self.voice_lines.push(VoiceLine {
            position: self.people[i].position + DVec3::new(0.0, 0.0, 1.6),
            path,
        });
        true
    }

    pub(super) fn ride_comfort(&mut self, dt: f32, f: &Frame) {
        let Some(v) = f.player_bus else { return };
        if dt <= 0.0 || self.avatar_only {
            return;
        }
        let a = v.physics.a_trans;
        let k = self
            .comfort
            .step(dt, self.time * 1000.0, v.physics.speed, a.x, a.y, v.crashes);
        if k <= 0.0 {
            return;
        }
        for i in 0..self.people.len() {
            if self.people[i].remote || self.people[i].avatar {
                continue;
            }
            let Some(p) = self.pax(i) else { continue };
            if p.bus != Some(BusId::Player)
                || p.inside != Some(BusId::Player)
                || !matches!(
                    p.task,
                    Task::InBusToPlace | Task::InBusToExit | Task::SittingInBus
                )
            {
                continue;
            }
            if p.bad_at[2] <= 0.0 {
                let r = [
                    self.rand_f() as f32,
                    self.rand_f() as f32,
                    self.rand_f() as f32,
                ];
                self.pax_mut(i).unwrap().bad_at = bad_ride_thresholds(r);
            }
            let p = self.pax_mut(i).unwrap();
            p.discomfort += (1.0 - p.discomfort) * k;
            let Some(c) = bad_ride_complaint(p.discomfort, p.complaint, p.bad_at) else {
                continue;
            };
            p.complaint = c;
            log::debug!(
                "{} complains about the driving ({c})",
                self.people[i].label()
            );
            self.say(
                i,
                ["TooBad_A", "TooBad_B", "TooBad_C"][c as usize - 1],
                true,
            );
            if c == 3 {
                self.message = Some("Passengers want to leave because of your driving.".into());
                self.set_task(i, Task::InBusToExit, f);
            }
        }
    }

    pub(super) fn greet_or_complain(&mut self, i: usize, bn: &BusNow) {
        let Some((whinge, chat)) = self.tickets.as_ref().map(|t| (t.whinge_prop, t.chattiness))
        else {
            return;
        };
        let air = bn.air;
        let mut complaint_seen = false;
        let mut code = 0u8;
        if bn.interior < 0.5 {
            let r = self.rand_f() as f32;
            if air.brightness < 0.2 + 0.3 * r {
                complaint_seen = true;
                if (self.rand_f() as f32) < whinge {
                    code = 1;
                }
            }
        }
        if let Some(t) = air.temp {
            let out = air.outside;
            let r = (self.rand() % 10) as f32 + 25.0;
            let hot = t > r && t > (self.rand() % 5) as f32 + 3.0 + out;
            let hot = hot || out * 0.5 + (self.rand() % 10) as f32 + 20.0 < t && t < 25.0;
            if hot {
                complaint_seen = true;
                if code == 0 && (self.rand_f() as f32) < whinge {
                    code = if air.rel_hum <= 0.9 + 0.1 * self.rand_f() as f32 {
                        3
                    } else {
                        5
                    };
                }
            }
            let r = (self.rand() % 10) as f32 + 8.0;
            let cold = if t >= r {
                t < (out - (self.rand() % 10) as f32) - 10.0
            } else {
                t < out + 5.0 + (self.rand() % 5) as f32
                    || t < (out - (self.rand() % 10) as f32) - 10.0
            };
            if cold {
                complaint_seen = true;
                if code == 0 && (self.rand_f() as f32) < whinge {
                    code = 4;
                }
            }
        }
        if self.delay > 300.0 {
            complaint_seen = true;
            if code == 0 && (self.rand_f() as f32) < whinge {
                code = 2;
            }
        }
        let k = 1 + self.rand() % 2;
        match code {
            1..=4 => {
                let what = ["TooDark", "TooLate", "TooHot", "TooCold"][code as usize - 1];
                self.say(i, &format!("{what}_{k}"), true);
            }
            5 => {
                self.say(i, "TooWet_1", true);
            }
            _ if (self.rand_f() as f32) < chat => {
                let h = (self.time_of_day.rem_euclid(86_400.0) / 3600.0).floor() as i32;
                let daypart = match h {
                    3..=10 => 1,
                    18..=23 => 2,
                    _ => 0,
                };
                let k = self.rand() % if daypart == 0 { 2 } else { 3 };
                let line = match (k, daypart) {
                    (0 | 1, _) => format!("Hello_{}", k + 1),
                    (_, 1) => "GoodMorning_1".into(),
                    _ => "GoodEvening_1".into(),
                };
                self.say(i, &line, false);
            }
            _ => {}
        }
        self.stepped_in += 1;
        if !complaint_seen {
            self.content += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_riders_feel_hard_braking_fast_bends_and_a_jerky_foot() {
        let dt = 0.02;
        let run = |f: &dyn Fn(f64) -> (f32, f32, f32), secs: f64| {
            let mut c = RideComfort::default();
            let mut jolts = Vec::new();
            let mut t = 0.0;
            while t < secs {
                let (v, lat, long) = f(t);
                let k = c.step(dt, (t + 10.0) * 1000.0, v, lat, long, 0);
                if k > 0.0 {
                    jolts.push((t, k));
                }
                t += dt as f64;
            }
            jolts
        };
        // pulling away at 1.2 m/s², cruising, braking at 1.5 m/s² to a stop: nothing
        let smooth = |t: f64| match t {
            t if t < 10.0 => (1.2 * t as f32, 0.0, 1.2),
            t if t < 20.0 => (12.0, 0.0, 0.0),
            t if t < 28.0 => (12.0 - 1.5 * (t as f32 - 20.0), 0.0, -1.5),
            _ => (0.0, 0.0, 0.0),
        };
        assert!(run(&smooth, 40.0).is_empty());
        assert!(run(&|_| (10.0, 1.5, 0.0), 10.0).is_empty());
        // an emergency stop at 7 m/s²: one jolt, not one a frame
        let hard = run(&|t| (14.0, 0.0, if t < 1.0 { 0.0 } else { -7.0 }), 2.0);
        assert_eq!(hard.len(), 1, "{hard:?}");
        assert_eq!(hard[0].1, 0.1);
        assert_eq!(run(&|_| (12.0, 4.0, 0.0), 6.0).len(), 1);
        // throttle and brake every 1.5 s: the fifth swing on upsets them, each further one too
        let jerky = run(
            &|t| {
                (
                    8.0,
                    0.0,
                    if (t / 1.5).floor() as i64 % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    },
                )
            },
            15.0,
        );
        assert!(
            jerky.len() >= 4 && jerky.iter().all(|j| j.1 == 0.05) && jerky[0].0 > 5.0,
            "{jerky:?}"
        );
        assert!(run(&|_| (0.0, 5.0, -8.0), 5.0).is_empty());
        let mut impact = RideComfort::default();
        assert_eq!(impact.step(dt, 1_000.0, 10.0, 0.0, 0.0, 0), 0.0);
        assert_eq!(impact.step(dt, 1_020.0, 0.0, 0.0, 0.0, 1), 0.15);
        assert_eq!(impact.step(dt, 1_040.0, 0.0, 0.0, 0.0, 1), 0.0);
    }

    #[test]
    fn complaints_come_worst_first_and_once_each() {
        let at = bad_ride_thresholds([0.5, 0.5, 0.5]);
        assert!(
            (at[0] - 0.05).abs() < 1e-6
                && (at[1] - 0.3).abs() < 1e-6
                && (at[2] - 0.65).abs() < 1e-6
        );
        let (lo, hi) = (bad_ride_thresholds([0.0; 3]), bad_ride_thresholds([1.0; 3]));
        assert!(
            lo[1] >= 0.2 - 1e-6
                && hi[1] <= 0.4 + 1e-6
                && lo[2] >= 0.5 - 1e-6
                && hi[2] <= 0.8 + 1e-6
        );
        assert_eq!(bad_ride_complaint(0.01, 0, at), None);
        assert_eq!(bad_ride_complaint(0.1, 0, at), Some(1));
        assert_eq!(bad_ride_complaint(0.1, 1, at), None);
        assert_eq!(bad_ride_complaint(0.35, 1, at), Some(2));
        assert_eq!(bad_ride_complaint(0.9, 0, at), Some(3));
        assert_eq!(bad_ride_complaint(0.95, 3, at), None);
    }
}
